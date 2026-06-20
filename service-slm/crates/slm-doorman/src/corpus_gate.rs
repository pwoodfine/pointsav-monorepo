// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Corpus quality gate — the second-layer write-time gate.
//!
//! `queue::quality_gate_shadow` is the FIRST layer (runs at `/v1/shadow`
//! enqueue time, before a brief lands on disk). This module is the SECOND
//! layer — runs immediately before `apprenticeship::write_shadow_tuple`
//! and `verdict::write_dpo_pair` to catch:
//!
//! 1. **Duplicates** — same `(brief_hash, diff_hash)` already in the corpus
//!    (idempotency belt-and-braces beyond brief_id filename dedup).
//! 2. **Oversized diffs** — diffs above `MAX_DIFF_CHARS` (≈1000 LOC) carry
//!    poor signal and bloat the corpus.
//! 3. **BCSC posture violations** — references to "Sovereign Data
//!    Foundation" without forward-looking qualifiers (planned/intended/
//!    may/target) flag the tuple (`bcsc_flagged: true`); operator
//!    reviews before promotion. Flag-only — does not reject.
//! 4. **Do-Not-Use vocabulary** — terms from POINTSAV-Project-Instructions.md
//!    §5 reject the tuple outright. The placeholder regex set lives here
//!    until project-editorial ratifies a canonical list (outbox 2026-05-18
//!    pending).
//!
//! Phase 1 of `learning-loop-master-plan-2026-05-18.md` (P1-1.1).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{DoormanError, Result};

/// Maximum permitted `actual_diff` length. ≈1000 LOC at avg 50 chars/line.
/// Diffs above this cap are rejected — they bloat the corpus, are unlikely
/// to be useful single-shot LoRA training examples, and often indicate a
/// regenerated file or large rename that doesn't carry semantic signal.
pub const MAX_DIFF_CHARS: usize = 50_000;

/// Minimum rejected-side length for DPO pairs. Pairs where the rejected
/// response is shorter than this are template stubs or empty attempts that
/// teach the model "longer = better" rather than quality. 80 chars ≈ 1–2
/// sentences — the minimum for any meaningful code-review attempt.
pub const MIN_REJECTED_CHARS: usize = 80;

/// Maximum ratio of chosen length to rejected length for DPO pairs. When
/// chosen is more than 8× longer than rejected, the pair is degenerate:
/// DPO cannot learn preference signal from it — it learns token-count
/// discrimination instead (confirmed by Jun-14 training run; logps gap 6.7×).
pub const MAX_LENGTH_RATIO: f64 = 8.0;

/// Template-echo prefixes that indicate the OLMo attempt was never executed
/// and the rejected field contains a placeholder string, not a real attempt.
/// Pairs with these prefixes are rejected at write time.
///
/// `<unified diff:` (with colon) is the OLMo placeholder form — OLMo always
/// writes `<unified diff: --- a/...>` as a stub. A legitimately wrapped real
/// diff would be `<unified diff\ndiff --git ...` (no colon). Without the colon,
/// `check_dpo_pair()` falls through to the `REAL_DIFF_MARKERS` Rule B check.
const TEMPLATE_ECHO_PREFIXES: &[&str] = &[
    "<no diff provided",
    "<no changes",
    "<insert diff",
    "auto-reject: olmo-attempt-below-senior-standard",
    "auto-reject:",
    "<unified diff:",  // colon = OLMo stub; real wrapped diff omits the colon
];

/// If the rejected side starts with `<unified diff` (without colon) but ALSO
/// contains at least one of these markers, it holds a real diff. When none
/// of these markers appear, the prefix is a stub.
/// `<unified diff:` (with colon) is caught earlier as a TEMPLATE_ECHO_PREFIX.
const REAL_DIFF_MARKERS: &[&str] = &["diff --git", "--- a/", "+++ b/", "@@ "];

/// Verbatim tokens from the `APPRENTICE_SYSTEM_PROMPT` diff example. When a
/// rejected side contains two or more of these, OLMo copied the example rather
/// than producing a real diff — a stub that nonetheless carries `--- a/`/`@@`
/// markers and so slips past `REAL_DIFF_MARKERS`. Two-marker co-occurrence
/// keeps the false-positive rate near zero (a real diff touching a file
/// literally named `path/to/file` AND containing the lines `old line`/`new
/// line` is effectively impossible). Mirrors run-dpo-training.py.
const SYSTEM_PROMPT_EXAMPLE_MARKERS: &[&str] =
    &["path/to/file", "-old line", "+new line", " context line"];

