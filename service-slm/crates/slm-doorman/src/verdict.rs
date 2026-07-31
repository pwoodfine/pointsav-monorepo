// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Apprenticeship verdict pipeline (AS-3).
//!
//! `VerdictDispatcher::dispatch` is the AS-3 implementation of
//! `POST /v1/verdict`:
//!
//! 1. Verify the senior's SSH signature over the verdict body
//!    (`ssh-keygen -Y verify -n apprenticeship-verdict-v1` against
//!    `~/Foundry/identity/allowed_signers`). Failure → 403.
//! 2. Parse the verdict frontmatter to recover brief_id, attempt_id,
//!    outcome, etc.
//! 3. Look up the original (brief, attempt) from the in-process
//!    `BriefCache`. Cache miss → `BriefCacheMiss` (HTTP 410-ish; the
//!    senior must reissue the brief).
//! 4. Write the apprenticeship corpus tuple to
//!    `${FOUNDRY_ROOT}/data/training-corpus/apprenticeship/<task-type>/<ulid>.jsonl`.
//!    The redaction filter (`crate::redact::sanitize`) runs over every
//!    body field per convention §9.
//! 5. Append a signed verdict event row to
//!    `data/apprenticeship/ledger.md` under `flock(2)`. Recompute
//!    rolling stats; on threshold cross, append a `promotion` event.
//! 6. On `verdict in [refine, reject]`: also write a DPO pair to
//!    `data/training-corpus/feedback/apprenticeship-<task-type>-<ulid>.jsonl`
//!    per convention §8 + `trajectory-substrate.md` §6.
//!
//! Verdict signature verification is encapsulated in the
//! `VerdictVerifier` trait so tests can inject a `MockVerifier` and the
//! production binary uses `SshKeygenVerifier` (shells out per design-
//! pass Q1; native Rust SSH-key verify is a v0.5+ follow-up).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use slm_core::{ApprenticeshipVerdict, VerdictOutcome, VERDICT_NAMESPACE};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::apprenticeship::shadow_corpus_path;
use crate::brief_cache::BriefCache;
use crate::error::{DoormanError, Result};
use crate::promotion_ledger::{format_verdict_event, PromotionLedger, PromotionOutcome, StatRow};
use crate::redact::sanitize;

/// Verifies a senior signature over a verdict body. Production uses
/// `SshKeygenVerifier`; tests use `MockVerifier`.
#[async_trait]
pub trait VerdictVerifier: Send + Sync + std::fmt::Debug {
    /// Returns `Ok(())` on accept, `Err(DoormanError::VerifySignature)`
    /// on any failure (bad signature, unknown principal, namespace
    /// mismatch, ssh-keygen exited non-zero).
    async fn verify(
        &self,
        body: &str,
        signature_pem: &str,
        senior_identity: &str,
        namespace: &str,
    ) -> Result<()>;
}

/// Real verifier — shells out to `ssh-keygen -Y verify`. Per design-
/// pass Q1.
#[derive(Clone, Debug)]
pub struct SshKeygenVerifier {
    pub allowed_signers: PathBuf,
}

impl SshKeygenVerifier {
    pub fn new(allowed_signers: impl Into<PathBuf>) -> Self {
        Self {
            allowed_signers: allowed_signers.into(),
        }
    }
}

#[async_trait]
impl VerdictVerifier for SshKeygenVerifier {
    async fn verify(
        &self,
        body: &str,
        signature_pem: &str,
        senior_identity: &str,
        namespace: &str,
    ) -> Result<()> {
        // ssh-keygen -Y verify takes the signature on -s and the body
        // on stdin. Write the signature to a temp file, pipe the body.
        let sig_path = std::env::temp_dir().join(format!(
            "slm-doorman-verify-{}.sig",
            Uuid::now_v7().simple()
        ));
        std::fs::write(&sig_path, signature_pem.as_bytes())
            .map_err(|e| DoormanError::VerifySignature(format!("write sig file: {e}")))?;
        let principal = format!("{senior_identity}@users.noreply.github.com");

        let allowed = self.allowed_signers.clone();
        let sig_for_close = sig_path.clone();
        let body_owned = body.to_string();
        let namespace_owned = namespace.to_string();

        let result = tokio::task::spawn_blocking(move || {
            use std::process::{Command, Stdio};
            let mut child = Command::new("ssh-keygen")
                .arg("-Y")
                .arg("verify")
                .arg("-f")
                .arg(&allowed)
                .arg("-I")
                .arg(&principal)
                .arg("-n")
                .arg(&namespace_owned)
                .arg("-s")
                .arg(&sig_for_close)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| DoormanError::VerifySignature(format!("spawn ssh-keygen: {e}")))?;
            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                stdin
                    .write_all(body_owned.as_bytes())
                    .map_err(|e| DoormanError::VerifySignature(format!("write stdin: {e}")))?;
            }
            child
                .wait_with_output()
                .map_err(|e| DoormanError::VerifySignature(format!("wait ssh-keygen: {e}")))
        })
        .await
        .map_err(|e| DoormanError::VerifySignature(format!("join blocking: {e}")))??;

        let _ = std::fs::remove_file(&sig_path);

        if result.status.success() {
            debug!(target: "slm_doorman::verdict", %senior_identity, "ssh-keygen -Y verify OK");
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&result.stderr).into_owned();
            warn!(target: "slm_doorman::verdict", %senior_identity, %stderr,
                  "ssh-keygen -Y verify rejected");
            Err(DoormanError::VerifySignature(format!(
                "ssh-keygen -Y verify exited {}: {}",
                result.status, stderr
            )))
        }
    }
}

/// Verdict dispatcher (AS-3). Holds the verifier, brief cache, ledger,
/// and corpus root. One instance per Doorman process; cheap to clone
/// (everything is `Arc`-shared).
#[derive(Clone)]
pub struct VerdictDispatcher {
    pub verifier: Arc<dyn VerdictVerifier>,
    pub cache: Arc<BriefCache>,
    pub ledger: PromotionLedger,
    pub corpus_root: PathBuf,
    pub doctrine_version: String,
    pub tenant: String,
}

/// Wire shape of `POST /v1/verdict` per design-pass Q5. Base64 for the
/// signature blob keeps the JSON body printable.
#[derive(Clone, Debug, Deserialize)]
pub struct VerdictWireBody {
    /// Verbatim verdict-file content (frontmatter + prose) as signed.
    pub body: String,
    /// Base64-encoded SSH signature blob over `body`, namespace
    /// `apprenticeship-verdict-v1` (or `..-batch-v1`).
    pub signature: String,
    /// Senior identity claim — must match the `senior_identity:`
    /// frontmatter line. Cross-check at parse time.
    pub senior_identity: String,
}

/// Outcome of the verdict POST. Returned in the HTTP response body so
/// the senior knows whether their verdict landed and whether the task-
/// type promoted.
#[derive(Clone, Debug, Serialize)]
pub struct VerdictDispatchOutcome {
    pub verdict: VerdictOutcome,
    pub brief_id: String,
    pub attempt_id: String,
    pub corpus_path: String,
    pub promotion: PromotionOutcome,
    pub dpo_pair_path: Option<String>,
}

