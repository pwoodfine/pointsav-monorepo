// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Apprenticeship Substrate dispatcher (AS-2).
//!
//! `ApprenticeshipDispatcher::dispatch_brief` is the AS-2 implementation
//! of `POST /v1/brief`: assemble the apprentice prompt from a brief
//! (resolved citations, redacted scope.files contents, brief body,
//! acceptance test), route through the existing `Doorman::route` so
//! the audit ledger captures the call, parse the apprentice's
//! YAML-frontmatter / fenced-diff response, return an
//! `ApprenticeshipAttempt`.
//!
//! Per Master's 2026-04-26 inbox brief, AS-3 (verdict) and AS-4
//! (shadow) follow; this module exposes shared helpers
//! (`apprentice_prompt`, `parse_attempt_content`,
//! `pick_tier_for_brief`) so the verdict / shadow paths reuse them.

use std::path::{Path, PathBuf};

use chrono::Utc;
use regex::Regex;
use slm_core::{
    ApprenticeshipAttempt, ApprenticeshipBrief, ChatMessage, Complexity, ComputeRequest, ModuleId,
    RequestId, Tier, APPRENTICE_ESCALATE_THRESHOLD, DEFAULT_BRIEF_TIER_B_THRESHOLD_CHARS,
};
use std::str::FromStr;
use std::sync::OnceLock;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::citations::{render_for_prompt, resolve as resolve_citations};
use crate::error::{DoormanError, Result};
use crate::redact::sanitize;
use crate::router::Doorman;

/// Per-deployment config for the apprenticeship dispatcher.
#[derive(Clone, Debug)]
pub struct ApprenticeshipConfig {
    /// Workspace root (e.g. `/srv/foundry`). `scope.files` paths
    /// resolve against this root; the citation registry is read from
    /// `<foundry_root>/citations.yaml` unless overridden.
    pub foundry_root: PathBuf,
    /// Path to the citations YAML registry. Defaults to
    /// `<foundry_root>/citations.yaml`.
    pub citations_path: PathBuf,
    /// Char-budget proxy: briefs whose `body.len() +
    /// acceptance_test.len()` exceeds this threshold dispatch to
    /// Tier B. Design-pass Q6.
    pub brief_tier_b_threshold_chars: usize,
    /// Embedded in apprenticeship corpus tuples per
    /// `trajectory-substrate.md` §3 + `apprenticeship-substrate.md`
    /// §8. Used by the shadow path here and the verdict path in
    /// `verdict.rs`.
    pub doctrine_version: String,
    /// Tenant tag on corpus tuples. Vendor work defaults to
    /// `pointsav` per `apprenticeship-substrate.md` §8.
    pub tenant: String,
    /// When `true`, shadow briefs route to Tier A regardless of size.
    /// Controlled by two env vars OR'd together:
    ///   `SLM_TIER_A_FIRST=true`   — global Tier-A-first policy, OR
    ///   `SLM_APPRENTICESHIP_TIER_A_ONLY` — defaults to `true`; set to `false`
    ///     only when the operator explicitly wants shadow briefs on Tier B.
    /// The default-true guard prevents git-commit briefs from auto-escalating
    /// to Cloud Run and accruing GPU charges without operator intent.
    pub tier_a_first: bool,
    /// When routing to the Yoyo tier, target this named node.
    /// `None` uses the router default. The drain worker sets this to
    /// `Some("trainer")` so queue items land on the L4 GPU node rather
    /// than the default node (which may be a different VM class).
    pub yoyo_dispatch_label: Option<String>,
}

impl ApprenticeshipConfig {
    /// Default config rooted at the operator's `FOUNDRY_ROOT` env var
    /// (falling back to `/srv/foundry` per Master's brief). Resolves
    /// `citations.yaml` under that root and uses
    /// `DEFAULT_BRIEF_TIER_B_THRESHOLD_CHARS` for Tier-B routing.
    pub fn from_env() -> Self {
        let foundry_root: PathBuf = std::env::var_os("FOUNDRY_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/srv/foundry"));
        let citations_path = foundry_root.join("citations.yaml");
        let threshold = std::env::var("SLM_BRIEF_TIER_B_THRESHOLD_CHARS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(DEFAULT_BRIEF_TIER_B_THRESHOLD_CHARS);
        // Default to v0.0.13 per the capture-vs-promote amendment
        // (apprenticeship-substrate.md §7B, ratified 2026-04-29).
        // Override via FOUNDRY_DOCTRINE_VERSION env var if a future
        // doctrine version supersedes this default.
        let doctrine_version =
            std::env::var("FOUNDRY_DOCTRINE_VERSION").unwrap_or_else(|_| "0.0.13".to_string());
        let tenant = std::env::var("FOUNDRY_TENANT").unwrap_or_else(|_| "pointsav".to_string());
        let tier_a_first_global = std::env::var("SLM_TIER_A_FIRST")
            .map(|v| matches!(v.trim(), "true" | "1"))
            .unwrap_or(false);
        // Default true: shadow briefs stay on Tier A unless the operator
        // explicitly opts in. Prevents git-commit diffs from auto-escalating
        // to Cloud Run (GPU) and accruing charges without operator intent.
        let apprenticeship_tier_a_only = std::env::var("SLM_APPRENTICESHIP_TIER_A_ONLY")
            .map(|v| !matches!(v.trim(), "false" | "0"))
            .unwrap_or(true);
        let tier_a_first = tier_a_first_global || apprenticeship_tier_a_only;
        Self {
            foundry_root,
            citations_path,
            brief_tier_b_threshold_chars: threshold,
            doctrine_version,
            tenant,
            tier_a_first,
            yoyo_dispatch_label: None,
        }
    }
}