/// Forward-looking-information qualifiers per BCSC posture
/// (`conventions/bcsc-disclosure-posture.md`). When "Sovereign Data
/// Foundation" appears without one of these markers in the same sentence,
/// the tuple is flagged (not rejected) so an operator reviews before the
/// tuple promotes into the trainable corpus.
const BCSC_FORWARD_LOOKING_MARKERS: &[&str] = &[
    "planned",
    "intended",
    "may ",
    "target",
    "anticipated",
    "expected",
    "forecast",
];

/// BCSC posture trigger terms. Co-occurrence with one of
/// `BCSC_FORWARD_LOOKING_MARKERS` in the same sentence is required; absence
/// flags the tuple. Co-evolution with `POINTSAV-Project-Instructions.md`
/// is project-editorial scope — when ratified, this list is replaced by
/// the canonical version.
const BCSC_TRIGGER_TERMS: &[&str] = &[
    "Sovereign Data Foundation",
    "the Foundation",
];

/// Placeholder Do-Not-Use term list (POINTSAV-Project-Instructions.md §5).
/// Match is case-insensitive. Project-editorial ratification of the
/// canonical regex set is pending (outbox 2026-05-18); this list is the
/// minimum-known subset extracted from the workspace cleanup-log and
/// recent BCSC violation fixes. **Reject** the tuple if any match.
///
/// When editorial publishes the canonical YAML/Lark artifact, swap this
/// constant for a runtime-loaded set keyed by version hash.
const DO_NOT_USE_TERMS: &[&str] = &[
    "sovereign telemetry",          // → "Verified System Telemetry"
    "cognitive forge",              // retired term per cleanup-log
    "cognitive-forge",
    "ai-first",                     // marketing vocab, Bloomberg violation
    "ai-powered",                   // marketing vocab
    "ai-driven",                    // marketing vocab
    "next-generation ai",           // marketing vocab
    "cutting-edge ai",              // marketing vocab
    "revolutionary",                // marketing vocab
    "game-changing",                // marketing vocab
    "groundbreaking ai",            // marketing vocab
];

/// Outcome of the corpus-gate check for a single tuple about to be written.
///
/// The caller (`write_shadow_tuple`, `write_dpo_pair`) embeds these fields
/// in the JSONL row so audit replay can verify the gate ran and which
/// scans flagged.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CorpusGateOutcome {
    /// SHA-256 of `brief.body` (hex, lowercase). Used for `(brief, diff)`
    /// dedup and audit cross-reference.
    pub brief_hash: String,
    /// SHA-256 of `actual_diff` (or DPO `corrected_diff`) (hex, lowercase).
    pub diff_hash: String,
    /// True when a BCSC trigger term appears without a forward-looking
    /// marker in the same sentence. **Flag only** — the tuple still
    /// writes; operator reviews before promotion. False when no trigger
    /// term is present OR when every trigger is properly qualified.
    pub bcsc_flagged: bool,
    /// Triggered BCSC sentences (verbatim, max 5 returned to bound JSONL
    /// row size). Empty when `bcsc_flagged: false`.
    pub bcsc_violations: Vec<String>,
}