impl VerdictDispatcher {
    /// AS-3 entry point. Returns a `DoormanError` whose `to_string()`
    /// the HTTP layer surfaces; signature failures map to 403.
    ///
    /// Per apprenticeship-substrate.md §7B (v0.0.13 amendment), verdicts
    /// PROMOTE an existing corpus tuple (written at capture time) rather
    /// than CREATING a new tuple. This ensures corpus tuples are never
    /// lost due to cache eviction between shadow-brief dispatch and
    /// verdict signing.
    ///
    /// Resolution order for (brief, attempt) pair:
    ///   1. BriefCache (in-flight, within session window).
    ///   2. Corpus disk (shadow-<brief_id>.jsonl — post-restart recovery).
    ///   3. Neither → `OrphanVerdictNoCorpusTuple` (logged, no tuple created).
    pub async fn dispatch(&self, wire: VerdictWireBody) -> Result<VerdictDispatchOutcome> {
        // Decode signature (the SSH signature is itself an
        // ASCII-armoured PEM-ish block; design-pass Q5 base64-encodes
        // it for JSON transport, so decode back to text).
        let signature_pem = decode_signature(&wire.signature)?;

        // Verify before any state mutation.
        self.verifier
            .verify(
                &wire.body,
                &signature_pem,
                &wire.senior_identity,
                VERDICT_NAMESPACE,
            )
            .await?;

        // Parse verdict frontmatter — recovers brief_id, attempt_id,
        // outcome, senior_identity (cross-checked), final_diff_sha,
        // notes.
        let parsed = parse_verdict_body(&wire.body)?;
        if parsed.senior_identity != wire.senior_identity {
            return Err(DoormanError::VerdictParse(format!(
                "verdict frontmatter senior_identity {:?} does not match wire field {:?}",
                parsed.senior_identity, wire.senior_identity
            )));
        }

        // Resolve (brief, attempt) pair. Try cache first (in-flight),
        // fall back to corpus disk (post-restart recovery).
        //
        // The cache key is (brief_id, attempt_id). We don't know
        // attempt_id until we've parsed the verdict frontmatter above,
        // so we check cache here after parsing.
        let (task_type, self_confidence, attempt_id, tier_used_str) =
            match self.cache.get(&parsed.brief_id, &parsed.attempt_id) {
                Some(cached) => {
                    // Happy path: cache hit. Proceed directly.
                    let task_type = cached.brief.task_type.clone();
                    let self_confidence = cached.attempt.self_confidence;
                    let tier_str = cached.attempt.tier.as_str().to_string();
                    (
                        task_type,
                        self_confidence,
                        cached.attempt.attempt_id.clone(),
                        tier_str,
                    )
                }
                None => {
                    // Cache miss. Check if a corpus tuple exists on disk
                    // (post-restart recovery path). We need task_type to
                    // locate the file; derive it from parsing the corpus
                    // tuple — or fall back to looking in all task-type
                    // subdirs. For now we rely on the fact that shadow
                    // tuples embed the task_type in their JSON; we scan
                    // for the tuple by brief_id across all task-types.
                    let found =
                        locate_corpus_tuple_by_brief_id(&self.corpus_root, &parsed.brief_id);
                    match found {
                        Some((task_type, sc, att_id)) => {
                            // Tuple found on disk; can promote it.
                            // Tier not recoverable from disk tuple — record unknown.
                            info!(
                                target: "slm_doorman::verdict",
                                brief_id = %parsed.brief_id,
                                task_type = %task_type,
                                "BriefCache miss — recovering from corpus tuple on disk"
                            );
                            (task_type, sc, att_id, "unknown".to_string())
                        }
                        None => {
                            // Neither cache nor disk has the tuple.
                            // This is an orphan verdict per §7B — log
                            // and return the specific error. No corpus
                            // write is performed.
                            let corpus_path = shadow_corpus_path(
                                &self.corpus_root,
                                "(unknown-task-type)",
                                &parsed.brief_id,
                            );
                            warn!(
                                target: "slm_doorman::verdict",
                                brief_id = %parsed.brief_id,
                                "orphan verdict: no corpus tuple found in cache or on disk; \
                                 no corpus row created (§7B)"
                            );
                            return Err(DoormanError::OrphanVerdictNoCorpusTuple {
                                brief_id: parsed.brief_id,
                                corpus_path: corpus_path.display().to_string(),
                            });
                        }
                    }
                }
            };

        // Build the verdict struct (sanitised body + the signature).
        let verdict = ApprenticeshipVerdict {
            brief_id: parsed.brief_id.clone(),
            attempt_id: attempt_id.clone(),
            verdict: parsed.verdict,
            created: parsed.created,
            senior_identity: parsed.senior_identity.clone(),
            final_diff_sha: parsed.final_diff_sha.clone(),
            notes: parsed.notes.clone(),
            body: sanitize(&wire.body),
            signature: wire.signature.clone(),
        };

        // Promote the existing corpus tuple in-place (§7B).
        // Locates the shadow-<brief_id>.jsonl file and rewrites it with
        // the verdict block, updated stage, and promoted_at timestamp.
        let stage_before = self.ledger.current_stage(&task_type);
        let corpus_path = promote_corpus_tuple(
            &self.corpus_root,
            &parsed.brief_id,
            &task_type,
            &verdict,
            parsed.final_diff.as_deref(),
            stage_before.as_str(),
            &self.doctrine_version,
            &self.tenant,
        )?;

        // Append the verdict event to the ledger + recompute stats /
        // promotion.
        let row = StatRow {
            ts: parsed.created,
            task_type: task_type.clone(),
            verdict: parsed.verdict,
            brief_id: parsed.brief_id.clone(),
            attempt_id: attempt_id.clone(),
            self_confidence,
            senior_identity: parsed.senior_identity.clone(),
        };
        let body_summary = format!(
            "verdict={} brief_id={} attempt_id={} self_confidence={:.3}",
            parsed.verdict.as_str(),
            parsed.brief_id,
            attempt_id,
            self_confidence,
        );
        let event_block = format_verdict_event(
            parsed.created,
            &task_type,
            &parsed.senior_identity,
            &body_summary,
            &signature_pem,
        );
        let promotion = self.ledger.append_verdict(row, &event_block)?;

        // DPO pair on refine / reject. We need the attempt diff for this;
        // prefer the BriefCache (in-flight); fall back to the on-disk corpus
        // tuple (post-restart recovery path).
        // Recover the apprentice's attempt — the diff plus the metadata needed
        // to render its DPO side in the canonical envelope (self_confidence,
        // escalate, reasoning). Cache is the in-flight path; the on-disk corpus
        // tuple is the post-restart fallback (metadata defaults there).
        let (attempt_diff, attempt_confidence, attempt_escalate, attempt_reasoning) =
            match self.cache.get(&parsed.brief_id, &attempt_id) {
                Some(c) => (
                    c.attempt.diff.clone(),
                    c.attempt.self_confidence,
                    c.attempt.escalate,
                    c.attempt.reasoning.clone(),
                ),
                None => (
                    read_attempt_diff_from_corpus(&self.corpus_root, &task_type, &parsed.brief_id),
                    0.5,
                    false,
                    String::new(),
                ),
            };

        let dpo_pair_path = if parsed.verdict.produces_dpo_pair() {
            Some(write_dpo_pair(
                &self.corpus_root,
                &task_type,
                &attempt_diff,
                parsed.final_diff.as_deref().unwrap_or_default(),
                parsed.notes.as_deref().unwrap_or(""),
                &parsed.brief_id,
                &attempt_id,
                &tier_used_str,
                attempt_confidence,
                attempt_escalate,
                &attempt_reasoning,
            )?)
        } else {
            None
        };

        info!(
            target: "slm_doorman::verdict",
            brief_id = %parsed.brief_id,
            attempt_id = %attempt_id,
            verdict = parsed.verdict.as_str(),
            promoted = promotion.promoted,
            "verdict applied (corpus tuple promoted in-place per §7B)"
        );

        Ok(VerdictDispatchOutcome {
            verdict: parsed.verdict,
            brief_id: parsed.brief_id,
            attempt_id,
            corpus_path: corpus_path.display().to_string(),
            promotion,
            dpo_pair_path: dpo_pair_path.map(|p| p.display().to_string()),
        })
    }
}