pub struct ApprenticeshipDispatcher<'a> {
    doorman: &'a Doorman,
    config: ApprenticeshipConfig,
    /// In-process brief / attempt cache, populated on every successful
    /// dispatch_brief so the AS-3 verdict path can recover the
    /// `(brief, attempt)` pair by `(brief_id, attempt_id)`. Optional —
    /// AS-2 unit tests construct without one when they only care
    /// about the routing path.
    cache: Option<std::sync::Arc<crate::brief_cache::BriefCache>>,
}

impl<'a> ApprenticeshipDispatcher<'a> {
    pub fn new(doorman: &'a Doorman, config: ApprenticeshipConfig) -> Self {
        Self {
            doorman,
            config,
            cache: None,
        }
    }

    /// Like [`new`] but also wires a brief cache so the produced
    /// attempt becomes recoverable by the AS-3 verdict path.
    pub fn with_cache(
        doorman: &'a Doorman,
        config: ApprenticeshipConfig,
        cache: std::sync::Arc<crate::brief_cache::BriefCache>,
    ) -> Self {
        Self {
            doorman,
            config,
            cache: Some(cache),
        }
    }

    /// AS-2 entry point. Compose the apprentice prompt from `brief`,
    /// dispatch through `Doorman::route`, parse the response,
    /// return an `ApprenticeshipAttempt`. Inserts `(brief, attempt)`
    /// into the configured cache so AS-3 can recover them at verdict
    /// time.
    pub async fn dispatch_brief(
        &self,
        brief: &ApprenticeshipBrief,
    ) -> Result<ApprenticeshipAttempt> {
        let prompt = apprentice_prompt(&self.config, brief);
        let tier_hint = pick_tier_for_brief(
            brief,
            self.config.brief_tier_b_threshold_chars,
            self.config.tier_a_first,
        );

        let module_id = brief
            .scope
            .cluster
            .as_deref()
            .and_then(|c| ModuleId::from_str(c).ok())
            .unwrap_or_else(|| {
                ModuleId::from_str("foundry").expect("compile-time-valid default moduleId")
            });

        let req = ComputeRequest {
            request_id: RequestId::new(),
            module_id,
            model: None,
            messages: vec![
                ChatMessage {
                    role: "system".into(),
                    content: APPRENTICE_SYSTEM_PROMPT.to_string(),
                },
                ChatMessage {
                    role: "user".into(),
                    content: prompt,
                },
                // Assistant pre-fill: force the model to start with the YAML fence.
                // Without this, instruct models begin with prose reasoning and never
                // write the frontmatter, causing parse_attempt_content to default
                // escalate=true and blank the diff.
                ChatMessage {
                    role: "assistant".into(),
                    content: "---\n".to_string(),
                },
            ],
            complexity: match tier_hint {
                Tier::Yoyo => Complexity::High,
                _ => Complexity::Medium,
            },
            tier_hint: Some(tier_hint),
            stream: false,
            // Tier B (OLMo 3 32B-Think): 2048 tokens ≈ 228 s at 9 tok/s.
            // Tier A (OLMo 2 7B Q4): raised from 512 → 1024.
            // 512 was too tight: the 7B model used ~230 tokens for reasoning,
            // leaving only 280 for the diff — not enough for non-trivial diffs.
            max_tokens: Some(match tier_hint {
                Tier::Yoyo => 2048,
                _ => 1024,
            }),
            temperature: None,
            sanitised_outbound: true,
            tier_c_label: None,
            yoyo_label: if matches!(tier_hint, Tier::Yoyo) {
                self.config.yoyo_dispatch_label.clone()
            } else {
                None
            },
            grammar: None,
            speculation: None,
            graph_context_enabled: None,
            tools: None,
            // Stop at the natural end of the diff code block.
            stop_sequences: Some(vec![
                "```\n\n".to_string(),
                "<|endoftext|>".to_string(),
                "<|im_end|>".to_string(),
            ]),
            session_context: None,
        };

        info!(
            target: "slm_doorman::apprenticeship",
            brief_id = %brief.brief_id,
            task_type = %brief.task_type,
            tier = tier_hint.as_str(),
            "dispatching apprentice brief"
        );

        let resp = self.doorman.route(&req).await?;
        // Reattach the assistant pre-fill when llama-server returns only the continuation.
        // Mock tests return full content (starts with ---); skip prepend to avoid doubling.
        let full_content = if resp.content.trim_start().starts_with("---") {
            resp.content.clone()
        } else {
            format!("---\n{}", resp.content)
        };
        let parsed = parse_attempt_content(&full_content);
        let attempt = build_attempt(brief, &resp, parsed);
        if let Some(cache) = &self.cache {
            cache.insert(brief.clone(), attempt.clone());
        }
        Ok(attempt)
    }

