// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Adapter registry and fuse-at-build composition for the service-slm substrate.
//!
//! Tracks every LoRA adapter trained by the substrate as a versioned artifact with
//! full provenance: base model, corpus snapshot SHA, training timestamp, signer,
//! evaluation result, and promotion status.
//!
//! Loaded from `data/adapters/registry.yaml` at startup; serves as the source of
//! truth for `ComputeRequest.adapter_version` resolution and for the
//! `/v1/adapters/registry` introspection endpoint.
//!
//! Fuse-at-build composition (`fuse_adapters`) pre-composes adapter tuples into a
//! single GGUF delta for deployment on llama-server nodes that do not yet support
//! runtime hot-swap. The real merge implementation is deferred until the llama.cpp
//! LoRA-merge PR lands upstream; the current stub returns a symbolic composed ID so
//! the rest of the substrate can be wired end-to-end.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

/// One adapter entry. Lives at `data/adapters/registry.yaml` under
/// `adapters.<id>` keyed by canonical adapter ID
/// (e.g. `coding-lora-2026-05-18`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdapterEntry {
    /// Canonical adapter ID — populated by the registry loader after
    /// YAML parse from the key in the `adapters:` map.
    #[serde(skip)]
    pub id: String,
    /// Base model the adapter was trained against
    /// (e.g. `OLMo-2-1124-7B-Instruct-Q4_K_M`).
    pub base_model: String,
    /// SHA-256 (hex) of the corpus snapshot tarball used for training.
    /// Cross-reference with `data/training-corpus/snapshots/<date>/manifest.json`.
    pub corpus_sha: String,
    /// Doctrine version pinned at training time (e.g. `0.0.13`).
    pub doctrine_version: String,
    /// ISO 8601 UTC timestamp when training completed.
    pub trained_at: String,
    /// SSH-key fingerprint or operator identity that signed the
    /// promotion. Empty string when the adapter has not yet promoted
    /// past `eval_pending` stage.
    #[serde(default)]
    pub signer: String,
    /// On-disk path to the adapter weights (relative to FOUNDRY_ROOT).
    pub weights_path: String,
    /// Eval result against the held-out set: `pass` | `fail` | `pending`.
    /// `bin/eval-adapter.sh` writes this field.
    #[serde(default = "default_eval_pending")]
    pub eval_result: String,
    /// Eval-pass percentage (0.0-1.0) when `eval_result == "pass"` or
    /// `"fail"`. Useful for regression detection across versions.
    #[serde(default)]
    pub eval_score: Option<f64>,
    /// Promotion stage:
    ///   - `eval_pending` — training complete; eval not yet run
    ///   - `eval_ok` — eval passed but not promoted to production
    ///   - `promoted` — adapter loaded by production Tier-A / Tier-B
    ///   - `retired` — superseded by a newer version
    ///   - `rejected` — eval failed; never promoted
    #[serde(default = "default_stage_eval_pending")]
    pub stage: String,
    /// ISO 8601 timestamp the stage last changed (for audit replay).
    #[serde(default)]
    pub stage_changed_at: String,
    /// Free-form notes — operator scratch pad for context-sensitive
    /// caveats (e.g. "wedged on context length; defer until P2-2.8 lands").
    #[serde(default)]
    pub notes: String,
    /// Sigstore signature (base64). Empty in P3-3.4-skeleton; populated
    /// by P3-3.4-followup once key setup lands.
    #[serde(default)]
    pub signature: String,
}

fn default_eval_pending() -> String {
    "pending".to_string()
}

fn default_stage_eval_pending() -> String {
    "eval_pending".to_string()
}

/// Wire shape for `data/adapters/registry.yaml`. Serialize is required
/// so `write_registry` can round-trip the in-memory state back to disk
/// after appends/stage transitions.
#[derive(Debug, Serialize, Deserialize)]
struct RegistryWire {
    #[serde(default)]
    adapters: HashMap<String, AdapterEntry>,
}

/// In-memory adapter registry. Read-mostly; the production loader is
/// `AdapterRegistry::open(path)` at startup. Future mutations
/// (`bin/lora-update.sh` writing a new entry) go through
/// `AdapterRegistry::append` which appends to the YAML and refreshes
/// the in-memory state under the write lock.
pub struct AdapterRegistry {
    path: PathBuf,
    inner: RwLock<HashMap<String, AdapterEntry>>,
}