/// Errors specific to the corpus gate. Wrap into `DoormanError::CorpusGateRejected`
/// so call sites surface a uniform error to the drain worker.
#[derive(Clone, Debug)]
pub enum CorpusGateReject {
    DuplicateTuple { brief_hash: String, diff_hash: String },
    DiffTooLarge { len: usize, max: usize },
    DoNotUseTerm { term: String, where_found: WhereFound },
    /// Rejected side is shorter than MIN_REJECTED_CHARS — likely a template
    /// stub or empty attempt; would teach the model "longer = better".
    RejectedTooShort { len: usize, min: usize },
    /// Chosen side is shorter than MIN_REJECTED_CHARS — an empty/near-empty
    /// chosen produces an INVERTED preference (the model learns to prefer no
    /// output). Symmetric guard to RejectedTooShort (2026-06-19 audit).
    ChosenTooShort { len: usize, min: usize },
    /// Rejected side contains a template-echo prefix indicating the attempt
    /// was never executed (e.g. the field contains a placeholder string).
    TemplateEchoRejected { prefix: String },
    /// Rejected side echoes the verbatim system-prompt diff EXAMPLE
    /// (`path/to/file` + `old line`/`new line` ...) rather than a real diff —
    /// the post-0506d359 failure mode that passes every other gate as a
    /// "real diff" because it contains `--- a/` / `@@` markers (2026-06-19 audit).
    SystemPromptExampleEcho { markers: usize },
    /// Chosen is more than MAX_LENGTH_RATIO × longer than rejected — DPO
    /// cannot distinguish quality from token count at this ratio.
    LengthRatioTooExtreme { chosen_len: usize, rejected_len: usize, ratio: f64, max: f64 },
}

#[derive(Clone, Copy, Debug)]
pub enum WhereFound {
    BriefBody,
    Diff,
}

impl std::fmt::Display for WhereFound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WhereFound::BriefBody => f.write_str("brief.body"),
            WhereFound::Diff => f.write_str("diff"),
        }
    }
}

impl From<CorpusGateReject> for DoormanError {
    fn from(r: CorpusGateReject) -> Self {
        let reason = match r {
            CorpusGateReject::DuplicateTuple { brief_hash, diff_hash } => format!(
                "duplicate tuple already in corpus (brief_hash={brief_hash}, diff_hash={diff_hash})"
            ),
            CorpusGateReject::DiffTooLarge { len, max } => format!(
                "diff too large ({len} chars > {max} max); diffs above the cap carry poor LoRA signal"
            ),
            CorpusGateReject::DoNotUseTerm { term, where_found } => format!(
                "Do-Not-Use term '{term}' detected in {where_found} (POINTSAV-Project-Instructions.md §5)"
            ),
            CorpusGateReject::RejectedTooShort { len, min } => format!(
                "rejected side too short ({len} chars < {min} min); template stub would teach length-discrimination"
            ),
            CorpusGateReject::ChosenTooShort { len, min } => format!(
                "chosen side too short ({len} chars < {min} min); empty/near-empty chosen creates an inverted preference"
            ),
            CorpusGateReject::TemplateEchoRejected { prefix } => format!(
                "rejected side is a template placeholder (starts with '{prefix}'); no real OLMo attempt captured"
            ),
            CorpusGateReject::SystemPromptExampleEcho { markers } => format!(
                "rejected side echoes the system-prompt diff example ({markers} marker tokens); not a real attempt"
            ),
            CorpusGateReject::LengthRatioTooExtreme { chosen_len, rejected_len, ratio, max } => format!(
                "DPO length ratio {ratio:.1}× exceeds {max:.1}× max (chosen={chosen_len} chars, rejected={rejected_len} chars); would teach token-count not quality"
            ),
        };
        DoormanError::CorpusGateRejected { reason }
    }
}

/// On-disk dedup index. One line per accepted tuple:
/// `{"brief_hash":"...","diff_hash":"...","brief_id":"...","written_at":"..."}`.
///
/// Loaded into memory on first use; appended to after every successful
/// gate pass. The file lives at
/// `data/training-corpus/.corpus-index.jsonl` relative to the corpus root.
pub struct CorpusIndex {
    path: PathBuf,
    seen: Mutex<HashSet<(String, String)>>,
}