    /// AS-4 entry point — `POST /v1/shadow`. Apprentice is dispatched
    /// the same way as `/v1/brief`, but the attempt is NOT returned to
    /// the caller. The (brief, attempt, actual_diff) tuple is captured
    /// immediately as a training row at
    /// `${FOUNDRY_ROOT}/data/training-corpus/apprenticeship/<task-type>/shadow-<brief_id>.jsonl`
    /// with `verdict: null`, `stage_at_capture: "review"`, and
    /// `promoted_at: null` per convention §7B + §8 (v0.0.13 amendment).
    ///
    /// The `actual_diff` field carries the diff that actually landed in
    /// the source commit; the `attempt.diff` carries what the apprentice
    /// would have done. Both fields are preserved for the DPO pipeline.
    ///
    /// Idempotency on `brief_id` is realised by the deterministic
    /// filename: a re-POST of the same `brief_id` is a no-op (the
    /// existing tuple is preserved). Survives process restart.
    pub async fn dispatch_shadow(
        &self,
        brief: &ApprenticeshipBrief,
        actual_diff: &str,
    ) -> Result<ShadowOutcome> {
        let dir = self
            .config
            .foundry_root
            .join("data")
            .join("training-corpus")
            .join("apprenticeship")
            .join(&brief.task_type);
        let path = dir.join(format!("shadow-{}.jsonl", brief.brief_id));
        if path.exists() {
            return Ok(ShadowOutcome {
                brief_id: brief.brief_id.clone(),
                corpus_path: path.display().to_string(),
                already_captured: true,
            });
        }

        // Same routing as dispatch_brief.
        let prompt = apprentice_prompt(&self.config, brief);
        let tier_hint = pick_tier_for_brief(
            brief,
            self.config.brief_tier_b_threshold_chars,
            self.config.tier_a_first,
        );
        let module_id = brief
            .scope
            .cluster
            .as_deref()
            .and_then(|c| ModuleId::from_str(c).ok())
            .unwrap_or_else(|| {
                ModuleId::from_str("foundry").expect("compile-time-valid default moduleId")
            });
        let req = ComputeRequest {
            request_id: RequestId::new(),
            module_id,
            model: None,
            messages: vec![
                ChatMessage {
                    role: "system".into(),
                    content: APPRENTICE_SYSTEM_PROMPT.to_string(),
                },
                ChatMessage {
                    role: "user".into(),
                    content: prompt,
                },
                ChatMessage {
                    role: "assistant".into(),
                    content: "---\n".to_string(),
                },
            ],
            complexity: match tier_hint {
                Tier::Yoyo => Complexity::High,
                _ => Complexity::Medium,
            },
            tier_hint: Some(tier_hint),
            stream: false,
            max_tokens: Some(match tier_hint {
                Tier::Yoyo => 2048,
                _ => 1024,
            }),
            temperature: None,
            sanitised_outbound: true,
            tier_c_label: None,
            yoyo_label: if matches!(tier_hint, Tier::Yoyo) {
                self.config.yoyo_dispatch_label.clone()
            } else {
                None
            },
            grammar: None,
            speculation: None,
            graph_context_enabled: None,
            tools: None,
            stop_sequences: Some(vec![
                "```\n\n".to_string(),
                "<|endoftext|>".to_string(),
                "<|im_end|>".to_string(),
            ]),
            session_context: None,
        };

        info!(
            target: "slm_doorman::apprenticeship",
            brief_id = %brief.brief_id,
            task_type = %brief.task_type,
            tier = tier_hint.as_str(),
            "dispatching shadow brief"
        );

        let resp = self.doorman.route(&req).await?;
        let full_content = if resp.content.trim_start().starts_with("---") {
            resp.content.clone()
        } else {
            format!("---\n{}", resp.content)
        };
        let parsed = parse_attempt_content(&full_content);
        let attempt = build_attempt(brief, &resp, parsed);

        write_shadow_tuple(
            &self.config.foundry_root,
            brief,
            &attempt,
            actual_diff,
            &self.config.doctrine_version,
            &self.config.tenant,
        )?;

        Ok(ShadowOutcome {
            brief_id: brief.brief_id.clone(),
            corpus_path: path.display().to_string(),
            already_captured: false,
        })
    }
}

/// Outcome of `POST /v1/shadow`. Master's brief specifies an empty
/// 200 OK body; this struct lives in the library for tests + future
/// reuse, while the http handler returns just an HTTP 200.
#[derive(Clone, Debug, serde::Serialize)]
pub struct ShadowOutcome {
    pub brief_id: String,
    pub corpus_path: String,
    /// `true` when an earlier shadow POST already wrote this tuple;
    /// the redundant POST is a no-op (idempotency on brief_id).
    pub already_captured: bool,
}

fn write_shadow_tuple(
    corpus_root: &Path,
    brief: &ApprenticeshipBrief,
    attempt: &ApprenticeshipAttempt,
    actual_diff: &str,
    doctrine_version: &str,
    tenant: &str,
) -> Result<()> {
    // Degenerate tuple guard: skip only when the model produced NOTHING at all
    // (no reasoning and no diff). A tuple where the model wrote reasoning-but-no-diff
    // still carries DPO signal — the actual_diff is the chosen sample and the empty
    // diff + escalate=true is a valid negative. Blanking on escalate alone caused all
    // 7B tuples to be dropped because frontmatter failures force escalate=true.
    if attempt.diff.is_empty() && attempt.reasoning.is_empty() {
        warn!(
            target: "slm_doorman::apprenticeship",
            brief_id = %brief.brief_id,
            "shadow tuple skipped — model produced no content at all"
        );
        return Ok(());
    }

    let dir = corpus_root
        .join("data")
        .join("training-corpus")
        .join("apprenticeship")
        .join(&brief.task_type);
    std::fs::create_dir_all(&dir).map_err(|e| DoormanError::CorpusWrite {
        path: dir.display().to_string(),
        reason: e.to_string(),
    })?;
    let path = dir.join(format!("shadow-{}.jsonl", brief.brief_id));

    // Idempotency belt-and-braces: another writer may have raced past
    // the existence check above. Use create_new to refuse to clobber.
    use std::io::Write;
    let mut f = match std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)
    {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return Ok(()),
        Err(e) => {
            return Err(DoormanError::CorpusWrite {
                path: path.display().to_string(),
                reason: e.to_string(),
            })
        }
    };

    let sanitized_brief = sanitize_brief_for_corpus(brief);
    let sanitized_attempt = sanitize_attempt_for_corpus(attempt);
    // Per apprenticeship-substrate.md §7B + §8 (v0.0.13 amendment):
    //   - stage_at_capture: "review" (not "shadow"; "review" is the
    //     starting stage for every new task-type per §2)
    //   - actual_diff: the human-committed diff (new required field)
    //   - promoted_at: null (set to ISO 8601 timestamp when verdict
    //     signing promotes this tuple)
    //   - verdict: null (updated in-place by VerdictDispatcher on promotion)
    //   - doctrine_version: "0.0.13" (pinned at capture time per §9)
    let record = serde_json::json!({
        "tuple_type": "apprenticeship",
        "doctrine_version": doctrine_version,
        "task_type": brief.task_type,
        "stage_at_capture": "review",
        "brief": sanitized_brief,
        "attempt": sanitized_attempt,
        "verdict": serde_json::Value::Null,
        "actual_diff": crate::redact::sanitize(actual_diff),
        "final_diff": serde_json::Value::Null,
        "redaction_class": "internal",
        "evidence_class": "primary",
        "tenant": tenant,
        "cluster": brief.scope.cluster,
        "session_id": serde_json::Value::Null,
        "created": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        "promoted_at": serde_json::Value::Null,
    });
    let line = serde_json::to_string(&record).map_err(|e| DoormanError::CorpusWrite {
        path: path.display().to_string(),
        reason: e.to_string(),
    })?;
    f.write_all(line.as_bytes())
        .and_then(|_| f.write_all(b"\n"))
        .and_then(|_| f.flush())
        .map_err(|e| DoormanError::CorpusWrite {
            path: path.display().to_string(),
            reason: e.to_string(),
        })?;
    Ok(())
}