impl AdapterRegistry {
    /// Open the registry at `path`. Returns an empty registry (no error)
    /// when the file does not exist — the substrate degrades to "no
    /// adapters known" rather than failing service startup. Same pattern
    /// as `citations::CitationRegistry::open`.
    pub fn open(path: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let path = path.into();
        let entries = if path.exists() {
            load(&path)?
        } else {
            HashMap::new()
        };
        Ok(Self {
            path,
            inner: RwLock::new(entries),
        })
    }

    /// Standard entrypoint — reads `FOUNDRY_ROOT/data/adapters/registry.yaml`
    /// (overridable via `FOUNDRY_ADAPTER_REGISTRY_PATH`).
    pub fn from_env() -> anyhow::Result<Self> {
        let path = std::env::var_os("FOUNDRY_ADAPTER_REGISTRY_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let foundry_root: PathBuf = std::env::var_os("FOUNDRY_ROOT")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("/srv/foundry"));
                foundry_root
                    .join("data")
                    .join("adapters")
                    .join("registry.yaml")
            });
        Self::open(path)
    }

    /// Count registered adapters.
    pub fn len(&self) -> usize {
        self.inner.read().expect("registry poisoned").len()
    }

    /// Convenience.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Path to the underlying YAML file (diagnostic surface).
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Look up an adapter by canonical ID.
    pub fn get(&self, id: &str) -> Option<AdapterEntry> {
        self.inner
            .read()
            .expect("registry poisoned")
            .get(id)
            .cloned()
    }

    /// Return every adapter currently in the registry, sorted by
    /// `trained_at` descending (newest first).
    pub fn list(&self) -> Vec<AdapterEntry> {
        let guard = self.inner.read().expect("registry poisoned");
        let mut entries: Vec<AdapterEntry> = guard.values().cloned().collect();
        entries.sort_by(|a, b| b.trained_at.cmp(&a.trained_at));
        entries
    }

    /// Return only adapters at `stage == "promoted"`.
    pub fn promoted(&self) -> Vec<AdapterEntry> {
        self.list()
            .into_iter()
            .filter(|a| a.stage == "promoted")
            .collect()
    }

    /// Append a new entry to the registry. The YAML file is rewritten
    /// in-place under the write lock.
    pub fn append(&self, mut entry: AdapterEntry) -> anyhow::Result<()> {
        let mut guard = self.inner.write().expect("registry poisoned");
        if entry.stage_changed_at.is_empty() {
            entry.stage_changed_at =
                chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        }
        let id = entry.id.clone();
        if id.is_empty() {
            anyhow::bail!("adapter entry missing id");
        }
        guard.insert(id, entry);
        write_registry(&self.path, &guard)?;
        Ok(())
    }

    /// Update an existing entry's stage (e.g. `eval_pending` → `eval_ok`).
    pub fn set_stage(&self, id: &str, new_stage: &str) -> anyhow::Result<()> {
        let mut guard = self.inner.write().expect("registry poisoned");
        let entry = guard
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("unknown adapter: {id}"))?;
        entry.stage = new_stage.to_string();
        entry.stage_changed_at =
            chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        write_registry(&self.path, &guard)?;
        Ok(())
    }
}

fn load(path: &Path) -> anyhow::Result<HashMap<String, AdapterEntry>> {
    let body = std::fs::read_to_string(path)?;
    let wire: RegistryWire = serde_yaml::from_str(&body)?;
    let mut out = HashMap::new();
    for (id, mut entry) in wire.adapters.into_iter() {
        entry.id = id.clone();
        out.insert(id, entry);
    }
    Ok(out)
}

fn write_registry(path: &Path, entries: &HashMap<String, AdapterEntry>) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let wire = RegistryWire {
        adapters: entries.clone(),
    };
    let body = serde_yaml::to_string(&wire)?;
    let tmp = path.with_extension("yaml.tmp");
    std::fs::write(&tmp, body)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Fuse-at-build adapter composition.