impl CorpusIndex {
    /// Open or create the corpus index for a given corpus root
    /// (`<corpus_root>/data/training-corpus/.corpus-index.jsonl`).
    /// Reads any existing entries on disk into memory.
    pub fn open(corpus_root: &Path) -> Result<Self> {
        let dir = corpus_root.join("data").join("training-corpus");
        std::fs::create_dir_all(&dir).map_err(|e| DoormanError::CorpusWrite {
            path: dir.display().to_string(),
            reason: e.to_string(),
        })?;
        let path = dir.join(".corpus-index.jsonl");
        let mut seen = HashSet::new();
        if path.exists() {
            let body = std::fs::read_to_string(&path).map_err(|e| DoormanError::CorpusWrite {
                path: path.display().to_string(),
                reason: e.to_string(),
            })?;
            for line in body.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if let Ok(row) = serde_json::from_str::<CorpusIndexRow>(trimmed) {
                    seen.insert((row.brief_hash, row.diff_hash));
                }
                // Malformed rows are skipped silently — the index is a
                // best-effort dedup hint; the gate is the source of truth.
            }
        }
        Ok(Self {
            path,
            seen: Mutex::new(seen),
        })
    }

    /// Check the `(brief_hash, diff_hash)` pair against the index.
    /// - Returns `Ok(())` if the pair is new (and inserts it both in
    ///   memory and to the sidecar file).
    /// - Returns `Err(CorpusGateReject::DuplicateTuple)` if the pair is
    ///   already present.
    fn contains_or_insert(
        &self,
        brief_hash: &str,
        diff_hash: &str,
        brief_id: &str,
    ) -> std::result::Result<(), CorpusGateReject> {
        let mut guard = self.seen.lock().expect("corpus index mutex poisoned");
        let key = (brief_hash.to_string(), diff_hash.to_string());
        if guard.contains(&key) {
            return Err(CorpusGateReject::DuplicateTuple {
                brief_hash: brief_hash.to_string(),
                diff_hash: diff_hash.to_string(),
            });
        }
        // Append to sidecar; on failure we still insert in-memory so we
        // don't double-write within the same process, but we surface a
        // warn-level trace. The sidecar is a hint, not the source of truth.
        let row = CorpusIndexRow {
            brief_hash: brief_hash.to_string(),
            diff_hash: diff_hash.to_string(),
            brief_id: brief_id.to_string(),
            written_at: chrono::Utc::now()
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        };
        if let Ok(line) = serde_json::to_string(&row) {
            use std::io::Write as _;
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)
            {
                let _ = f.write_all(line.as_bytes());
                let _ = f.write_all(b"\n");
            }
        }
        guard.insert(key);
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CorpusIndexRow {
    brief_hash: String,
    diff_hash: String,
    brief_id: String,
    written_at: String,
}

/// Run the corpus gate against a (brief_body, diff, brief_id) tuple.
///
/// Returns `Ok(CorpusGateOutcome)` when the gate passes — caller embeds
/// the hashes + BCSC flag in the JSONL row.
///
/// Returns `Err(DoormanError::CorpusGateRejected)` when the gate rejects —
/// caller MUST NOT write the JSONL row.
pub fn check(
    index: &CorpusIndex,
    brief_id: &str,
    brief_body: &str,
    diff: &str,
) -> Result<CorpusGateOutcome> {
    // 1. Size cap on diff.
    if diff.len() > MAX_DIFF_CHARS {
        return Err(CorpusGateReject::DiffTooLarge {
            len: diff.len(),
            max: MAX_DIFF_CHARS,
        }
        .into());
    }

    // 2. Do-Not-Use term scan (case-insensitive). Reject on hit.
    let brief_lc = brief_body.to_lowercase();
    let diff_lc = diff.to_lowercase();
    for term in DO_NOT_USE_TERMS {
        if brief_lc.contains(term) {
            return Err(CorpusGateReject::DoNotUseTerm {
                term: (*term).to_string(),
                where_found: WhereFound::BriefBody,
            }
            .into());
        }
        if diff_lc.contains(term) {
            return Err(CorpusGateReject::DoNotUseTerm {
                term: (*term).to_string(),
                where_found: WhereFound::Diff,
            }
            .into());
        }
    }

    // 3. BCSC posture scan (flag-only).
    let bcsc_violations = scan_bcsc_violations(brief_body, diff);
    let bcsc_flagged = !bcsc_violations.is_empty();

    // 4. Dedup by (brief_hash, diff_hash).
    let brief_hash = sha256_hex(brief_body);
    let diff_hash = sha256_hex(diff);
    index.contains_or_insert(&brief_hash, &diff_hash, brief_id)?;

    Ok(CorpusGateOutcome {
        brief_hash,
        diff_hash,
        bcsc_flagged,
        bcsc_violations,
    })
}