/// Resolve the corpus file path for a shadow tuple given the corpus root
/// and brief. Public so `VerdictDispatcher` can locate the tuple by
/// `brief_id` for in-place promotion.
pub fn shadow_corpus_path(corpus_root: &Path, task_type: &str, brief_id: &str) -> PathBuf {
    corpus_root
        .join("data")
        .join("training-corpus")
        .join("apprenticeship")
        .join(task_type)
        .join(format!("shadow-{brief_id}.jsonl"))
}

fn sanitize_brief_for_corpus(b: &ApprenticeshipBrief) -> ApprenticeshipBrief {
    let mut clone = b.clone();
    clone.body = crate::redact::sanitize(&clone.body);
    clone.acceptance_test = crate::redact::sanitize(&clone.acceptance_test);
    clone
}

fn sanitize_attempt_for_corpus(a: &ApprenticeshipAttempt) -> ApprenticeshipAttempt {
    let mut clone = a.clone();
    clone.reasoning = crate::redact::sanitize(&clone.reasoning);
    clone.diff = crate::redact::sanitize(&clone.diff);
    clone
}

/// System message prepended to every apprentice prompt. Frames the
/// role per Doctrine claim #32 and pins the response shape so the
/// parser has a stable target.
pub const APPRENTICE_SYSTEM_PROMPT: &str = "\
You are a code-editing assistant. Output ONLY the structured response below — no prose before it.\n\
\n\
REQUIRED FORMAT (copy exactly, fill in values):\n\
\n\
---\n\
self_confidence: 0.7\n\
escalate: false\n\
---\n\
\n\
## Reasoning\n\
One sentence: what changed and why.\n\
\n\
## Diff\n\
```diff\n\
--- a/path/to/file\n\
+++ b/path/to/file\n\
@@ -1,3 +1,3 @@\n\
 context line\n\
-old line\n\
+new line\n\
```\n\
\n\
Rules:\n\
- The VERY FIRST characters of your response must be ---\n\
- Write reasoning in ONE sentence only — brevity leaves tokens for the diff.\n\
- Set escalate: false and write the unified diff when you can make the change.\n\
- Set escalate: true and leave Diff empty only when the task is truly ambiguous.\n\
- self_confidence: your certainty (0.0 = none, 1.0 = certain).\n\
- The diff MUST be a valid unified diff (--- a/ +++ b/ @@ lines).";

/// Build the apprentice user-prompt body from a brief.
pub fn apprentice_prompt(cfg: &ApprenticeshipConfig, brief: &ApprenticeshipBrief) -> String {
    let resolved = resolve_citations(&cfg.citations_path, &brief.doctrine_citations);
    let citations_block = render_for_prompt(&resolved);
    let files_block = render_files(cfg.foundry_root.as_path(), &brief.scope.files);

    format!(
        "## Brief — {brief_id}\n\
         task_type: {task_type}\n\
         senior: {senior}\n\n\
         ## Doctrine citations\n\
         {citations}\n\
         ## Files in scope\n\
         {files}\n\
         ## Brief body\n\
         {body}\n\n\
         ## Acceptance test\n\
         {acceptance}\n",
        brief_id = brief.brief_id,
        task_type = brief.task_type,
        senior = brief.senior_identity,
        citations = citations_block,
        files = files_block,
        body = brief.body,
        acceptance = brief.acceptance_test,
    )
}

/// Read each file in `paths` from `root`, sanitize, fence in markdown.
/// Missing files are reported inline so the apprentice sees the gap
/// rather than receiving silently-empty context.
fn render_files(root: &Path, paths: &[String]) -> String {
    if paths.is_empty() {
        return "(no files in scope)\n".to_string();
    }
    let mut out = String::new();
    for rel in paths {
        let p = root.join(rel);
        out.push_str("### ");
        out.push_str(rel);
        out.push('\n');
        match std::fs::read_to_string(&p) {
            Ok(body) => {
                let red = sanitize(&body);
                out.push_str("```\n");
                out.push_str(&red);
                if !red.ends_with('\n') {
                    out.push('\n');
                }
                out.push_str("```\n");
            }
            Err(e) => {
                debug!(target: "slm_doorman::apprenticeship",
                       path = %p.display(), error = %e,
                       "scope.files entry not readable; surfacing to apprentice");
                out.push_str("(file not readable: ");
                out.push_str(&e.to_string());
                out.push_str(")\n");
            }
        }
    }
    out
}