///
/// Pre-composes a base adapter with zero or more overlay adapters into a single
/// composed identifier. The real GGUF merge is deferred until the llama.cpp
/// runtime LoRA hot-swap PR lands upstream; this stub returns a symbolic ID so
/// the rest of the training pipeline can be wired end-to-end without the
/// hardware-level constraint.
///
/// Returns a canonical composed ID in the form `base+overlay1+overlay2`.
pub fn fuse_adapters(base: &str, overlays: &[&str]) -> anyhow::Result<String> {
    if base.is_empty() {
        anyhow::bail!("fuse_adapters: base adapter ID must not be empty");
    }
    if overlays.is_empty() {
        return Ok(base.to_string());
    }
    Ok(format!("{}+{}", base, overlays.join("+")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "adapter-registry-{}.yaml",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ))
    }

    #[test]
    fn empty_when_file_missing() {
        let r = AdapterRegistry::open("/does/not/exist/registry.yaml").unwrap();
        assert!(r.is_empty());
        assert!(r.get("foo").is_none());
    }

    #[test]
    fn append_and_list_round_trip() {
        let path = tmp_path();
        let r = AdapterRegistry::open(&path).unwrap();
        assert!(r.is_empty());

        r.append(AdapterEntry {
            id: "coding-lora-v1".to_string(),
            base_model: "OLMo-2-7B".to_string(),
            corpus_sha: "abc123".to_string(),
            doctrine_version: "0.0.13".to_string(),
            trained_at: "2026-05-18T10:00:00Z".to_string(),
            signer: String::new(),
            weights_path: "data/lora/coding-lora-v1".to_string(),
            eval_result: "pending".to_string(),
            eval_score: None,
            stage: "eval_pending".to_string(),
            stage_changed_at: String::new(),
            notes: String::new(),
            signature: String::new(),
        })
        .unwrap();

        assert_eq!(r.len(), 1);
        let got = r.get("coding-lora-v1").unwrap();
        assert_eq!(got.base_model, "OLMo-2-7B");
        assert!(
            !got.stage_changed_at.is_empty(),
            "stage_changed_at auto-stamped"
        );

        let r2 = AdapterRegistry::open(&path).unwrap();
        assert_eq!(r2.len(), 1);
        assert_eq!(r2.get("coding-lora-v1").unwrap().corpus_sha, "abc123");

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn set_stage_advances_lifecycle() {
        let path = tmp_path();
        let r = AdapterRegistry::open(&path).unwrap();
        r.append(AdapterEntry {
            id: "lora-v2".to_string(),
            base_model: "OLMo".to_string(),
            corpus_sha: "def".to_string(),
            doctrine_version: "0.0.13".to_string(),
            trained_at: "2026-05-18T11:00:00Z".to_string(),
            signer: "".into(),
            weights_path: "data/lora/v2".to_string(),
            eval_result: "pending".to_string(),
            eval_score: None,
            stage: "eval_pending".to_string(),
            stage_changed_at: String::new(),
            notes: String::new(),
            signature: String::new(),
        })
        .unwrap();

        r.set_stage("lora-v2", "eval_ok").unwrap();
        assert_eq!(r.get("lora-v2").unwrap().stage, "eval_ok");
        r.set_stage("lora-v2", "promoted").unwrap();
        assert_eq!(r.get("lora-v2").unwrap().stage, "promoted");
        assert_eq!(r.promoted().len(), 1);

        let err = r.set_stage("does-not-exist", "promoted");
        assert!(err.is_err());

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn list_sorts_newest_first() {
        let path = tmp_path();
        let r = AdapterRegistry::open(&path).unwrap();
        for (id, ts) in [
            ("a", "2026-05-18T01:00:00Z"),
            ("b", "2026-05-18T03:00:00Z"),
            ("c", "2026-05-18T02:00:00Z"),
        ] {
            r.append(AdapterEntry {
                id: id.to_string(),
                base_model: "OLMo".to_string(),
                corpus_sha: id.to_string(),
                doctrine_version: "0.0.13".to_string(),
                trained_at: ts.to_string(),
                signer: "".into(),
                weights_path: format!("data/lora/{id}"),
                eval_result: "pending".to_string(),
                eval_score: None,
                stage: "eval_pending".to_string(),
                stage_changed_at: String::new(),
                notes: String::new(),
                signature: String::new(),
            })
            .unwrap();
        }
        let listed = r.list();
        assert_eq!(listed.len(), 3);
        assert_eq!(listed[0].id, "b");
        assert_eq!(listed[1].id, "c");
        assert_eq!(listed[2].id, "a");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn fuse_adapters_stub() {
        assert_eq!(fuse_adapters("base", &[]).unwrap(), "base");
        assert_eq!(
            fuse_adapters("constitutional", &["engineering", "role"]).unwrap(),
            "constitutional+engineering+role"
        );
        assert!(fuse_adapters("", &["x"]).is_err());
    }
}