/// Lighter-weight gate for DPO-pair writes (no dedup; just max-diff +
/// Do-Not-Use + BCSC scans on the corrected_diff). Use from
/// `verdict::write_dpo_pair` where dedup is not applicable (each DPO row
/// is keyed by `(brief_id, attempt_id, ulid)` — the ulid disambiguates).
pub fn scan_diff_only(diff: &str) -> Result<CorpusGateOutcome> {
    if diff.len() > MAX_DIFF_CHARS {
        return Err(CorpusGateReject::DiffTooLarge {
            len: diff.len(),
            max: MAX_DIFF_CHARS,
        }
        .into());
    }
    let diff_lc = diff.to_lowercase();
    for term in DO_NOT_USE_TERMS {
        if diff_lc.contains(term) {
            return Err(CorpusGateReject::DoNotUseTerm {
                term: (*term).to_string(),
                where_found: WhereFound::Diff,
            }
            .into());
        }
    }
    let bcsc_violations = scan_bcsc_violations("", diff);
    let bcsc_flagged = !bcsc_violations.is_empty();
    let diff_hash = sha256_hex(diff);
    Ok(CorpusGateOutcome {
        brief_hash: String::new(),
        diff_hash,
        bcsc_flagged,
        bcsc_violations,
    })
}

/// Full DPO-pair gate: runs all quality checks on both the rejected and chosen
/// sides of a preference pair before writing to disk.
///
/// Runs: template-echo detection, minimum-length check, length-ratio check,
/// max-length check on chosen, Do-Not-Use scan on both sides.
///
/// Returns `CorpusGateOutcome` on pass; `Err(DoormanError::CorpusGateRejected)`
/// on any hard failure.
pub fn check_dpo_pair(rejected: &str, chosen: &str) -> Result<CorpusGateOutcome> {
    // 1. Template-echo detection on the rejected side.
    // Rule A: hard prefix match on known sentinel strings.
    // Rule B: "<unified diff" prefix is only a placeholder when no real diff
    //   markers follow; OLMo legitimately wraps real diffs with that header.
    let rejected_lc = rejected.trim().to_lowercase();
    let mut echo_prefix: Option<&str> = None;
    for prefix in TEMPLATE_ECHO_PREFIXES {
        if rejected_lc.starts_with(prefix) {
            echo_prefix = Some(prefix);
            break;
        }
    }
    if echo_prefix.is_none() && rejected_lc.starts_with("<unified diff") {
        let has_real_diff = REAL_DIFF_MARKERS.iter().any(|m| rejected.contains(m));
        if !has_real_diff {
            echo_prefix = Some("<unified diff");
        }
    }
    if let Some(prefix) = echo_prefix {
        return Err(CorpusGateReject::TemplateEchoRejected {
            prefix: prefix.to_string(),
        }
        .into());
    }

    // 1b. System-prompt example echo: the rejected side copied the verbatim
    // diff example from APPRENTICE_SYSTEM_PROMPT. Caught by ≥2 marker
    // co-occurrence (a single marker could appear in a legitimate diff).
    let example_markers = SYSTEM_PROMPT_EXAMPLE_MARKERS
        .iter()
        .filter(|m| rejected.contains(*m))
        .count();
    if example_markers >= 2 {
        return Err(CorpusGateReject::SystemPromptExampleEcho {
            markers: example_markers,
        }
        .into());
    }

    // 2. Minimum length on rejected side.
    if rejected.len() < MIN_REJECTED_CHARS {
        return Err(CorpusGateReject::RejectedTooShort {
            len: rejected.len(),
            min: MIN_REJECTED_CHARS,
        }
        .into());
    }

    // 2b. Minimum length on CHOSEN side — symmetric guard. An empty or
    // near-empty chosen with a real rejected diff is an inverted pair that
    // teaches the model to prefer producing nothing.
    if chosen.len() < MIN_REJECTED_CHARS {
        return Err(CorpusGateReject::ChosenTooShort {
            len: chosen.len(),
            min: MIN_REJECTED_CHARS,
        }
        .into());
    }

    // 3. Length ratio: chosen must not be more than MAX_LENGTH_RATIO × rejected.
    if !rejected.is_empty() {
        let ratio = chosen.len() as f64 / rejected.len() as f64;
        if ratio > MAX_LENGTH_RATIO {
            return Err(CorpusGateReject::LengthRatioTooExtreme {
                chosen_len: chosen.len(),
                rejected_len: rejected.len(),
                ratio,
                max: MAX_LENGTH_RATIO,
            }
            .into());
        }
    }

    // 4. Max-length on chosen (same cap as shadow diffs).
    if chosen.len() > MAX_DIFF_CHARS {
        return Err(CorpusGateReject::DiffTooLarge {
            len: chosen.len(),
            max: MAX_DIFF_CHARS,
        }
        .into());
    }

    // 5. Do-Not-Use scan on both sides.
    let chosen_lc = chosen.to_lowercase();
    for term in DO_NOT_USE_TERMS {
        if rejected_lc.contains(term) {
            return Err(CorpusGateReject::DoNotUseTerm {
                term: (*term).to_string(),
                where_found: WhereFound::Diff,
            }
            .into());
        }
        if chosen_lc.contains(term) {
            return Err(CorpusGateReject::DoNotUseTerm {
                term: (*term).to_string(),
                where_found: WhereFound::Diff,
            }
            .into());
        }
    }

    // 6. BCSC posture scan (flag-only, does not reject).
    let bcsc_violations = scan_bcsc_violations(rejected, chosen);
    let bcsc_flagged = !bcsc_violations.is_empty();
    let diff_hash = sha256_hex(chosen);
    Ok(CorpusGateOutcome {
        brief_hash: String::new(),
        diff_hash,
        bcsc_flagged,
        bcsc_violations,
    })
}