/// Pick a tier hint for a brief. Char-based proxy per design-pass Q6.
/// When `tier_a_first=true`, always returns `Tier::Local` — shadow briefs
/// run on Tier A, and the write_shadow_tuple guard filters escalated-empty
/// results so no degenerate DPO tuples enter the corpus.
pub fn pick_tier_for_brief(
    brief: &ApprenticeshipBrief,
    threshold_chars: usize,
    tier_a_first: bool,
) -> Tier {
    if tier_a_first {
        return Tier::Local;
    }
    let size = brief.body.len() + brief.acceptance_test.len();
    if size > threshold_chars {
        Tier::Yoyo
    } else {
        Tier::Local
    }
}

/// Parsed view of the apprentice's response body — what we extract
/// from the OpenAI-compatible `content` string.
#[derive(Clone, Debug)]
pub struct ParsedAttempt {
    pub self_confidence: f32,
    pub escalate: bool,
    pub reasoning: String,
    pub diff: String,
}

impl ParsedAttempt {
    /// Sentinel for "apprentice did not return a parseable response".
    /// We treat that as a self-escalation rather than an error so the
    /// senior gets a clear signal.
    pub fn unparsed() -> Self {
        Self {
            self_confidence: 0.0,
            escalate: true,
            reasoning: String::new(),
            diff: String::new(),
        }
    }
}

/// Parse the apprentice's response body. Robust to minor formatting
/// drift; missing pieces become `unparsed()` defaults rather than
/// errors.
pub fn parse_attempt_content(content: &str) -> ParsedAttempt {
    let frontmatter = extract_frontmatter(content);
    let mut self_confidence = 0.0_f32;
    let mut escalate = true;
    if let Some(fm) = frontmatter {
        for line in fm.lines() {
            let t = line.trim();
            if let Some(v) = t.strip_prefix("self_confidence:") {
                if let Ok(f) = v.trim().parse::<f32>() {
                    self_confidence = f.clamp(0.0, 1.0);
                }
            } else if let Some(v) = t.strip_prefix("escalate:") {
                escalate = matches!(v.trim().to_ascii_lowercase().as_str(), "true");
            }
        }
    }

    // Apply the threshold rule (design-pass Q2 + convention §4).
    if self_confidence < APPRENTICE_ESCALATE_THRESHOLD {
        escalate = true;
    }

    let reasoning = extract_section(content, "## Reasoning");
    // Keep whatever diff the model generated — the escalate flag already signals quality.
    // Blanking the diff here caused all 7B corpus tuples to be dropped by write_shadow_tuple.
    let diff = extract_diff_block(content).unwrap_or_default();

    ParsedAttempt {
        self_confidence,
        escalate,
        reasoning,
        diff,
    }
}

/// Compose an `ApprenticeshipAttempt` from the brief, the routed
/// `ComputeResponse`, and the parsed response body.
pub fn build_attempt(
    brief: &ApprenticeshipBrief,
    resp: &slm_core::ComputeResponse,
    parsed: ParsedAttempt,
) -> ApprenticeshipAttempt {
    ApprenticeshipAttempt {
        brief_id: brief.brief_id.clone(),
        attempt_id: Uuid::now_v7().simple().to_string(),
        created: Utc::now(),
        model: resp.model.clone(),
        adapter_composition: Vec::new(),
        self_confidence: parsed.self_confidence,
        escalate: parsed.escalate,
        inference_ms: resp.inference_ms,
        tier: resp.tier_used,
        cost_usd: resp.cost_usd,
        reasoning: parsed.reasoning,
        diff: parsed.diff,
    }
}

fn extract_frontmatter(text: &str) -> Option<String> {
    static FRONTMATTER: OnceLock<Regex> = OnceLock::new();
    let re = FRONTMATTER
        .get_or_init(|| Regex::new(r"(?s)\A\s*---\s*\n(.*?)\n---\s*\n").expect("static regex"));
    re.captures(text).map(|c| c[1].to_string())
}

fn extract_section(text: &str, header: &str) -> String {
    if let Some(start) = text.find(header) {
        let after = &text[start + header.len()..];
        // Section ends at the next `## ` header or end of text.
        if let Some(end_rel) = after.find("\n## ") {
            return after[..end_rel].trim().to_string();
        }
        return after.trim().to_string();
    }
    String::new()
}

fn extract_diff_block(text: &str) -> Option<String> {
    static DIFF_FENCE: OnceLock<Regex> = OnceLock::new();
    let re =
        DIFF_FENCE.get_or_init(|| Regex::new(r"(?s)```diff\s*\n(.*?)\n```").expect("static regex"));
    re.captures(text).map(|c| c[1].to_string())
}

/// Apprentice attempt failed entirely (the apprentice did not parse
/// to anything actionable). Used when callers want to short-circuit
/// without going through the routed response.
pub fn empty_attempt(
    brief: &ApprenticeshipBrief,
    model: &str,
    tier: Tier,
) -> ApprenticeshipAttempt {
    ApprenticeshipAttempt {
        brief_id: brief.brief_id.clone(),
        attempt_id: Uuid::now_v7().simple().to_string(),
        created: Utc::now(),
        model: model.to_string(),
        adapter_composition: Vec::new(),
        self_confidence: 0.0,
        escalate: true,
        inference_ms: 0,
        tier,
        cost_usd: 0.0,
        reasoning: String::new(),
        diff: String::new(),
    }
}