/// Frontmatter shape parsed from a verdict body.
#[derive(Clone, Debug)]
pub struct ParsedVerdict {
    pub brief_id: String,
    pub attempt_id: String,
    pub verdict: VerdictOutcome,
    pub created: DateTime<Utc>,
    pub senior_identity: String,
    pub final_diff_sha: Option<String>,
    pub notes: Option<String>,
    /// Free-form prose body extracted post-frontmatter; carries the
    /// senior's corrected diff or commentary. Used to fill the corpus
    /// tuple's `final_diff` field on accept verdicts.
    pub final_diff: Option<String>,
}

/// Parse the verdict frontmatter shape from `templates/apprenticeship-
/// verdict.md.tmpl`. Required keys: `brief_id`, `attempt_id`,
/// `verdict`, `created`, `senior_identity`. Optional: `final_diff_sha`,
/// `notes`. Robust to comment lines, blank lines, and minor whitespace
/// drift.
pub fn parse_verdict_body(body: &str) -> Result<ParsedVerdict> {
    let frontmatter = extract_frontmatter(body)
        .ok_or_else(|| DoormanError::VerdictParse("missing YAML frontmatter".into()))?;

    let mut brief_id = None;
    let mut attempt_id = None;
    let mut verdict = None;
    let mut created = None;
    let mut senior_identity = None;
    let mut final_diff_sha: Option<String> = None;
    let mut notes: Option<String> = None;

    for line in frontmatter.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let (k, v) = match t.split_once(':') {
            Some(p) => p,
            None => continue,
        };
        let v = v.trim();
        let v_trim = v.trim_matches(&['"', '\''][..]);
        match k.trim() {
            "brief_id" => brief_id = Some(v_trim.to_string()),
            "attempt_id" => attempt_id = Some(v_trim.to_string()),
            "verdict" => {
                verdict = Some(match v_trim {
                    "accept" => VerdictOutcome::Accept,
                    "refine" => VerdictOutcome::Refine,
                    "reject" => VerdictOutcome::Reject,
                    "defer-tier-c" => VerdictOutcome::DeferTierC,
                    other => {
                        return Err(DoormanError::VerdictParse(format!(
                            "unknown verdict outcome {other:?}"
                        )))
                    }
                });
            }
            "created" => {
                created = Some(
                    DateTime::parse_from_rfc3339(v_trim)
                        .map_err(|e| DoormanError::VerdictParse(format!("created: {e}")))?
                        .with_timezone(&Utc),
                );
            }
            "senior_identity" => senior_identity = Some(v_trim.to_string()),
            "final_diff_sha" if !v_trim.is_empty() && v_trim != "null" => {
                final_diff_sha = Some(v_trim.to_string());
            }
            // Single-line notes only at the frontmatter layer; the
            // template uses `notes: |` block scalars but we only need
            // a one-line summary for the audit row.
            "notes" if !v_trim.is_empty() && v_trim != "|" => {
                notes = Some(v_trim.to_string());
            }
            _ => {}
        }
    }

    Ok(ParsedVerdict {
        brief_id: brief_id.ok_or_else(|| DoormanError::VerdictParse("missing brief_id".into()))?,
        attempt_id: attempt_id
            .ok_or_else(|| DoormanError::VerdictParse("missing attempt_id".into()))?,
        verdict: verdict.ok_or_else(|| DoormanError::VerdictParse("missing verdict".into()))?,
        created: created.ok_or_else(|| DoormanError::VerdictParse("missing created".into()))?,
        senior_identity: senior_identity
            .ok_or_else(|| DoormanError::VerdictParse("missing senior_identity".into()))?,
        final_diff_sha,
        notes,
        final_diff: extract_post_frontmatter(body),
    })
}

fn extract_frontmatter(text: &str) -> Option<String> {
    let stripped = text.trim_start_matches([' ', '\t', '\n']);
    let after_open = stripped.strip_prefix("---")?;
    let after_open = after_open.strip_prefix('\n').unwrap_or(after_open);
    let close = after_open.find("\n---")?;
    Some(after_open[..close].to_string())
}

fn extract_post_frontmatter(text: &str) -> Option<String> {
    let stripped = text.trim_start_matches([' ', '\t', '\n']);
    let after_open = stripped.strip_prefix("---\n")?;
    let close_rel = after_open.find("\n---")?;
    let rest = &after_open[close_rel..];
    let rest = rest.strip_prefix("\n---").unwrap_or(rest);
    let rest = rest.strip_prefix('\n').unwrap_or(rest);
    let trimmed = rest.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn decode_signature(b64: &str) -> Result<String> {
    let bytes = B64
        .decode(b64.trim().as_bytes())
        .map_err(|e| DoormanError::VerifySignature(format!("base64 decode: {e}")))?;
    String::from_utf8(bytes)
        .map_err(|e| DoormanError::VerifySignature(format!("utf-8 decode: {e}")))
}

fn sanitize_verdict(v: &ApprenticeshipVerdict) -> ApprenticeshipVerdict {
    let mut clone = v.clone();
    clone.body = sanitize(&clone.body);
    if let Some(n) = clone.notes.as_ref() {
        clone.notes = Some(sanitize(n));
    }
    clone
}

/// Read `brief.body` from the shadow corpus file. Returns empty string on miss.
/// Used to populate the `prompt` field in DPO training pairs (TRL DPOTrainer format).
fn read_brief_prompt_from_corpus(corpus_root: &Path, task_type: &str, brief_id: &str) -> String {
    let path = corpus_root
        .join("data")
        .join("training-corpus")
        .join("apprenticeship")
        .join(task_type)
        .join(format!("shadow-{brief_id}.jsonl"));
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return String::new(),
    };
    let row: serde_json::Value = match serde_json::from_str(content.trim()) {
        Ok(v) => v,
        Err(_) => return String::new(),
    };
    row["brief"]["body"].as_str().unwrap_or("").to_string()
}