fn sha256_hex(input: &str) -> String {
    let mut h = Sha256::new();
    h.update(input.as_bytes());
    let out = h.finalize();
    let mut s = String::with_capacity(out.len() * 2);
    for b in out.iter() {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Scan brief_body and diff for BCSC trigger terms; return any sentences
/// that contain a trigger without an accompanying forward-looking marker.
/// Up to 5 violations are returned (callers embed in JSONL; the cap bounds
/// row size).
fn scan_bcsc_violations(brief_body: &str, diff: &str) -> Vec<String> {
    let mut out = Vec::new();
    for text in [brief_body, diff] {
        for sentence in split_into_sentences(text) {
            let s_lc = sentence.to_lowercase();
            let has_trigger = BCSC_TRIGGER_TERMS
                .iter()
                .any(|t| s_lc.contains(&t.to_lowercase()));
            if !has_trigger {
                continue;
            }
            let has_marker = BCSC_FORWARD_LOOKING_MARKERS
                .iter()
                .any(|m| s_lc.contains(m));
            if !has_marker {
                out.push(sentence.trim().to_string());
                if out.len() >= 5 {
                    return out;
                }
            }
        }
    }
    out
}

/// Naive sentence splitter — splits on `.`, `!`, `?` followed by whitespace.
/// Good enough for BCSC flagging; precision is not critical (flag is for
/// operator review, not auto-reject).
fn split_into_sentences(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if (b == b'.' || b == b'!' || b == b'?') && i + 1 < bytes.len() {
            let next = bytes[i + 1];
            if next == b' ' || next == b'\n' || next == b'\t' {
                if i + 1 > start {
                    if let Some(slice) = text.get(start..=i) {
                        out.push(slice);
                    }
                }
                start = i + 1;
            }
        }
        i += 1;
    }
    if start < bytes.len() {
        if let Some(slice) = text.get(start..) {
            out.push(slice);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_root(label: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "slm-corpus-gate-{label}-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn passes_on_clean_tuple() {
        let root = tmp_root("clean");
        let index = CorpusIndex::open(&root).unwrap();
        let outcome = check(&index, "brief-1", "fix the cache invalidation on writes when the worker drains", "+ a line\n- another line\n").unwrap();
        assert!(!outcome.brief_hash.is_empty());
        assert!(!outcome.diff_hash.is_empty());
        assert!(!outcome.bcsc_flagged);
        assert!(outcome.bcsc_violations.is_empty());
    }

    #[test]
    fn rejects_duplicate_within_process() {
        let root = tmp_root("dup");
        let index = CorpusIndex::open(&root).unwrap();
        let brief = "fix the cache invalidation on writes when the worker drains";
        let diff = "+ a line\n";
        check(&index, "brief-1", brief, diff).unwrap();
        let err = check(&index, "brief-2", brief, diff).unwrap_err();
        assert!(matches!(err, DoormanError::CorpusGateRejected { .. }));
    }

    #[test]
    fn rejects_duplicate_across_index_reload() {
        let root = tmp_root("dup-reload");
        {
            let index = CorpusIndex::open(&root).unwrap();
            check(&index, "brief-1", "alpha bravo charlie delta echo foxtrot", "+ diff content\n").unwrap();
        }
        // Re-open: in-memory state is rebuilt from sidecar.
        let index = CorpusIndex::open(&root).unwrap();
        let err = check(&index, "brief-2", "alpha bravo charlie delta echo foxtrot", "+ diff content\n").unwrap_err();
        assert!(matches!(err, DoormanError::CorpusGateRejected { .. }));
    }

    #[test]
    fn rejects_oversized_diff() {
        let root = tmp_root("oversized");
        let index = CorpusIndex::open(&root).unwrap();
        let huge = "+ line\n".repeat(MAX_DIFF_CHARS / 7 + 100);
        let err = check(&index, "brief-1", "decent brief body with enough characters to pass", &huge).unwrap_err();
        assert!(matches!(err, DoormanError::CorpusGateRejected { .. }));
    }

    #[test]
    fn rejects_do_not_use_term_in_brief() {
        let root = tmp_root("dnu-brief");
        let index = CorpusIndex::open(&root).unwrap();
        let brief = "we are building a Sovereign Telemetry pipeline for fleet observability across the deployment substrate";
        let err = check(&index, "brief-1", brief, "+ ok diff\n").unwrap_err();
        assert!(matches!(err, DoormanError::CorpusGateRejected { .. }));
    }

    #[test]
    fn rejects_do_not_use_term_in_diff() {
        let root = tmp_root("dnu-diff");
        let index = CorpusIndex::open(&root).unwrap();
        let err = check(&index, "brief-1", "clean brief body with sufficient context describing the change", "+ // The cognitive forge subsystem starts here\n").unwrap_err();
        assert!(matches!(err, DoormanError::CorpusGateRejected { .. }));
    }

    #[test]
    fn flags_bcsc_violation_without_rejecting() {
        let root = tmp_root("bcsc-flag");
        let index = CorpusIndex::open(&root).unwrap();
        // Trigger without forward-looking marker.
        let outcome = check(
            &index,
            "brief-1",
            "the Sovereign Data Foundation is the active custodian of training corpus rights for this deployment",
            "+ unrelated diff line\n",
        ).unwrap();
        assert!(outcome.bcsc_flagged);
        assert!(!outcome.bcsc_violations.is_empty());
    }

    #[test]
    fn passes_bcsc_when_qualified() {
        let root = tmp_root("bcsc-ok");
        let index = CorpusIndex::open(&root).unwrap();
        // Trigger WITH forward-looking marker — no flag.
        let outcome = check(
            &index,
            "brief-1",
            "the Sovereign Data Foundation is the planned custodian of training corpus rights subject to future ratification",
            "+ unrelated diff line\n",
        ).unwrap();
        assert!(!outcome.bcsc_flagged);
        assert!(outcome.bcsc_violations.is_empty());
    }

    #[test]
    fn sha256_hex_is_stable() {
        assert_eq!(
            sha256_hex(""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    // --- check_dpo_pair tests ---

    fn decent_chosen() -> &'static str {
        "+ fn process_records(records: &[Record]) -> Result<Vec<Summary>> {\n\
         +     records.iter().map(|r| summarize(r)).collect()\n\
         + }\n"
    }

    fn decent_rejected() -> &'static str {
        "Here is a basic attempt at the function using an iterator approach to process the records.\n"
    }

    #[test]
    fn dpo_pair_passes_on_clean_pair() {
        let outcome = check_dpo_pair(decent_rejected(), decent_chosen()).unwrap();
        assert!(!outcome.diff_hash.is_empty());
    }

    #[test]
    fn dpo_pair_rejects_template_echo() {
        let err = check_dpo_pair("<unified diff placeholder text goes here>", decent_chosen())
            .unwrap_err();
        assert!(matches!(err, DoormanError::CorpusGateRejected { reason } if reason.contains("template placeholder")));
    }

    #[test]
    fn dpo_pair_rejects_auto_reject_prefix() {
        let err = check_dpo_pair(
            "auto-reject: olmo-attempt-below-senior-standard",
            decent_chosen(),
        )
        .unwrap_err();
        assert!(matches!(err, DoormanError::CorpusGateRejected { reason } if reason.contains("template placeholder")));
    }

    #[test]
    fn dpo_pair_rejects_short_rejected() {
        let err = check_dpo_pair("ok", decent_chosen()).unwrap_err();
        assert!(matches!(err, DoormanError::CorpusGateRejected { reason } if reason.contains("too short")));
    }

    #[test]
    fn dpo_pair_rejects_extreme_length_ratio() {
        // chosen is a 10× longer string than rejected; rejected must be ≥ MIN_REJECTED_CHARS
        let rejected = "A rejected response of sufficient length to pass the minimum character threshold for the corpus gate quality check.";
        let chosen = "x".repeat(rejected.len() * 10);
        let err = check_dpo_pair(rejected, &chosen).unwrap_err();
        assert!(matches!(err, DoormanError::CorpusGateRejected { reason } if reason.contains("ratio")));
    }

    #[test]
    fn dpo_pair_passes_at_ratio_boundary() {
        // just under the 8× cap should pass
        let rejected = "A rejected attempt at solving this problem using a reasonable approach that includes some detail.";
        let chosen = "x".repeat((rejected.len() as f64 * 7.9) as usize);
        check_dpo_pair(rejected, &chosen).unwrap();
    }

    #[test]
    fn dpo_pair_rejects_unified_diff_colon_placeholder_with_real_markers() {
        // Adversarial case confirmed by 6-agent corpus audit (2026-06-15):
        // 291/324 `<unified diff:` placeholders embed "--- a/" and "@@ " so
        // the old !has_real_diff gate was bypassed, letting 85 pass all checks.
        // The colon suffix is now in TEMPLATE_ECHO_PREFIXES and caught before
        // the REAL_DIFF_MARKERS check runs.
        let placeholder = "<unified diff: --- a/Cargo.toml\n+++ b/Cargo.toml\n@@ -1 +1 @@\n-version = \"0.0.1\"\n+version = \"0.1.0\"\n".repeat(4);
        let chosen = "diff --git a/Cargo.toml b/Cargo.toml\n--- a/Cargo.toml\n+++ b/Cargo.toml\n@@ -1 +1 @@\n-version = \"0.0.1\"\n+version = \"0.1.0\"".repeat(3);
        let err = check_dpo_pair(&placeholder, &chosen).unwrap_err();
        assert!(
            matches!(err, DoormanError::CorpusGateRejected { reason } if reason.contains("template placeholder")),
            "placeholder with embedded real markers must be rejected by template-echo check"
        );
    }

    #[test]
    fn dpo_pair_rejects_short_chosen() {
        // A real rejected diff but an empty/near-empty chosen — an inverted pair
        // that teaches the model to prefer producing nothing.
        let rejected = "A rejected attempt of sufficient length to clear the minimum-character floor on the rejected side.";
        let err = check_dpo_pair(rejected, "ok").unwrap_err();
        assert!(
            matches!(err, DoormanError::CorpusGateRejected { reason } if reason.contains("chosen side too short")),
            "near-empty chosen must be rejected; got {err:?}"
        );
    }

    #[test]
    fn dpo_pair_rejects_system_prompt_example_echo() {
        // OLMo copied the verbatim system-prompt diff example. It carries
        // "--- a/" and "@@ " so REAL_DIFF_MARKERS would pass it — the
        // ≥2-marker co-occurrence check must catch it first.
        let echo = "--- a/path/to/file\n+++ b/path/to/file\n@@ -1,3 +1,3 @@\n context line\n-old line\n+new line\n";
        let err = check_dpo_pair(echo, decent_chosen()).unwrap_err();
        assert!(
            matches!(err, DoormanError::CorpusGateRejected { reason } if reason.contains("system-prompt diff example")),
            "verbatim system-prompt example echo must be rejected; got {err:?}"
        );
    }
}