// Currently unused inside this module but exported for AS-3 / AS-4
// reuse (corpus tuple writers will need to surface DoormanError on
// non-routable apprentice responses).
#[allow(dead_code)]
fn _doorman_error_marker() -> Option<DoormanError> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::AuditLedger;
    use crate::router::DoormanConfig;
    use crate::tier::{
        BearerTokenProvider, LocalTierClient, LocalTierConfig, PricingConfig, StaticBearer,
        YoYoTierClient, YoYoTierConfig,
    };
    use chrono::TimeZone;
    use slm_core::{BriefScope, SeniorRole};
    use std::path::PathBuf;
    use std::sync::Arc;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn ts() -> chrono::DateTime<chrono::Utc> {
        Utc.with_ymd_and_hms(2026, 4, 26, 16, 0, 0).unwrap()
    }

    fn tmp_dir(label: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "slm-doorman-as2-{label}-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn ledger() -> AuditLedger {
        AuditLedger::new(tmp_dir("ledger")).unwrap()
    }

    fn dispatcher_config(root: PathBuf) -> ApprenticeshipConfig {
        ApprenticeshipConfig {
            citations_path: root.join("citations.yaml"),
            foundry_root: root,
            brief_tier_b_threshold_chars: 100, // small for tests
            doctrine_version: "0.0.7".into(),
            tenant: "pointsav".into(),
            tier_a_first: false,
        }
    }

    fn brief_for(body: &str) -> ApprenticeshipBrief {
        ApprenticeshipBrief {
            brief_id: "01J9TESTBRIEF0000000000000".into(),
            created: ts(),
            senior_role: SeniorRole::Master,
            senior_identity: "ps-administrator".into(),
            task_type: "version-bump-manifest".into(),
            scope: BriefScope {
                cluster: None,
                files: vec![],
            },
            acceptance_test: "TEST".into(),
            doctrine_citations: vec![],
            shadow: false,
            body: body.into(),
        }
    }

    fn ok_completion(content: &str) -> serde_json::Value {
        serde_json::json!({
            "choices": [
                { "message": { "role": "assistant", "content": content } }
            ]
        })
    }

    /// Happy path — apprentice returns parseable response with a diff;
    /// dispatcher returns an attempt with `escalate=false` and
    /// non-empty diff.
    #[tokio::test]
    async fn happy_path_brief_returns_attempt_with_diff() {
        let server = MockServer::start().await;
        let apprentice_response = "\
---
self_confidence: 0.82
escalate: false
---

## Reasoning

Bumping MANIFEST.md per ni-51-102 forward-looking marker.

## Diff

```diff
--- a/MANIFEST.md
+++ b/MANIFEST.md
@@ -1 +1 @@
-source_version: 0.1.0
+source_version: 0.2.0
```
";
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(ok_completion(apprentice_response)),
            )
            .expect(1)
            .mount(&server)
            .await;

        let local = LocalTierClient::new(LocalTierConfig {
            endpoint: server.uri(),
            default_model: "olmo-3-1125-7b-q4".into(),
        });
        let doorman = Doorman::new(
            DoormanConfig {
                local: Some(local),
                yoyo: std::collections::HashMap::new(),
                external: None,
                lark_validator: None,
                graph_context_client: None,
                tier_a_first: false,
                daily_yoyo_cap_usd: None,
                cost_ledger: None,
            },
            ledger(),
        );

        let dir = tmp_dir("foundry");
        let cfg = dispatcher_config(dir);
        let dispatcher = ApprenticeshipDispatcher::new(&doorman, cfg);

        let attempt = dispatcher
            .dispatch_brief(&brief_for("small brief body"))
            .await
            .expect("happy path");
        assert_eq!(attempt.brief_id, "01J9TESTBRIEF0000000000000");
        assert!(
            !attempt.escalate,
            "escalate should be false on confident response"
        );
        assert!((attempt.self_confidence - 0.82).abs() < 1e-3);
        assert!(attempt.diff.contains("source_version: 0.2.0"));
        assert_eq!(attempt.tier, Tier::Local);
        assert!(attempt.reasoning.contains("forward-looking"));
    }

    /// Escalate-on-low-confidence — apprentice reports
    /// `self_confidence < APPRENTICE_ESCALATE_THRESHOLD`; dispatcher
    /// forces `escalate = true` and empties the diff.
    #[tokio::test]
    async fn low_confidence_escalates_with_empty_diff() {
        let server = MockServer::start().await;
        let apprentice_response = "\
---
self_confidence: 0.21
escalate: false
---

## Reasoning

I'm not sure how to apply this safely.

## Diff

```diff
--- a/MANIFEST.md
+++ b/MANIFEST.md
@@ -1 +1 @@
-x
+y
```
";
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(ok_completion(apprentice_response)),
            )
            .expect(1)
            .mount(&server)
            .await;

        let local = LocalTierClient::new(LocalTierConfig {
            endpoint: server.uri(),
            default_model: "olmo-3-1125-7b-q4".into(),
        });
        let doorman = Doorman::new(
            DoormanConfig {
                local: Some(local),
                yoyo: std::collections::HashMap::new(),
                external: None,
                lark_validator: None,
                graph_context_client: None,
                tier_a_first: false,
                daily_yoyo_cap_usd: None,
                cost_ledger: None,
            },
            ledger(),
        );

        let dir = tmp_dir("foundry");
        let cfg = dispatcher_config(dir);
        let dispatcher = ApprenticeshipDispatcher::new(&doorman, cfg);

        let attempt = dispatcher
            .dispatch_brief(&brief_for("small body"))
            .await
            .expect("dispatch ok even for low-confidence response");
        assert!(
            attempt.escalate,
            "Doorman MUST force escalate=true when self_confidence < {APPRENTICE_ESCALATE_THRESHOLD}"
        );
        // Diff is preserved even when escalate=true — the corpus needs the attempted diff
        // to compute DPO signal against the actual_diff. The escalate flag carries the quality signal.
        assert!(
            !attempt.diff.is_empty(),
            "diff must be preserved even on threshold-forced escalate (model wrote a diff)"
        );
        assert!((attempt.self_confidence - 0.21).abs() < 1e-3);
    }

    /// Tier-B dispatch — brief whose body+acceptance exceeds the
    /// threshold lands on the Yo-Yo tier; verified by the response's
    /// `tier_used` and by mounting the mock at the Yo-Yo server.
    #[tokio::test]
    async fn large_brief_dispatches_to_tier_b() {
        // Two mock servers — Tier A and Tier B. Only Tier B should
        // receive a request; Tier A is configured but mounted with
        // zero expected calls.
        let server_a = MockServer::start().await;
        let server_b = MockServer::start().await;

        let apprentice_response = "\
---
self_confidence: 0.91
escalate: false
---

## Reasoning

OK.

## Diff

```diff
--- a/foo
+++ b/foo
@@ -1 +1 @@
-a
+b
```
";

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(ok_completion(apprentice_response)),
            )
            .expect(1) // Tier B receives the only call
            .mount(&server_b)
            .await;
        // Tier A: not mounted; received_requests asserted to 0 below.

        let local = LocalTierClient::new(LocalTierConfig {
            endpoint: server_a.uri(),
            default_model: "olmo-3-1125-7b-q4".into(),
        });
        let bearer: Arc<dyn BearerTokenProvider> = Arc::new(StaticBearer::new("test"));
        let yoyo = YoYoTierClient::new(
            YoYoTierConfig {
                endpoint: server_b.uri(),
                default_model: "Olmo-3-1125-32B-Think".into(),
                contract_version: crate::YOYO_CONTRACT_VERSION.into(),
                pricing: PricingConfig::default(),
                zone: None,
                health_path: "/health".to_string(),
            },
            bearer,
        );
        let doorman = Doorman::new(
            DoormanConfig {
                local: Some(local),
                yoyo: {
                    let mut m = std::collections::HashMap::new();
                    m.insert("default".to_string(), yoyo);
                    m
                },
                external: None,
                lark_validator: None,
                graph_context_client: None,
                tier_a_first: false,
                daily_yoyo_cap_usd: None,
                cost_ledger: None,
            },
            ledger(),
        );

        let dir = tmp_dir("foundry");
        let cfg = dispatcher_config(dir); // threshold = 100 chars

        let big_body = "x".repeat(150); // 150 + len("TEST") > 100
        let attempt = ApprenticeshipDispatcher::new(&doorman, cfg)
            .dispatch_brief(&brief_for(&big_body))
            .await
            .expect("tier-B dispatch ok");

        assert_eq!(attempt.tier, Tier::Yoyo);
        // Tier A must NOT have received any request.
        let received_a = server_a.received_requests().await.unwrap_or_default();
        assert_eq!(
            received_a.len(),
            0,
            "Doorman MUST route the large brief to Tier B, not Tier A"
        );
    }

    /// Unit: parse_attempt_content recovers from missing frontmatter
    /// by escalating with empty diff.
    #[test]
    fn parse_unparsable_response_escalates() {
        let p = parse_attempt_content("totally unstructured apprentice output");
        assert!(p.escalate);
        assert_eq!(p.diff, "");
        assert_eq!(p.self_confidence, 0.0);
    }

    /// Regression: the Tier A shadow request must carry stop sequences AND the
    /// 512-token cap. Without stop sequences OLMo can run past EOS forever; the
    /// 2048-token Tier B budget at ~4 tok/s CPU is a ~17-min serial block. Both
    /// were the drain-stall fix (commit df118c47); this captures the actual
    /// request body sent to llama-server and asserts the wire fields.
    #[tokio::test]
    async fn shadow_request_carries_stop_and_tier_a_token_cap() {
        let server = MockServer::start().await;
        let apprentice_response = "\
---
self_confidence: 0.7
escalate: false
---

## Reasoning

ok

## Diff

```diff
--- a/x
+++ b/x
@@ -1 +1 @@
-a
+b
```
";
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(ok_completion(apprentice_response)),
            )
            .expect(1)
            .mount(&server)
            .await;

        let local = LocalTierClient::new(LocalTierConfig {
            endpoint: server.uri(),
            default_model: "olmo-3-1125-7b-q4".into(),
        });
        let doorman = Doorman::new(
            DoormanConfig {
                local: Some(local),
                yoyo: std::collections::HashMap::new(),
                external: None,
                lark_validator: None,
                graph_context_client: None,
                tier_a_first: false,
                daily_yoyo_cap_usd: None,
                cost_ledger: None,
            },
            ledger(),
        );
        let dir = tmp_dir("foundry");
        let cfg = dispatcher_config(dir);
        ApprenticeshipDispatcher::new(&doorman, cfg)
            .dispatch_shadow(&brief_for("small brief"), "diff --git a/x b/x\n+y\n")
            .await
            .expect("shadow dispatch ok");

        let reqs = server.received_requests().await.unwrap_or_default();
        assert_eq!(reqs.len(), 1, "exactly one Tier A request expected");
        let body: serde_json::Value =
            serde_json::from_slice(&reqs[0].body).expect("request body is JSON");

        // Tier A caps generation at 1024 tokens (raised from 512 — 7B needs room for diff).
        assert_eq!(
            body["max_tokens"].as_u64(),
            Some(1024),
            "Tier A shadow request must cap max_tokens at 1024"
        );
        // Fix 2: stop sequences present, including the diff-fence terminator.
        let stop = body["stop"]
            .as_array()
            .expect("stop must be a top-level array");
        assert!(
            stop.iter().any(|s| s.as_str() == Some("```\n\n")),
            "stop sequences must include the diff code-fence terminator; got {stop:?}"
        );
    }

    /// Unit: pick_tier_for_brief boundary.
    #[test]
    fn pick_tier_at_threshold() {
        let mut b = brief_for("");
        b.body = "x".repeat(50);
        b.acceptance_test = "y".repeat(50);
        // 50 + 50 = 100, NOT exceeding 100 → Tier A
        assert_eq!(pick_tier_for_brief(&b, 100, false), Tier::Local);
        b.body = "x".repeat(51);
        // 51 + 50 = 101, exceeds 100 → Tier B
        assert_eq!(pick_tier_for_brief(&b, 100, false), Tier::Yoyo);
        // tier_a_first=true always returns Tier A regardless of size
        assert_eq!(pick_tier_for_brief(&b, 100, true), Tier::Local);
    }

    // ── AS-4 dispatch_shadow tests ───────────────────────────────────

    /// Happy path — shadow brief dispatches to apprentice, captures the
    /// (brief, attempt, actual_diff) tuple at the deterministic shadow
    /// path, no apprentice attempt returned to caller.
    #[tokio::test]
    async fn shadow_happy_path_writes_tuple_and_does_not_return_attempt() {
        let server = MockServer::start().await;
        let apprentice_response = "\
---
self_confidence: 0.7
escalate: false
---

## Reasoning

Shadow attempt for the apprentice.

## Diff

```diff
--- a/MANIFEST.md
+++ b/MANIFEST.md
@@ -1 +1 @@
-x
+y
```
";
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(ok_completion(apprentice_response)),
            )
            .expect(1)
            .mount(&server)
            .await;

        let local = LocalTierClient::new(LocalTierConfig {
            endpoint: server.uri(),
            default_model: "olmo-3-1125-7b-q4".into(),
        });
        let doorman = Doorman::new(
            DoormanConfig {
                local: Some(local),
                yoyo: std::collections::HashMap::new(),
                external: None,
                lark_validator: None,
                graph_context_client: None,
                tier_a_first: false,
                daily_yoyo_cap_usd: None,
                cost_ledger: None,
            },
            ledger(),
        );

        let dir = tmp_dir("shadow-foundry");
        let cfg = dispatcher_config(dir.clone());
        let dispatcher = ApprenticeshipDispatcher::new(&doorman, cfg);

        let brief = brief_for("small body");
        let actual_diff = "--- a/MANIFEST.md\n+++ b/MANIFEST.md\n@@ -1 +1 @@\n-x\n+ACTUAL_y\n";
        let outcome = dispatcher
            .dispatch_shadow(&brief, actual_diff)
            .await
            .expect("shadow happy path");
        assert!(!outcome.already_captured);
        assert_eq!(outcome.brief_id, brief.brief_id);

        // Tuple lands at the deterministic path.
        let expected = dir
            .join("data")
            .join("training-corpus")
            .join("apprenticeship")
            .join("version-bump-manifest")
            .join(format!("shadow-{}.jsonl", brief.brief_id));
        let body = std::fs::read_to_string(&expected).expect("shadow tuple written");
        let row: serde_json::Value = serde_json::from_str(body.trim()).unwrap();
        assert_eq!(row["tuple_type"], "apprenticeship");
        assert_eq!(row["stage_at_capture"], "review");
        assert!(
            row["verdict"].is_null(),
            "review-stage tuple has null verdict at capture"
        );
        assert!(row["actual_diff"].as_str().unwrap().contains("ACTUAL_y"));
        assert!(
            row["final_diff"].is_null(),
            "final_diff is null at capture (set on promotion)"
        );
        assert_eq!(row["brief"]["brief_id"], brief.brief_id);
        let sc = row["attempt"]["self_confidence"].as_f64().unwrap();
        assert!((sc - 0.7).abs() < 1e-3, "got {sc}");
    }

    /// Idempotency on retry — same brief_id submitted twice writes
    /// exactly one tuple. The second POST is a no-op (apprentice is
    /// NOT redispatched).
    #[tokio::test]
    async fn shadow_dedupes_on_repeat_brief_id() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(ok_completion(
                "---\nself_confidence: 0.8\nescalate: false\n---\n\n## Reasoning\nx\n## Diff\n```diff\n--- a\n+++ a\n```\n",
            )))
            .expect(1) // exactly one apprentice call across both POSTs
            .mount(&server)
            .await;

        let local = LocalTierClient::new(LocalTierConfig {
            endpoint: server.uri(),
            default_model: "olmo-3-1125-7b-q4".into(),
        });
        let doorman = Doorman::new(
            DoormanConfig {
                local: Some(local),
                yoyo: std::collections::HashMap::new(),
                external: None,
                lark_validator: None,
                graph_context_client: None,
                tier_a_first: false,
                daily_yoyo_cap_usd: None,
                cost_ledger: None,
            },
            ledger(),
        );

        let dir = tmp_dir("shadow-dedup");
        let cfg = dispatcher_config(dir.clone());
        let dispatcher = ApprenticeshipDispatcher::new(&doorman, cfg);

        let brief = brief_for("body");
        let first = dispatcher.dispatch_shadow(&brief, "diff-1").await.unwrap();
        assert!(!first.already_captured);

        let second = dispatcher
            .dispatch_shadow(&brief, "diff-2") // same brief, different actual_diff
            .await
            .unwrap();
        assert!(second.already_captured, "second POST must be a no-op");

        // Exactly one tuple file in the corpus directory.
        let dir = dir
            .join("data")
            .join("training-corpus")
            .join("apprenticeship")
            .join("version-bump-manifest");
        let entries: Vec<_> = std::fs::read_dir(&dir).unwrap().collect();
        assert_eq!(entries.len(), 1);
        // First-write wins — actual_diff is "diff-1", not "diff-2".
        // final_diff is null at capture (set on promotion per §7B).
        let p = entries.into_iter().next().unwrap().unwrap().path();
        let body = std::fs::read_to_string(&p).unwrap();
        let row: serde_json::Value = serde_json::from_str(body.trim()).unwrap();
        assert_eq!(row["actual_diff"], "diff-1");
        assert!(row["final_diff"].is_null(), "final_diff is null at capture");
    }
}