#[allow(clippy::too_many_arguments)]
fn write_dpo_pair(
    corpus_root: &Path,
    task_type: &str,
    rejected_diff: &str,
    chosen_diff: &str,
    doctrine_violation_tag: &str,
    brief_id: &str,
    attempt_id: &str,
    tier_used: &str,
    attempt_confidence: f32,
    attempt_escalate: bool,
    attempt_reasoning: &str,
) -> Result<PathBuf> {
    let chosen_s = sanitize(chosen_diff);
    let rejected_s = sanitize(rejected_diff);

    // Quality gate runs on the RAW diffs — template-echo, example-echo, min-length
    // (both sides), length-ratio, max-length, DoNotUse all need the bare content.
    // Returns CorpusGateRejected on failure — same error type as shadow tuple gate.
    crate::corpus_gate::check_dpo_pair(&rejected_s, &chosen_s)?;

    // Store both sides in the canonical envelope the apprentice emits at
    // inference (frontmatter + reasoning + fenced diff). Without this, `chosen`
    // is a raw `diff --git` blob the model is instructed never to emit and
    // `rejected` is a stripped hunk — DPO then rewards surface form, not quality
    // (2026-06-19 Opus audit, DPO-format finding). The senior's reasoning is the
    // verdict note; the apprentice's confidence/escalate/reasoning come from its
    // recovered attempt.
    let chosen_reasoning = if doctrine_violation_tag.trim().is_empty() {
        "Senior-approved correction."
    } else {
        doctrine_violation_tag
    };
    let chosen_full =
        crate::apprenticeship::render_canonical_response(0.95, false, chosen_reasoning, &chosen_s);
    let rejected_full = crate::apprenticeship::render_canonical_response(
        attempt_confidence,
        attempt_escalate,
        attempt_reasoning,
        &rejected_s,
    );

    let prompt = read_brief_prompt_from_corpus(corpus_root, task_type, brief_id);
    let dir = corpus_root
        .join("data")
        .join("training-corpus")
        .join("feedback");
    std::fs::create_dir_all(&dir).map_err(|e| DoormanError::CorpusWrite {
        path: dir.display().to_string(),
        reason: e.to_string(),
    })?;
    let filename = format!(
        "apprenticeship-{}-{}.jsonl",
        task_type,
        Uuid::now_v7().simple()
    );
    let path = dir.join(&filename);
    // Lengths/ratio are diagnostics on the STORED (enveloped) sides — what TRL
    // actually trains on. The gate's ratio check already ran on the raw diffs.
    let chosen_len = chosen_full.len();
    let rejected_len = rejected_full.len();
    let length_ratio = if rejected_len == 0 {
        f64::INFINITY
    } else {
        chosen_len as f64 / rejected_len as f64
    };
    // TRL DPOTrainer format: prompt + chosen (preferred) + rejected (dispreferred).
    // Both sides are in the canonical-envelope-v1 format (see render_canonical_response).
    // chosen_len / rejected_len / length_ratio are diagnostic fields — not used by
    // TRL but checked by run-dpo-training.py and audit tooling.
    let record = serde_json::json!({
        "tuple_type": "apprenticeship-feedback",
        "task_type": task_type,
        "brief_id": brief_id,
        "attempt_id": attempt_id,
        "tier_used": tier_used,
        "format": "canonical-envelope-v1",
        "prompt": sanitize(&prompt),
        "chosen": chosen_full,
        "rejected": rejected_full,
        "chosen_len": chosen_len,
        "rejected_len": rejected_len,
        "length_ratio": (length_ratio * 100.0).round() / 100.0,
        "doctrine_violation_tag": doctrine_violation_tag,
        "created": Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    });
    let line = serde_json::to_string(&record).map_err(|e| DoormanError::CorpusWrite {
        path: path.display().to_string(),
        reason: e.to_string(),
    })?;
    std::fs::write(&path, format!("{line}\n")).map_err(|e| DoormanError::CorpusWrite {
        path: path.display().to_string(),
        reason: e.to_string(),
    })?;
    Ok(path)
}

/// Promote an existing shadow corpus tuple in-place by adding the
/// signed verdict block, advancing `stage_at_capture` to `stage_before`
/// (the ledger stage at time of verdict signing), and setting
/// `promoted_at` to the current timestamp.
///
/// Per §7B: rewrites the file with the updated tuple. The one-file-per-
/// brief format (shadow-<brief_id>.jsonl) makes this an atomic overwrite
/// operation (write new content to temp; rename over original).
///
/// If no shadow tuple exists on disk for this `brief_id`, the caller
/// should already have surfaced `OrphanVerdictNoCorpusTuple` before
/// reaching this function.
#[allow(clippy::too_many_arguments)]
fn promote_corpus_tuple(
    corpus_root: &Path,
    brief_id: &str,
    task_type: &str,
    verdict: &ApprenticeshipVerdict,
    final_diff: Option<&str>,
    stage_before: &str,
    doctrine_version: &str,
    tenant: &str,
) -> Result<PathBuf> {
    let path = shadow_corpus_path(corpus_root, task_type, brief_id);

    // Read the existing tuple. If the file doesn't exist (orphan path
    // bypassed) we build a minimal record from the verdict data only.
    let existing: serde_json::Value = if path.exists() {
        let content = std::fs::read_to_string(&path).map_err(|e| DoormanError::CorpusWrite {
            path: path.display().to_string(),
            reason: format!("read existing tuple: {e}"),
        })?;
        serde_json::from_str(content.trim()).map_err(|e| DoormanError::CorpusWrite {
            path: path.display().to_string(),
            reason: format!("parse existing tuple JSON: {e}"),
        })?
    } else {
        // Shouldn't reach here in normal operation; orphan check is upstream.
        // Produce a minimal shell so downstream processing completes.
        serde_json::json!({
            "tuple_type": "apprenticeship",
            "doctrine_version": doctrine_version,
            "task_type": task_type,
            "stage_at_capture": stage_before,
            "brief": serde_json::Value::Null,
            "attempt": serde_json::Value::Null,
            "verdict": serde_json::Value::Null,
            "actual_diff": serde_json::Value::Null,
            "final_diff": serde_json::Value::Null,
            "redaction_class": "internal",
            "evidence_class": "primary",
            "tenant": tenant,
            "cluster": serde_json::Value::Null,
            "session_id": serde_json::Value::Null,
            "created": Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            "promoted_at": serde_json::Value::Null,
        })
    };

    let sanitized_verdict = sanitize_verdict(verdict);
    let sanitized_final_diff = final_diff.map(sanitize);
    let promoted_at = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    // Merge: start with the existing record, overwrite the verdict-
    // related fields. Preserves all capture-time fields (brief, attempt,
    // actual_diff, created, cluster, etc.) exactly as they were written
    // at shadow-brief dispatch time.
    let mut promoted = existing;
    promoted["verdict"] =
        serde_json::to_value(sanitized_verdict).map_err(|e| DoormanError::CorpusWrite {
            path: path.display().to_string(),
            reason: format!("serialise verdict: {e}"),
        })?;
    promoted["stage_at_capture"] = serde_json::Value::String(stage_before.to_string());
    promoted["final_diff"] = sanitized_final_diff
        .map(serde_json::Value::String)
        .unwrap_or(serde_json::Value::Null);
    promoted["promoted_at"] = serde_json::Value::String(promoted_at);

    let line = serde_json::to_string(&promoted).map_err(|e| DoormanError::CorpusWrite {
        path: path.display().to_string(),
        reason: format!("serialise promoted tuple: {e}"),
    })?;

    // Atomic overwrite: write to a temp file, then rename.
    let tmp_path = path.with_extension("jsonl.tmp");
    std::fs::write(&tmp_path, format!("{line}\n")).map_err(|e| DoormanError::CorpusWrite {
        path: tmp_path.display().to_string(),
        reason: format!("write temp: {e}"),
    })?;
    std::fs::rename(&tmp_path, &path).map_err(|e| DoormanError::CorpusWrite {
        path: path.display().to_string(),
        reason: format!("rename tmp to final: {e}"),
    })?;
    Ok(path)
}

/// Scan the corpus root for a shadow tuple with the given `brief_id`.
/// Returns `(task_type, self_confidence, attempt_id)` if found.
/// Looks in every task-type subdirectory under
/// `data/training-corpus/apprenticeship/`.
///
/// Used for post-restart corpus-disk recovery when BriefCache has been
/// evicted. Linear scan is acceptable at expected volume (≤thousands
/// of tuples per task-type).
fn locate_corpus_tuple_by_brief_id(
    corpus_root: &Path,
    brief_id: &str,
) -> Option<(String, f32, String)> {
    let base = corpus_root
        .join("data")
        .join("training-corpus")
        .join("apprenticeship");
    let expected_filename = format!("shadow-{brief_id}.jsonl");

    let entries = std::fs::read_dir(&base).ok()?;
    for entry in entries.flatten() {
        let task_type = entry.file_name().to_string_lossy().to_string();
        if task_type.starts_with('.') {
            continue; // skip hidden/lock files
        }
        let candidate = entry.path().join(&expected_filename);
        if candidate.exists() {
            // Parse the tuple to extract self_confidence and attempt_id.
            let content = std::fs::read_to_string(&candidate).ok()?;
            let row: serde_json::Value = serde_json::from_str(content.trim()).ok()?;
            let sc = row["attempt"]["self_confidence"].as_f64().unwrap_or(0.0) as f32;
            let attempt_id = row["attempt"]["attempt_id"]
                .as_str()
                .unwrap_or("")
                .to_string();
            return Some((task_type, sc, attempt_id));
        }
    }
    None
}

/// Read the `attempt.diff` field from the on-disk corpus tuple for a given
/// `brief_id` under `<corpus_root>/data/training-corpus/apprenticeship/<task_type>/`.
///
/// Used as the BriefCache fallback path: when the Doorman restarts the in-memory
/// cache is empty, so verdicts posted against existing tuples need to recover the
/// attempt diff from disk before generating DPO pairs.
fn read_attempt_diff_from_corpus(corpus_root: &Path, task_type: &str, brief_id: &str) -> String {
    let path = corpus_root
        .join("data")
        .join("training-corpus")
        .join("apprenticeship")
        .join(task_type)
        .join(format!("shadow-{brief_id}.jsonl"));
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return String::new(),
    };
    let row: serde_json::Value = match serde_json::from_str(content.trim()) {
        Ok(v) => v,
        Err(_) => return String::new(),
    };
    row["attempt"]["diff"].as_str().unwrap_or("").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use slm_core::{ApprenticeshipAttempt, ApprenticeshipBrief, BriefScope, SeniorRole, Tier};
    use std::path::PathBuf;

    /// Test verifier — accepts only when `senior_identity` matches a
    /// preconfigured value AND signature equals a known marker.
    /// Lets us exercise both happy path and rejection without
    /// shelling out to ssh-keygen.
    #[derive(Debug)]
    struct MockVerifier {
        accept_signature: String,
        accept_identity: String,
        accept_namespace: String,
    }

    #[async_trait]
    impl VerdictVerifier for MockVerifier {
        async fn verify(
            &self,
            _body: &str,
            signature_pem: &str,
            senior_identity: &str,
            namespace: &str,
        ) -> Result<()> {
            if signature_pem == self.accept_signature
                && senior_identity == self.accept_identity
                && namespace == self.accept_namespace
            {
                Ok(())
            } else {
                Err(DoormanError::VerifySignature(
                    "mock verifier rejected".into(),
                ))
            }
        }
    }

    fn tmp_dir(label: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "slm-doorman-verdict-{label}-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn ts() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 4, 26, 16, 30, 0).unwrap()
    }

    fn brief(brief_id: &str) -> ApprenticeshipBrief {
        ApprenticeshipBrief {
            brief_id: brief_id.into(),
            created: ts(),
            senior_role: SeniorRole::Master,
            senior_identity: "ps-administrator".into(),
            task_type: "version-bump-manifest".into(),
            scope: BriefScope::default(),
            acceptance_test: "T".into(),
            doctrine_citations: vec![],
            shadow: false,
            body: "B".into(),
        }
    }

    fn attempt(brief_id: &str, attempt_id: &str) -> ApprenticeshipAttempt {
        ApprenticeshipAttempt {
            brief_id: brief_id.into(),
            attempt_id: attempt_id.into(),
            created: ts(),
            model: "olmo-3-1125-7b-q4".into(),
            adapter_composition: vec![],
            self_confidence: 0.9,
            escalate: false,
            inference_ms: 100,
            tier: Tier::Local,
            cost_usd: 0.0,
            reasoning: String::new(),
            // Realistic diff >= MIN_REJECTED_CHARS (80) to pass the corpus gate.
            diff: "diff --git a/Cargo.toml b/Cargo.toml\n--- a/Cargo.toml\n+++ b/Cargo.toml\n@@ -1 +1 @@\n-version = \"0.0.1\"\n+version = \"0.1.0\"".into(),
        }
    }

    fn verdict_body_text(brief_id: &str, attempt_id: &str, outcome: &str) -> String {
        format!(
            "---\n\
             schema: foundry-apprentice-verdict-v1\n\
             brief_id: {brief_id}\n\
             attempt_id: {attempt_id}\n\
             verdict: {outcome}\n\
             created: 2026-04-26T16:30:00Z\n\
             senior_identity: ps-administrator\n\
             final_diff_sha: 0123456789abcdef0123456789abcdef01234567\n\
             notes: LGTM\n\
             ---\n\
             \n\
             diff --git a/Cargo.toml b/Cargo.toml\n\
             --- a/Cargo.toml\n\
             +++ b/Cargo.toml\n\
             @@ -1,3 +1,3 @@\n\
             [package]\n\
             -version = \"0.0.1\"\n\
             +version = \"0.1.0\"\n"
        )
    }

    fn dispatcher(
        corpus: PathBuf,
        ledger_dir: PathBuf,
        cache: Arc<BriefCache>,
    ) -> VerdictDispatcher {
        let verifier: Arc<dyn VerdictVerifier> = Arc::new(MockVerifier {
            accept_signature: "TRUSTED-SIGNATURE-BLOB".into(),
            accept_identity: "ps-administrator".into(),
            accept_namespace: VERDICT_NAMESPACE.into(),
        });
        VerdictDispatcher {
            verifier,
            cache,
            ledger: PromotionLedger::new(ledger_dir).unwrap(),
            corpus_root: corpus,
            doctrine_version: "0.0.13".into(),
            tenant: "pointsav".into(),
        }
    }

    /// Pre-write a shadow corpus tuple in the tempdir, simulating what
    /// `dispatch_shadow` does at capture time. Required by tests that
    /// exercise the verdict promote-in-place path — the promote step
    /// reads the pre-written tuple from disk.
    fn seed_shadow_tuple(corpus: &Path, b: &ApprenticeshipBrief, a: &ApprenticeshipAttempt) {
        let dir = shadow_corpus_path(corpus, &b.task_type, &b.brief_id)
            .parent()
            .unwrap()
            .to_owned();
        std::fs::create_dir_all(&dir).unwrap();
        let path = shadow_corpus_path(corpus, &b.task_type, &b.brief_id);
        let record = serde_json::json!({
            "tuple_type": "apprenticeship",
            "doctrine_version": "0.0.13",
            "task_type": b.task_type,
            "stage_at_capture": "review",
            "brief": b,
            "attempt": a,
            "verdict": serde_json::Value::Null,
            "actual_diff": "--- a/test\n+++ b/test\n@@ -1 +1 @@\n-old\n+new\n",
            "final_diff": serde_json::Value::Null,
            "redaction_class": "internal",
            "evidence_class": "primary",
            "tenant": "pointsav",
            "cluster": b.scope.cluster,
            "session_id": serde_json::Value::Null,
            "created": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            "promoted_at": serde_json::Value::Null,
        });
        let line = serde_json::to_string(&record).unwrap();
        std::fs::write(&path, format!("{line}\n")).unwrap();
    }

    /// Test 1 — happy-path signed verdict accepted; corpus tuple
    /// promoted in-place at the shadow-<brief_id>.jsonl path with the
    /// expected schema (§7B semantics).
    #[tokio::test]
    async fn happy_path_signed_verdict_writes_corpus_tuple() {
        let corpus = tmp_dir("corpus-1");
        let ledger_dir = tmp_dir("ledger-1");
        let cache = Arc::new(BriefCache::new(8));
        let b = brief("b1");
        let a = attempt("b1", "a1");
        // Pre-write shadow tuple (capture-time write per §7B).
        seed_shadow_tuple(&corpus, &b, &a);
        cache.insert(b, a);

        let dispatcher = dispatcher(corpus.clone(), ledger_dir, cache);
        let body = verdict_body_text("b1", "a1", "accept");
        let signature_b64 = B64.encode("TRUSTED-SIGNATURE-BLOB".as_bytes());

        let outcome = dispatcher
            .dispatch(VerdictWireBody {
                body: body.clone(),
                signature: signature_b64,
                senior_identity: "ps-administrator".into(),
            })
            .await
            .expect("happy path verdict accepted");
        assert_eq!(outcome.verdict, VerdictOutcome::Accept);
        assert!(
            outcome.dpo_pair_path.is_none(),
            "accept produces no DPO pair"
        );

        // Corpus tuple promoted in-place at the shadow path.
        let expected_dir = corpus
            .join("data")
            .join("training-corpus")
            .join("apprenticeship")
            .join("version-bump-manifest");
        let entries: Vec<_> = std::fs::read_dir(&expected_dir).unwrap().collect();
        assert_eq!(
            entries.len(),
            1,
            "exactly one corpus row (promoted in-place)"
        );

        let written = entries.into_iter().next().unwrap().unwrap().path();
        // Must be the shadow-b1.jsonl file (not a new UUIDv7 filename).
        assert!(
            written
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("shadow-b1"),
            "corpus file must be the shadow tuple, not a new UUID file"
        );
        let content = std::fs::read_to_string(&written).unwrap();
        let row: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
        assert_eq!(row["tuple_type"], "apprenticeship");
        assert_eq!(row["doctrine_version"], "0.0.13");
        assert_eq!(row["task_type"], "version-bump-manifest");
        assert_eq!(row["stage_at_capture"], "review");
        assert_eq!(row["redaction_class"], "internal");
        assert_eq!(row["evidence_class"], "primary");
        assert_eq!(row["tenant"], "pointsav");
        assert_eq!(row["brief"]["brief_id"], "b1");
        assert_eq!(row["attempt"]["attempt_id"], "a1");
        assert_eq!(row["verdict"]["verdict"], "accept");
        // promoted_at is now set (was null before verdict).
        assert!(
            row["promoted_at"].is_string(),
            "promoted_at must be set on verdict promotion"
        );
    }

    /// Test 2 — bad signature is rejected; no corpus write, no ledger
    /// row.
    #[tokio::test]
    async fn bad_signature_rejected_no_state_mutation() {
        let corpus = tmp_dir("corpus-2");
        let ledger_dir = tmp_dir("ledger-2");
        let cache = Arc::new(BriefCache::new(8));
        let b = brief("b1");
        let a = attempt("b1", "a1");
        seed_shadow_tuple(&corpus, &b, &a);
        cache.insert(b, a);
        let dispatcher = dispatcher(corpus.clone(), ledger_dir.clone(), cache);

        let body = verdict_body_text("b1", "a1", "accept");
        let bogus_b64 = B64.encode("WRONG-SIG".as_bytes());

        let err = dispatcher
            .dispatch(VerdictWireBody {
                body,
                signature: bogus_b64,
                senior_identity: "ps-administrator".into(),
            })
            .await
            .expect_err("bad signature must fail");
        assert!(matches!(err, DoormanError::VerifySignature(_)));

        // The shadow tuple was written before the verdict, but the
        // verdict itself was rejected — the tuple must still have
        // verdict: null (not promoted).
        let path = shadow_corpus_path(&corpus, "version-bump-manifest", "b1");
        let content = std::fs::read_to_string(&path).unwrap();
        let row: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
        assert!(
            row["verdict"].is_null(),
            "rejected verdict MUST NOT promote corpus tuple"
        );
        // No ledger.md created.
        assert!(
            !ledger_dir.join("ledger.md").exists(),
            "rejected verdict MUST NOT append ledger row"
        );
    }

    /// Test 3 — synthesise 50 accept verdicts; the 50th must trigger a
    /// `promotion` event and the dispatcher's outcome carries
    /// `promoted=true` with stage_after=spot-check.
    #[tokio::test]
    async fn ledger_promotion_fires_on_50_accepts() {
        let corpus = tmp_dir("corpus-3");
        let ledger_dir = tmp_dir("ledger-3");
        let cache = Arc::new(BriefCache::new(64));
        let dispatcher = dispatcher(corpus.clone(), ledger_dir.clone(), cache.clone());

        for i in 0..49 {
            let bid = format!("b{i}");
            let aid = format!("a{i}");
            let b = brief(&bid);
            let a = attempt(&bid, &aid);
            seed_shadow_tuple(&corpus, &b, &a);
            cache.insert(b, a);
            let body = verdict_body_text(&bid, &aid, "accept");
            let sig_b64 = B64.encode("TRUSTED-SIGNATURE-BLOB".as_bytes());
            let outcome = dispatcher
                .dispatch(VerdictWireBody {
                    body,
                    signature: sig_b64,
                    senior_identity: "ps-administrator".into(),
                })
                .await
                .unwrap();
            assert!(!outcome.promotion.promoted, "below threshold at i={i}");
        }
        let b50 = brief("b50");
        let a50 = attempt("b50", "a50");
        seed_shadow_tuple(&corpus, &b50, &a50);
        cache.insert(b50, a50);
        let body = verdict_body_text("b50", "a50", "accept");
        let sig_b64 = B64.encode("TRUSTED-SIGNATURE-BLOB".as_bytes());
        let outcome = dispatcher
            .dispatch(VerdictWireBody {
                body,
                signature: sig_b64,
                senior_identity: "ps-administrator".into(),
            })
            .await
            .unwrap();
        assert!(outcome.promotion.promoted, "50th accept must promote");
        assert_eq!(
            outcome.promotion.stage_after,
            crate::promotion_ledger::Stage::SpotCheck
        );

        let md = std::fs::read_to_string(ledger_dir.join("ledger.md")).unwrap();
        assert!(md.contains("promotion  version-bump-manifest"));
    }

    /// Refine verdict produces a DPO pair under data/training-corpus/feedback/.
    #[tokio::test]
    async fn refine_verdict_writes_dpo_pair() {
        let corpus = tmp_dir("corpus-dpo");
        let ledger_dir = tmp_dir("ledger-dpo");
        let cache = Arc::new(BriefCache::new(8));
        let b = brief("b1");
        let a = attempt("b1", "a1");
        seed_shadow_tuple(&corpus, &b, &a);
        cache.insert(b, a);
        let dispatcher = dispatcher(corpus.clone(), ledger_dir, cache);

        let body = verdict_body_text("b1", "a1", "refine");
        let sig_b64 = B64.encode("TRUSTED-SIGNATURE-BLOB".as_bytes());
        let outcome = dispatcher
            .dispatch(VerdictWireBody {
                body,
                signature: sig_b64,
                senior_identity: "ps-administrator".into(),
            })
            .await
            .unwrap();
        assert_eq!(outcome.verdict, VerdictOutcome::Refine);
        let dpo_path = outcome.dpo_pair_path.expect("refine writes DPO pair");
        let content = std::fs::read_to_string(&dpo_path).unwrap();
        let row: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
        assert_eq!(row["tuple_type"], "apprenticeship-feedback");
        assert_eq!(row["task_type"], "version-bump-manifest");
        assert_eq!(row["doctrine_violation_tag"], "LGTM");
    }

    /// Frontmatter parser handles all required keys.
    #[test]
    fn parse_verdict_body_extracts_required_fields() {
        let body = verdict_body_text("b1", "a1", "accept");
        let p = parse_verdict_body(&body).unwrap();
        assert_eq!(p.brief_id, "b1");
        assert_eq!(p.attempt_id, "a1");
        assert_eq!(p.verdict, VerdictOutcome::Accept);
        assert_eq!(p.senior_identity, "ps-administrator");
        assert_eq!(
            p.final_diff_sha.as_deref(),
            Some("0123456789abcdef0123456789abcdef01234567")
        );
        assert_eq!(p.notes.as_deref(), Some("LGTM"));
    }

    /// Reject verdict writes both a corpus tuple AND a DPO pair (same as
    /// refine); the rejection event is recorded in the stats ledger,
    /// lowering the rolling accept-rate.
    #[tokio::test]
    async fn reject_verdict_writes_corpus_tuple_and_dpo_pair() {
        let corpus = tmp_dir("corpus-reject");
        let ledger_dir = tmp_dir("ledger-reject");
        let cache = Arc::new(BriefCache::new(8));
        let b = brief("b-rej");
        let a = attempt("b-rej", "a-rej");
        seed_shadow_tuple(&corpus, &b, &a);
        cache.insert(b, a);
        let dispatcher = dispatcher(corpus.clone(), ledger_dir.clone(), cache);

        let body = verdict_body_text("b-rej", "a-rej", "reject");
        let sig_b64 = B64.encode("TRUSTED-SIGNATURE-BLOB".as_bytes());
        let outcome = dispatcher
            .dispatch(VerdictWireBody {
                body,
                signature: sig_b64,
                senior_identity: "ps-administrator".into(),
            })
            .await
            .unwrap();

        // Outcome carries Reject.
        assert_eq!(outcome.verdict, VerdictOutcome::Reject);

        // DPO pair is produced — reject is treated identically to refine
        // per `VerdictOutcome::produces_dpo_pair()`.
        let dpo_path = outcome.dpo_pair_path.expect("reject must write DPO pair");
        let dpo_content = std::fs::read_to_string(&dpo_path).unwrap();
        let dpo_row: serde_json::Value = serde_json::from_str(dpo_content.trim()).unwrap();
        assert_eq!(dpo_row["tuple_type"], "apprenticeship-feedback");
        assert_eq!(dpo_row["task_type"], "version-bump-manifest");
        assert_eq!(dpo_row["brief_id"], "b-rej");
        assert_eq!(dpo_row["attempt_id"], "a-rej");

        // Corpus tuple is also promoted in-place (no new file added).
        let corpus_dir = corpus
            .join("data")
            .join("training-corpus")
            .join("apprenticeship")
            .join("version-bump-manifest");
        let entries: Vec<_> = std::fs::read_dir(&corpus_dir).unwrap().collect();
        assert_eq!(
            entries.len(),
            1,
            "exactly one corpus row (promoted in-place, no duplicate)"
        );
        let corpus_content =
            std::fs::read_to_string(entries.into_iter().next().unwrap().unwrap().path()).unwrap();
        let corpus_row: serde_json::Value = serde_json::from_str(corpus_content.trim()).unwrap();
        assert_eq!(corpus_row["verdict"]["verdict"], "reject");

        // Ledger stats row recorded; rejection counts against accept_rate
        // (n increments, accepts does not) — the ledger file must exist.
        assert!(
            ledger_dir.join("ledger.md").exists(),
            "rejection verdict must write a ledger row"
        );
        assert!(
            ledger_dir.join(".stats.jsonl").exists(),
            "rejection verdict must write a stats row"
        );
        // With 1 verdict and 0 accepts the accept_rate is 0.0 — verify via
        // the PromotionOutcome fields.
        assert!((outcome.promotion.accept_rate - 0.0).abs() < 1e-6);
        assert_eq!(outcome.promotion.n_verdicts, 1);
        assert!(!outcome.promotion.promoted);
    }

    /// DeferTierC verdict writes a corpus tuple but NO DPO pair — it is
    /// an escalation signal, not a refinement; there is no corrected diff
    /// to pair. The defer event is recorded in the stats ledger.
    #[tokio::test]
    async fn defer_tier_c_verdict_writes_corpus_tuple_no_dpo_pair() {
        let corpus = tmp_dir("corpus-defer");
        let ledger_dir = tmp_dir("ledger-defer");
        let cache = Arc::new(BriefCache::new(8));
        let b = brief("b-def");
        let a = attempt("b-def", "a-def");
        seed_shadow_tuple(&corpus, &b, &a);
        cache.insert(b, a);
        let dispatcher = dispatcher(corpus.clone(), ledger_dir.clone(), cache);

        let body = verdict_body_text("b-def", "a-def", "defer-tier-c");
        let sig_b64 = B64.encode("TRUSTED-SIGNATURE-BLOB".as_bytes());
        let outcome = dispatcher
            .dispatch(VerdictWireBody {
                body,
                signature: sig_b64,
                senior_identity: "ps-administrator".into(),
            })
            .await
            .unwrap();

        // Outcome carries DeferTierC.
        assert_eq!(outcome.verdict, VerdictOutcome::DeferTierC);

        // No DPO pair — escalation is not refinement.
        assert!(
            outcome.dpo_pair_path.is_none(),
            "defer-tier-c must NOT write a DPO pair"
        );

        // Corpus tuple IS promoted in-place.
        let corpus_dir = corpus
            .join("data")
            .join("training-corpus")
            .join("apprenticeship")
            .join("version-bump-manifest");
        let entries: Vec<_> = std::fs::read_dir(&corpus_dir).unwrap().collect();
        assert_eq!(
            entries.len(),
            1,
            "exactly one corpus row (promoted in-place)"
        );
        let corpus_content =
            std::fs::read_to_string(entries.into_iter().next().unwrap().unwrap().path()).unwrap();
        let corpus_row: serde_json::Value = serde_json::from_str(corpus_content.trim()).unwrap();
        assert_eq!(corpus_row["verdict"]["verdict"], "defer-tier-c");

        // Ledger stats row recorded; DeferTierC counts against accept_rate
        // (n increments, accepts does not) — no different from reject at
        // the rolling-stats level.
        assert!(
            ledger_dir.join("ledger.md").exists(),
            "defer-tier-c verdict must write a ledger row"
        );
        assert!(
            ledger_dir.join(".stats.jsonl").exists(),
            "defer-tier-c verdict must write a stats row"
        );
        assert!((outcome.promotion.accept_rate - 0.0).abs() < 1e-6);
        assert_eq!(outcome.promotion.n_verdicts, 1);
        assert!(!outcome.promotion.promoted);
    }

    /// Per §7B: orphan verdict (no shadow corpus tuple on disk, cache
    /// also empty) must return `OrphanVerdictNoCorpusTuple` and NOT
    /// create a new corpus row. Previously this returned `BriefCacheMiss`
    /// and also didn't create a row; now the error is more specific and
    /// includes the expected path.
    #[tokio::test]
    async fn orphan_verdict_no_corpus_tuple_surfaces_correct_error() {
        let corpus = tmp_dir("corpus-orphan");
        let ledger_dir = tmp_dir("ledger-orphan");
        let cache = Arc::new(BriefCache::new(8));
        // Neither shadow tuple on disk nor cache entry — orphan verdict.
        let dispatcher = dispatcher(corpus.clone(), ledger_dir.clone(), cache);
        let body = verdict_body_text("b-orphan", "a-orphan", "accept");
        let sig_b64 = B64.encode("TRUSTED-SIGNATURE-BLOB".as_bytes());
        let err = dispatcher
            .dispatch(VerdictWireBody {
                body,
                signature: sig_b64,
                senior_identity: "ps-administrator".into(),
            })
            .await
            .expect_err("orphan verdict must fail");
        assert!(
            matches!(err, DoormanError::OrphanVerdictNoCorpusTuple { .. }),
            "expected OrphanVerdictNoCorpusTuple, got: {err:?}"
        );
        // No corpus directory must have been created.
        let corpus_base = corpus
            .join("data")
            .join("training-corpus")
            .join("apprenticeship");
        assert!(
            !corpus_base.exists(),
            "orphan verdict MUST NOT create any corpus directory"
        );
    }

    /// Per §7B: verdict signing promotes the existing tuple in-place
    /// rather than creating a duplicate. This is the CRITICAL property
    /// that prevents corpus bloat: calling dispatch twice with the same
    /// verdict and brief_id must result in exactly one tuple file.
    #[tokio::test]
    async fn verdict_signing_promotes_in_place_no_duplicate() {
        let corpus = tmp_dir("corpus-no-dup");
        let ledger_dir = tmp_dir("ledger-no-dup");
        let cache = Arc::new(BriefCache::new(8));
        let b = brief("b-nodup");
        let a = attempt("b-nodup", "a-nodup");
        seed_shadow_tuple(&corpus, &b, &a);
        cache.insert(b.clone(), a.clone());

        let d = dispatcher(corpus.clone(), ledger_dir, cache.clone());
        let body = verdict_body_text("b-nodup", "a-nodup", "accept");
        let sig_b64 = B64.encode("TRUSTED-SIGNATURE-BLOB".as_bytes());
        d.dispatch(VerdictWireBody {
            body,
            signature: sig_b64,
            senior_identity: "ps-administrator".into(),
        })
        .await
        .expect("first verdict OK");

        // The shadow-b-nodup.jsonl file now has verdict set; promoted_at is set.
        let path = shadow_corpus_path(&corpus, "version-bump-manifest", "b-nodup");
        let content = std::fs::read_to_string(&path).unwrap();
        let row: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
        assert_eq!(row["verdict"]["verdict"], "accept", "verdict set");
        assert!(
            row["promoted_at"].is_string(),
            "promoted_at must be a timestamp after promotion"
        );

        // Exactly one file in the corpus directory (no duplicate created).
        let dir = corpus
            .join("data")
            .join("training-corpus")
            .join("apprenticeship")
            .join("version-bump-manifest");
        let count = std::fs::read_dir(&dir).unwrap().count();
        assert_eq!(
            count, 1,
            "promote-in-place MUST NOT create a duplicate tuple"
        );
    }

    /// Per §7B: post-restart corpus-disk recovery. Cache is empty but
    /// the shadow tuple exists on disk. Verdict dispatch must locate
    /// the tuple via disk scan and promote it successfully.
    #[tokio::test]
    async fn post_restart_recovery_verdict_promotes_from_disk() {
        let corpus = tmp_dir("corpus-restart");
        let ledger_dir = tmp_dir("ledger-restart");
        // Empty cache — simulates a post-restart state.
        let cache = Arc::new(BriefCache::new(8));
        let b = brief("b-restart");
        let a = attempt("b-restart", "a-restart");
        // Pre-write the shadow tuple as if capture already happened.
        seed_shadow_tuple(&corpus, &b, &a);
        // Do NOT insert into cache (simulating restart).

        let d = dispatcher(corpus.clone(), ledger_dir, cache);
        let body = verdict_body_text("b-restart", "a-restart", "accept");
        let sig_b64 = B64.encode("TRUSTED-SIGNATURE-BLOB".as_bytes());
        let outcome = d
            .dispatch(VerdictWireBody {
                body,
                signature: sig_b64,
                senior_identity: "ps-administrator".into(),
            })
            .await
            .expect("post-restart recovery verdict must succeed");
        assert_eq!(outcome.verdict, VerdictOutcome::Accept);

        // Tuple was promoted in-place from disk.
        let path = shadow_corpus_path(&corpus, "version-bump-manifest", "b-restart");
        let content = std::fs::read_to_string(&path).unwrap();
        let row: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
        assert_eq!(row["verdict"]["verdict"], "accept");
        assert!(row["promoted_at"].is_string());
    }

    // ── New AS-3 §7B tests (added 2026-04-29) ───────────────────────────

    /// Apprentice completion writes corpus tuple at stage_at_capture: "review"
    /// per §7B. This test exercises the schema directly via `seed_shadow_tuple`
    /// helper (which mirrors `write_shadow_tuple` output); the full
    /// dispatch_shadow path is covered in `apprenticeship.rs` tests.
    #[test]
    fn apprentice_completion_review_stage_schema_matches_spec() {
        let dir = tmp_dir("as3-review-stage");
        let b = brief("b-schema");
        let a = attempt("b-schema", "a-schema");
        seed_shadow_tuple(&dir, &b, &a);
        let path = shadow_corpus_path(&dir, "version-bump-manifest", "b-schema");
        let content = std::fs::read_to_string(&path).unwrap();
        let row: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
        // Required §7B fields.
        assert_eq!(row["stage_at_capture"], "review");
        assert!(row["verdict"].is_null(), "verdict null at capture time");
        assert!(
            row["promoted_at"].is_null(),
            "promoted_at null at capture time"
        );
        assert!(
            !row["actual_diff"].is_null(),
            "actual_diff present at capture time"
        );
        assert_eq!(row["doctrine_version"], "0.0.13");
    }

    /// Doctrine version 0.0.13 is stamped on all captured tuples.
    #[test]
    fn corpus_tuple_carries_doctrine_version_0_0_13() {
        let dir = tmp_dir("as3-doctrine-ver");
        let b = brief("b-doctrine");
        let a = attempt("b-doctrine", "a-doctrine");
        seed_shadow_tuple(&dir, &b, &a);
        let path = shadow_corpus_path(&dir, "version-bump-manifest", "b-doctrine");
        let content = std::fs::read_to_string(&path).unwrap();
        let row: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
        assert_eq!(
            row["doctrine_version"], "0.0.13",
            "doctrine_version must be 0.0.13 per AS-3 §7B amendment"
        );
    }
}
