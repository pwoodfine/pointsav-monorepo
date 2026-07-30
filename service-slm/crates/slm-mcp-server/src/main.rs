// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// slm-mcp-server — Foundry MCP server (Sprint 3 + Sprint 4 + Sprint 5)
//
// Exposes Foundry capabilities as MCP tools over stdio.
// Requires a running slm-doorman-server (default: http://127.0.0.1:9080).
//
// Usage:
//   slm-mcp-server --doorman http://127.0.0.1:9080
//
// Env:
//   SLM_DOORMAN_URL   doorman base URL  (default: http://127.0.0.1:9080)
//   SLM_MODULE_ID     module-id header  (default: mcp-foundry)
//   FOUNDRY_ROOT      workspace root    (default: /srv/foundry)
//
// .mcp.json example:
//   { "mcpServers": { "foundry": {
//       "command": "/usr/local/bin/slm-mcp-server",
//       "args": ["--doorman", "http://127.0.0.1:9080"],
//       "env": { "SLM_MODULE_ID": "mcp-foundry", "FOUNDRY_ROOT": "/srv/foundry" },
//       "type": "stdio"
//   } } }

use rmcp::{tool, tool_handler, tool_router, ServerHandler, ServiceExt};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

// ── Input types ──────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct QueryDatagraphInput {
    /// Free-text or keyword query forwarded to the DataGraph
    q: String,
    /// Maximum number of entity results to return (default 10)
    limit: Option<u32>,
    /// If true, return a pre-formatted [ENTITY CONTEXT] block ready for ask_local prompt injection
    format_for_prompt: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct MutateDatagraphInput {
    /// Mutation payload — must match service-content mutate schema
    mutation: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct GetEntityContextInput {
    /// Entity name or identifier to look up
    entity: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct SubmitExtractionInput {
    /// Prose document content to extract entities from (ADR-07: no structured data)
    text: String,
    /// JSON Schema constraining the output entity array
    schema: serde_json::Value,
    /// Module-id override (defaults to SLM_MODULE_ID env var)
    module_id: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct AskLocalInput {
    /// Prompt to send to the local OLMo model via the Doorman.
    prompt: String,
    /// Maximum tokens to generate (default 300; hard cap 400 — ~108 s at 3.7 tok/s).
    max_tokens: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct GetSessionBriefInput {
    /// Session role: "command" (workspace root at FOUNDRY_ROOT) or "totebox" (archive clone).
    /// Defaults to "command".
    role: Option<String>,
    /// Archive name for totebox role, e.g. "project-intelligence". Required when role="totebox".
    archive: Option<String>,
    /// Maximum number of pending inbox messages to return in full (default 5).
    pending_limit: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct SendMailboxMessageInput {
    /// Destination: "command@claude-code" or "totebox@<archive-name>"
    to: String,
    /// Message subject line
    re: String,
    /// Message body text (no frontmatter — the tool constructs it)
    body: String,
    /// Priority: "high", "normal" (default), or "low"
    priority: Option<String>,
    /// msg-id of the message this replies to
    in_reply_to: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct QueryMailboxInput {
    /// Scope: "workspace" (command inbox/outbox only), "all" (all archives), or an archive name
    /// like "project-intelligence". Defaults to "workspace".
    scope: Option<String>,
    /// Which mailbox to query: "inbox" (default), "outbox", or "both"
    mailbox: Option<String>,
    /// Status filter: "pending" (default), "all", or any status string
    status_filter: Option<String>,
    /// Priority filter: "high", "normal", "low", or "all" (default)
    priority_filter: Option<String>,
    /// Maximum messages to return (default 20)
    limit: Option<u32>,
    /// Include 300-char body preview in results (default false)
    include_body: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct CastVerdictInput {
    /// UUID of the apprenticeship brief being reviewed
    brief_id: String,
    /// UUID of the shadow-captured attempt being judged
    attempt_id: String,
    /// Verdict: "accept" | "refine" | "reject" | "defer-tier-c"
    verdict: String,
    /// Optional human-readable review notes (single line)
    notes: Option<String>,
    /// Optional SHA256 of the corrected diff (for accept/refine verdicts)
    final_diff_sha: Option<String>,
    /// Optional free-form prose appended after the YAML frontmatter in the signed body
    prose: Option<String>,
    /// Senior identity override: "jwoodfine" or "pwoodfine".
    /// Defaults to the currently active identity from identity/.toggle.
    senior_identity: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct GetServiceStatusInput {
    /// Include apprenticeship queue counts from Doorman GET /v1/status/queue (default true)
    include_apprenticeship: Option<bool>,
    /// Include filesystem-level directory counts for cross-verification (default false)
    include_fs_counts: Option<bool>,
    /// Include audit-ledger entry count for the current calendar month (default false)
    include_audit_summary: Option<bool>,
}

// ── Mailbox parsing helper ────────────────────────────────────────────────────

#[derive(Debug, Default)]
struct MailboxMsg {
    fields: HashMap<String, String>,
    body: String,
}

impl MailboxMsg {
    fn get(&self, k: &str) -> &str {
        self.fields.get(k).map(|s| s.as_str()).unwrap_or("")
    }

    fn is_message(&self) -> bool {
        self.fields.contains_key("from") && self.fields.contains_key("re")
    }

    fn to_json(&self, include_body: bool) -> serde_json::Value {
        let mut m = serde_json::json!({
            "msg_id": self.get("msg-id"),
            "from": self.get("from"),
            "to": self.get("to"),
            "re": self.get("re"),
            "priority": self.get("priority"),
            "status": self.get("status"),
            "created": self.get("created"),
        });
        if include_body && !self.body.is_empty() {
            let preview: String = self.body.chars().take(300).collect();
            m["body_preview"] = serde_json::Value::String(preview);
        }
        m
    }
}

fn parse_mailbox(content: &str) -> Vec<MailboxMsg> {
    let mut messages: Vec<MailboxMsg> = Vec::new();
    let mut cur: Option<MailboxMsg> = None;
    let mut in_fm = false; // currently inside --- frontmatter ---

    for line in content.lines() {
        if line.trim() == "---" {
            if !in_fm {
                // Starting a new frontmatter block — finalize previous message if any
                if let Some(msg) = cur.take() {
                    if msg.is_message() {
                        messages.push(msg);
                    }
                }
                cur = Some(MailboxMsg::default());
                in_fm = true;
            } else {
                // Ending frontmatter, body follows
                in_fm = false;
            }
        } else if in_fm {
            if let Some(ref mut msg) = cur {
                if let Some((k, v)) = line.split_once(": ") {
                    msg.fields
                        .insert(k.trim().to_string(), v.trim().to_string());
                }
            }
        } else if let Some(ref mut msg) = cur {
            if !msg.body.is_empty() || !line.is_empty() {
                msg.body.push_str(line);
                msg.body.push('\n');
            }
        }
    }

    if let Some(msg) = cur {
        if msg.is_message() {
            messages.push(msg);
        }
    }

    messages
}

fn read_file_opt(path: &PathBuf) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

// ── Option C: SQLite mailbox index ────────────────────────────────────────────
//
// Dual-write store: send_mailbox_message writes here after mailbox-send.sh
// succeeds; query_mailbox backfills rows that have msg-ids.
// The .md files remain canonical — this is a performance/query index only.

const MAILBOX_DB_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS messages (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    msg_id      TEXT,
    archive     TEXT NOT NULL,
    mailbox     TEXT NOT NULL,
    from_field  TEXT,
    to_field    TEXT,
    re          TEXT,
    priority    TEXT,
    status      TEXT,
    created     TEXT,
    body        TEXT,
    indexed_at  TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_msg_id ON messages(msg_id)
    WHERE msg_id IS NOT NULL AND msg_id != '';
CREATE INDEX IF NOT EXISTS idx_archive_status ON messages(archive, status);
";

fn mailbox_db_insert(
    db_path: &std::path::Path,
    archive: &str,
    mailbox: &str,
    msg_id: &str,
    from: &str,
    to: &str,
    re: &str,
    priority: &str,
    status: &str,
    created: &str,
    body: &str,
) -> rusqlite::Result<()> {
    let conn = rusqlite::Connection::open(db_path)?;
    conn.execute_batch(MAILBOX_DB_SCHEMA)?;
    let now = chrono::Utc::now().to_rfc3339();
    let msg_id_opt: Option<&str> = if msg_id.is_empty() { None } else { Some(msg_id) };
    conn.execute(
        "INSERT OR IGNORE INTO messages \
         (msg_id, archive, mailbox, from_field, to_field, re, priority, status, created, body, indexed_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        rusqlite::params![
            msg_id_opt, archive, mailbox, from, to, re, priority, status, created, body, now
        ],
    )?;
    Ok(())
}

// ── Server struct ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct FoundryServer {
    client: reqwest::Client,
    doorman_url: String,
    module_id: String,
    foundry_root: PathBuf,
    mailbox_db_path: PathBuf,
}

impl FoundryServer {
    fn new(doorman_url: String, module_id: String, foundry_root: PathBuf) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .expect("reqwest client init");
        let mailbox_db_path = foundry_root.join("data").join("mailbox.db");
        Self {
            client,
            doorman_url,
            module_id,
            foundry_root,
            mailbox_db_path,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.doorman_url, path)
    }

    fn agent_path(&self, archive: Option<&str>) -> PathBuf {
        match archive {
            Some(name) => self.foundry_root.join("clones").join(name).join(".agent"),
            None => self.foundry_root.join(".agent"),
        }
    }
}

// ── Tool implementations ──────────────────────────────────────────────────────

#[tool_router]
impl FoundryServer {
    /// Query the Foundry DataGraph for entity context.
    ///
    /// Forwards to `POST /v1/graph/query` on the Doorman.
    /// Returns a JSON array of entity context objects.
    /// Set format_for_prompt=true to get a pre-formatted [ENTITY CONTEXT] block
    /// for direct injection into ask_local prompts.
    #[tool(
        description = "Query the Foundry DataGraph for entity context. Returns matching entities \
        and their attributes. Set format_for_prompt=true to get a pre-formatted [ENTITY CONTEXT] \
        block ready to include in an ask_local prompt."
    )]
    async fn query_datagraph(
        &self,
        rmcp::handler::server::wrapper::Parameters(p): rmcp::handler::server::wrapper::Parameters<
            QueryDatagraphInput,
        >,
    ) -> String {
        let limit = p.limit.unwrap_or(10);
        let body = serde_json::json!({ "q": p.q, "limit": limit });
        match self
            .client
            .post(self.url("/v1/graph/query"))
            .header("X-Foundry-Module-ID", &self.module_id)
            .json(&body)
            .send()
            .await
        {
            Ok(resp) => match resp.json::<serde_json::Value>().await {
                Ok(v) => {
                    if p.format_for_prompt.unwrap_or(false) {
                        let entities = v.as_array().cloned().unwrap_or_default();
                        let count = entities.len();
                        let mut block = String::from("[ENTITY CONTEXT]\n");
                        for e in &entities {
                            if let Some(name) = e.get("entity_name").and_then(|x| x.as_str()) {
                                block.push_str(&format!("Name: {name}\n"));
                            }
                            if let Some(cls) = e.get("classification").and_then(|x| x.as_str()) {
                                block.push_str(&format!("Classification: {cls}\n"));
                            }
                            block.push('\n');
                        }
                        block.push_str("[/ENTITY CONTEXT]");
                        serde_json::to_string_pretty(&serde_json::json!({
                            "entities": entities,
                            "entity_count": count,
                            "prompt_context_block": block,
                        }))
                        .unwrap_or_else(|_| "{}".into())
                    } else {
                        serde_json::to_string_pretty(&v).unwrap_or_else(|_| "{}".into())
                    }
                }
                Err(e) => format!("[ERROR] failed to parse response: {e}"),
            },
            Err(e) => format!("[ERROR] graph query failed: {e}"),
        }
    }

    /// Mutate the Foundry DataGraph (create / update / delete entities).
    ///
    /// Forwards `mutation` payload to `POST /v1/graph/mutate` on the Doorman.
    #[tool(
        description = "Mutate the Foundry DataGraph. Provide a mutation object matching the service-content mutate schema."
    )]
    async fn mutate_datagraph(
        &self,
        rmcp::handler::server::wrapper::Parameters(p): rmcp::handler::server::wrapper::Parameters<
            MutateDatagraphInput,
        >,
    ) -> String {
        match self
            .client
            .post(self.url("/v1/graph/mutate"))
            .header("X-Foundry-Module-ID", &self.module_id)
            .json(&p.mutation)
            .send()
            .await
        {
            Ok(resp) => {
                let status = resp.status().as_u16();
                match resp.json::<serde_json::Value>().await {
                    Ok(v) => format!(
                        "HTTP {status}\n{}",
                        serde_json::to_string_pretty(&v).unwrap_or_else(|_| "{}".into())
                    ),
                    Err(e) => format!("[ERROR] HTTP {status} — failed to parse response: {e}"),
                }
            }
            Err(e) => format!("[ERROR] graph mutate failed: {e}"),
        }
    }

    /// Fetch rich entity enrichment context by entity name.
    ///
    /// Convenience wrapper around query_datagraph scoped to a single entity.
    #[tool(
        description = "Fetch full entity enrichment context from the DataGraph by entity name or identifier."
    )]
    async fn get_entity_context(
        &self,
        rmcp::handler::server::wrapper::Parameters(p): rmcp::handler::server::wrapper::Parameters<
            GetEntityContextInput,
        >,
    ) -> String {
        let body = serde_json::json!({ "q": p.entity, "limit": 5 });
        match self
            .client
            .post(self.url("/v1/graph/query"))
            .header("X-Foundry-Module-ID", &self.module_id)
            .json(&body)
            .send()
            .await
        {
            Ok(resp) => match resp.json::<serde_json::Value>().await {
                Ok(v) => format!(
                    "Entity context for '{}':\n{}",
                    p.entity,
                    serde_json::to_string_pretty(&v).unwrap_or_else(|_| "[]".into())
                ),
                Err(e) => format!("[ERROR] failed to parse entity context: {e}"),
            },
            Err(e) => format!("[ERROR] entity context fetch failed: {e}"),
        }
    }

    /// [deprecated: use get_doorman_status] Get training corpus statistics and daily cost.
    #[tool(
        description = "[deprecated: use get_doorman_status] Retrieve Foundry corpus statistics and daily inference cost summary from the Doorman."
    )]
    async fn get_corpus_stats(&self) -> String {
        let health_fut = self.client.get(self.url("/healthz")).send();
        let cost_fut = self.client.get(self.url("/v1/cost/daily")).send();
        let (health_res, cost_res) = tokio::join!(health_fut, cost_fut);

        let health_str = match health_res {
            Ok(r) => r.text().await.unwrap_or_else(|_| "?".into()),
            Err(e) => format!("unreachable: {e}"),
        };
        let cost_str = match cost_res {
            Ok(r) => match r.json::<serde_json::Value>().await {
                Ok(v) => serde_json::to_string_pretty(&v).unwrap_or_else(|_| "{}".into()),
                Err(e) => format!("parse error: {e}"),
            },
            Err(e) => format!("unreachable: {e}"),
        };

        format!("Doorman health: {health_str}\n\nDaily cost:\n{cost_str}")
    }

    /// Submit a prose document to the Foundry entity extraction pipeline.
    ///
    /// Forwards to `POST /v1/extract` on the Doorman.
    /// Requires a JSON Schema that constrains the output entity array.
    /// Returns extracted entities or a deferred status if Yo-Yo is unavailable.
    #[tool(
        description = "Submit a prose document for entity extraction via the Foundry pipeline. Requires a JSON Schema for the output entity array."
    )]
    async fn submit_extraction(
        &self,
        rmcp::handler::server::wrapper::Parameters(p): rmcp::handler::server::wrapper::Parameters<
            SubmitExtractionInput,
        >,
    ) -> String {
        let module_id = p.module_id.as_deref().unwrap_or(&self.module_id);
        let body = serde_json::json!({
            "text": p.text,
            "schema": p.schema,
            "module_id": module_id,
        });
        match self
            .client
            .post(self.url("/v1/extract"))
            .json(&body)
            .send()
            .await
        {
            Ok(resp) => {
                let status = resp.status().as_u16();
                match resp.json::<serde_json::Value>().await {
                    Ok(v) => format!(
                        "HTTP {status}\n{}",
                        serde_json::to_string_pretty(&v).unwrap_or_else(|_| "{}".into())
                    ),
                    Err(e) => format!("[ERROR] HTTP {status} — parse error: {e}"),
                }
            }
            Err(e) => format!("[ERROR] extraction request failed: {e}"),
        }
    }

    /// [deprecated: use get_doorman_status] Check Doorman health: tier availability and circuit state.
    #[tool(
        description = "[deprecated: use get_doorman_status] Check Doorman health including tier availability (A/B/C), readiness, and circuit breaker state."
    )]
    async fn doorman_health(&self) -> String {
        let healthz_fut = self.client.get(self.url("/healthz")).send();
        let readyz_fut = self.client.get(self.url("/readyz")).send();
        let contract_fut = self.client.get(self.url("/v1/contract")).send();
        let (healthz, readyz, contract) = tokio::join!(healthz_fut, readyz_fut, contract_fut);

        let hz = match healthz {
            Ok(r) => r.text().await.unwrap_or_else(|_| "?".into()),
            Err(e) => format!("unreachable: {e}"),
        };
        let rz = match readyz {
            Ok(r) => {
                let status = r.status().as_u16();
                let body = r.text().await.unwrap_or_else(|_| "?".into());
                format!("HTTP {status} — {body}")
            }
            Err(e) => format!("unreachable: {e}"),
        };
        let ct = match contract {
            Ok(r) => match r.json::<serde_json::Value>().await {
                Ok(v) => serde_json::to_string_pretty(&v).unwrap_or_else(|_| "{}".into()),
                Err(e) => format!("parse error: {e}"),
            },
            Err(e) => format!("unreachable: {e}"),
        };

        format!("/healthz: {hz}\n/readyz: {rz}\n\nContract:\n{ct}")
    }

    /// Submit a prompt to the local OLMo model and return its response.
    ///
    /// Calls `POST /v1/chat/completions` on the Doorman, which routes to Tier A
    /// (local OLMo). Data never leaves the VM (SYS-ADR-07 compliant).
    /// Graph context injection is active — the Doorman automatically injects
    /// [ENTITY CONTEXT] from service-content before forwarding to the model.
    #[tool(
        description = "Submit a prompt to the local OLMo 7B model via the Doorman. \
        Returns the model response plus tier, inference time, and cost. \
        Graph context injection is active — the Doorman automatically prepends \
        [ENTITY CONTEXT] from the DataGraph before routing to the model. \
        No data leaves the VM (SYS-ADR-07 compliant)."
    )]
    async fn ask_local(
        &self,
        rmcp::handler::server::wrapper::Parameters(p): rmcp::handler::server::wrapper::Parameters<
            AskLocalInput,
        >,
    ) -> String {
        let max_tokens = p.max_tokens.unwrap_or(300).min(400);
        let body = serde_json::json!({
            "messages": [{"role": "user", "content": p.prompt}],
            "stream": false,
            "max_tokens": max_tokens,
        });
        match self
            .client
            .post(self.url("/v1/chat/completions"))
            .timeout(std::time::Duration::from_secs(180))
            .header("X-Foundry-Module-ID", &self.module_id)
            .json(&body)
            .send()
            .await
        {
            Ok(resp) => match resp.json::<serde_json::Value>().await {
                Ok(v) => {
                    let content = v["content"].as_str().unwrap_or("").to_string();
                    let tier = v["tier_used"].as_str().unwrap_or("?");
                    let model = v["model"].as_str().unwrap_or("?");
                    let ms = v["inference_ms"].as_u64().unwrap_or(0);
                    let cost = v["cost_usd"].as_f64().unwrap_or(0.0);
                    format!("{content}\n\n---\ntier={tier} model={model} inference={ms}ms cost=${cost:.6}")
                }
                Err(e) => format!("[ERROR] parse failed: {e}"),
            },
            Err(e) => format!("[ERROR] request failed: {e}"),
        }
    }

    // ── Sprint 4: Session efficiency + mailbox tools ──────────────────────────

    /// Get a structured session brief at startup — replaces reading 5+ files manually.
    ///
    /// Returns inbox pending count + first N messages, outbox summary, NOTAM status,
    /// session context digest, and workspace state flags. Saves 3,000–8,000 tokens
    /// per session by avoiding raw file reads.
    #[tool(
        description = "Get a structured session brief at startup. Returns inbox pending count, \
        first N messages with body previews, outbox summary, NOTAM status, session context digest, \
        and workspace state flags. Use instead of reading inbox.md, outbox.md, NOTAM.md, \
        session-context.md, and workspace-state.md separately."
    )]
    async fn get_session_brief(
        &self,
        rmcp::handler::server::wrapper::Parameters(p): rmcp::handler::server::wrapper::Parameters<
            GetSessionBriefInput,
        >,
    ) -> String {
        let role = p.role.as_deref().unwrap_or("command");
        let archive = p.archive.as_deref();
        let limit = p.pending_limit.unwrap_or(5) as usize;

        let agent = self.agent_path(if role == "totebox" { archive } else { None });

        // Read inbox
        let inbox_content = read_file_opt(&agent.join("inbox.md")).unwrap_or_default();
        let all_inbox = parse_mailbox(&inbox_content);
        let pending_inbox: Vec<&MailboxMsg> = all_inbox
            .iter()
            .filter(|m| {
                let s = m.get("status");
                s == "pending" || s == "in-progress" || s == "operator-pending"
            })
            .collect();
        let inbox_pending_count = pending_inbox.len();
        let messages_json: Vec<serde_json::Value> = pending_inbox
            .iter()
            .take(limit)
            .map(|m| m.to_json(true))
            .collect();
        let inbox_overflow = inbox_pending_count.saturating_sub(limit);

        // Read outbox
        let outbox_content = read_file_opt(&agent.join("outbox.md")).unwrap_or_default();
        let all_outbox = parse_mailbox(&outbox_content);
        let pending_outbox: Vec<&MailboxMsg> = all_outbox
            .iter()
            .filter(|m| m.get("status") == "pending")
            .collect();
        let outbox_pending_count = pending_outbox.len();
        let outbox_subjects: Vec<&str> =
            pending_outbox.iter().take(5).map(|m| m.get("re")).collect();

        // Read NOTAM
        let notam_active = read_file_opt(&self.foundry_root.join("NOTAM.md"))
            .map(|c| c.contains("[ACTIVE]") || c.contains("ACTIVE HAZARD"))
            .unwrap_or(false);

        // Read session-context digest (first 600 chars of last session entry)
        let session_context_digest =
            read_file_opt(&agent.join("memory").join("session-context.md"))
                .map(|c| c.chars().take(600).collect::<String>())
                .unwrap_or_else(|| "(not found)".into());

        // Read workspace-state flags (command only)
        let workspace_state_flags: Vec<String> = if role == "command" {
            read_file_opt(&agent.join("workspace-state.md"))
                .map(|c| {
                    c.lines()
                        .filter(|l| {
                            l.contains("CRITICAL") || l.contains("outbox=") || l.contains("⚠")
                        })
                        .take(10)
                        .map(|l| l.trim().to_string())
                        .collect()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        // Find carry-forward items from NEXT.md (unchecked boxes)
        let carry_forward: Vec<String> = read_file_opt(&self.foundry_root.join("NEXT.md"))
            .map(|c| {
                c.lines()
                    .filter(|l| l.trim().starts_with("- [ ]"))
                    .take(5)
                    .map(|l| l.trim()[5..].trim().to_string())
                    .collect()
            })
            .unwrap_or_default();

        serde_json::to_string_pretty(&serde_json::json!({
            "role": role,
            "archive": archive,
            "notam_active": notam_active,
            "inbox_pending_count": inbox_pending_count,
            "inbox_messages": messages_json,
            "inbox_overflow": inbox_overflow,
            "outbox_pending_count": outbox_pending_count,
            "outbox_subjects": outbox_subjects,
            "session_context_digest": session_context_digest,
            "workspace_state_flags": workspace_state_flags,
            "carry_forward_open": carry_forward,
        }))
        .unwrap_or_else(|e| format!("[ERROR] serialization failed: {e}"))
    }

    /// Send a mailbox message via the canonical bin/mailbox-send.sh path.
    ///
    /// Writes the message to the target archive's inbox.md with proper YAML frontmatter
    /// and appends an audit entry to data/mailbox-ledger.jsonl (M-10).
    /// This is the canonical write path — use it instead of hand-editing mailbox files.
    #[tool(
        description = "Send a mailbox message to a Command or Totebox archive inbox. \
        Routes through bin/mailbox-send.sh for M-2 misroute validation and M-10 audit ledger. \
        Use this instead of manually composing YAML frontmatter. \
        to must be 'command@claude-code' or 'totebox@<archive-name>'."
    )]
    async fn send_mailbox_message(
        &self,
        rmcp::handler::server::wrapper::Parameters(p): rmcp::handler::server::wrapper::Parameters<
            SendMailboxMessageInput,
        >,
    ) -> String {
        let script = self.foundry_root.join("bin").join("mailbox-send.sh");
        if !script.exists() {
            return format!("[ERROR] mailbox-send.sh not found at {}", script.display());
        }

        let priority = p.priority.as_deref().unwrap_or("normal");
        let mut cmd = tokio::process::Command::new("bash");
        cmd.arg(script.as_os_str())
            .arg("--to")
            .arg(&p.to)
            .arg("--re")
            .arg(&p.re)
            .arg("--priority")
            .arg(priority)
            .arg("--body-stdin")
            .current_dir(&self.foundry_root)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        if let Some(ref reply_to) = p.in_reply_to {
            cmd.arg("--in-reply-to").arg(reply_to);
        }

        match cmd.spawn() {
            Err(e) => format!("[ERROR] failed to spawn mailbox-send.sh: {e}"),
            Ok(mut child) => {
                // Write body to stdin
                if let Some(mut stdin) = child.stdin.take() {
                    use tokio::io::AsyncWriteExt;
                    let _ = stdin.write_all(p.body.as_bytes()).await;
                }

                match child.wait_with_output().await {
                    Err(e) => format!("[ERROR] mailbox-send.sh failed: {e}"),
                    Ok(output) => {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        if !output.status.success() {
                            return format!("[ERROR] mailbox-send.sh exit {}\nstdout: {stdout}\nstderr: {stderr}",
                                output.status.code().unwrap_or(-1));
                        }
                        // Parse msg-id from output: "  ✓ <archive> (msg-id: <ID>)"
                        let msg_id = stdout
                            .lines()
                            .find(|l| l.contains("msg-id:"))
                            .and_then(|l| {
                                let start = l.find("msg-id:")? + 7;
                                let s = l[start..].trim();
                                let end = s.find(')').unwrap_or(s.len());
                                Some(s[..end].trim().to_string())
                            })
                            .unwrap_or_else(|| "(unknown)".into());

                        let (dest_file, archive_name) = match p.to.as_str() {
                            "command@claude-code" => (
                                self.foundry_root.join(".agent").join("inbox.md"),
                                "workspace".to_string(),
                            ),
                            to if to.starts_with("totebox@") => {
                                let archive = &to["totebox@".len()..];
                                (
                                    self.foundry_root
                                        .join("clones")
                                        .join(archive)
                                        .join(".agent")
                                        .join("inbox.md"),
                                    archive.to_string(),
                                )
                            }
                            _ => (PathBuf::from("(unknown)"), "unknown".to_string()),
                        };

                        // Option C: dual-write to SQLite mailbox index
                        {
                            let db_path = self.mailbox_db_path.clone();
                            let msg_id_c = msg_id.clone();
                            let to_c = p.to.clone();
                            let re_c = p.re.clone();
                            let body_c = p.body.clone();
                            let priority_c = priority.to_string();
                            let created_c = chrono::Utc::now().to_rfc3339();
                            let _ = tokio::task::spawn_blocking(move || {
                                mailbox_db_insert(
                                    &db_path, &archive_name, "inbox",
                                    &msg_id_c, "", &to_c, &re_c,
                                    &priority_c, "pending", &created_c, &body_c,
                                )
                            })
                            .await;
                        }

                        serde_json::to_string_pretty(&serde_json::json!({
                            "ok": true,
                            "msg_id": msg_id,
                            "dest_file": dest_file.display().to_string(),
                            "ledger_written": true,
                            "stdout": stdout.trim(),
                        }))
                        .unwrap_or_else(|e| format!("[ERROR] {e}"))
                    }
                }
            }
        }
    }

    /// Get a comprehensive Doorman status snapshot in one call.
    ///
    /// Replaces calling doorman_health() and get_corpus_stats() separately.
    /// Handles /readyz returning 404 gracefully (known bug; binary rebuild pending).
    #[tool(
        description = "Get comprehensive Doorman status: tier A/B/C availability, queue depths, \
        daily cost, and throughput in one call. Replaces doorman_health() + get_corpus_stats(). \
        Degrades gracefully if /readyz returns 404 (known bug; binary rebuild pending)."
    )]
    async fn get_doorman_status(&self) -> String {
        let healthz_fut = self
            .client
            .get(self.url("/healthz"))
            .timeout(std::time::Duration::from_secs(5))
            .send();
        let readyz_fut = self
            .client
            .get(self.url("/readyz"))
            .timeout(std::time::Duration::from_secs(5))
            .send();
        let flow_fut = self
            .client
            .get(self.url("/v1/status/flow"))
            .timeout(std::time::Duration::from_secs(5))
            .send();
        let cost_fut = self
            .client
            .get(self.url("/v1/status/cost"))
            .timeout(std::time::Duration::from_secs(5))
            .send();

        let (healthz, readyz, flow, cost) =
            tokio::join!(healthz_fut, readyz_fut, flow_fut, cost_fut);

        let (doorman_reachable, healthz_body) = match healthz {
            Ok(r) => (true, r.text().await.unwrap_or_else(|_| "ok".into())),
            Err(_) => (false, "unreachable".into()),
        };

        let (readyz_ok, readyz_body) = match readyz {
            Ok(r) => {
                let s = r.status().as_u16();
                let body = r.text().await.unwrap_or_default();
                (s == 200, format!("HTTP {s} {body}"))
            }
            Err(e) => (false, format!("unreachable: {e}")),
        };

        let flow_val: serde_json::Value = match flow {
            Ok(r) => r.json().await.unwrap_or(serde_json::Value::Null),
            Err(_) => serde_json::Value::Null,
        };

        let cost_val: serde_json::Value = match cost {
            Ok(r) => r.json().await.unwrap_or(serde_json::Value::Null),
            Err(_) => serde_json::Value::Null,
        };

        // Extract well-known fields with fallbacks
        let tier_a_ready = flow_val
            .get("tier_a_ready")
            .and_then(|v| v.as_bool())
            .unwrap_or(doorman_reachable);
        let tier_b_ready = flow_val
            .get("tier_b_ready")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let tier_b_reason = flow_val
            .get("tier_b_reason")
            .and_then(|v| v.as_str())
            .unwrap_or(if tier_b_ready { "up" } else { "not available" });
        let tier_c_ready = flow_val
            .get("tier_c_ready")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let queue_pending = flow_val
            .get("queue_pending")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let queue_poison = flow_val
            .get("queue_poison")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let tok_per_s = flow_val
            .get("tier_a_tok_per_s")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let daily_usd = cost_val
            .get("daily_usd")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let daily_requests = cost_val
            .get("daily_request_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        serde_json::to_string_pretty(&serde_json::json!({
            "doorman_reachable": doorman_reachable,
            "healthz": healthz_body,
            "readyz_ok": readyz_ok,
            "readyz_note": if !readyz_ok { "known bug: /readyz 404 until binary rebuild" } else { "" },
            "tier_a_ready": tier_a_ready,
            "tier_b_ready": tier_b_ready,
            "tier_b_reason": tier_b_reason,
            "tier_c_ready": tier_c_ready,
            "queue_pending": queue_pending,
            "queue_poison": queue_poison,
            "tier_a_tok_per_s": tok_per_s,
            "daily_usd": daily_usd,
            "daily_requests": daily_requests,
            "readyz_raw": readyz_body,
        }))
        .unwrap_or_else(|e| format!("[ERROR] {e}"))
    }

    /// Query mailboxes across archives — replaces 23+ Read calls for a full sweep.
    ///
    /// scope="all" scans every archive clone. priority_filter="high" surfaces urgent items.
    #[tool(
        description = "Query mailboxes across one or all archives. scope='all' returns messages \
        from every archive in one call. Filters by status (default: pending) and priority. \
        Replaces 23+ Read tool calls for a full Command Session sweep."
    )]
    async fn query_mailbox(
        &self,
        rmcp::handler::server::wrapper::Parameters(p): rmcp::handler::server::wrapper::Parameters<
            QueryMailboxInput,
        >,
    ) -> String {
        let scope = p.scope.as_deref().unwrap_or("workspace");
        let mailbox_target = p.mailbox.as_deref().unwrap_or("inbox");
        let status_filter = p.status_filter.as_deref().unwrap_or("pending");
        let priority_filter = p.priority_filter.as_deref().unwrap_or("all");
        let limit = p.limit.unwrap_or(20) as usize;
        let include_body = p.include_body.unwrap_or(false);

        // Determine which agent paths to scan
        let mut agent_paths: Vec<(String, PathBuf)> = Vec::new();

        match scope {
            "workspace" => {
                agent_paths.push(("workspace".into(), self.foundry_root.join(".agent")));
            }
            "all" => {
                agent_paths.push(("workspace".into(), self.foundry_root.join(".agent")));
                let clones_dir = self.foundry_root.join("clones");
                if let Ok(entries) = std::fs::read_dir(&clones_dir) {
                    for entry in entries.flatten() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        let agent = entry.path().join(".agent");
                        if agent.exists() {
                            agent_paths.push((name, agent));
                        }
                    }
                }
            }
            archive_name => {
                let agent = self
                    .foundry_root
                    .join("clones")
                    .join(archive_name)
                    .join(".agent");
                agent_paths.push((archive_name.to_string(), agent));
            }
        }

        let mut all_messages: Vec<serde_json::Value> = Vec::new();
        let mut archives_with_pending: Vec<String> = Vec::new();
        let archives_scanned = agent_paths.len();

        for (archive_name, agent_path) in &agent_paths {
            let files: Vec<(&str, PathBuf)> = match mailbox_target {
                "inbox" => vec![("inbox", agent_path.join("inbox.md"))],
                "outbox" => vec![("outbox", agent_path.join("outbox.md"))],
                _ => vec![
                    ("inbox", agent_path.join("inbox.md")),
                    ("outbox", agent_path.join("outbox.md")),
                ],
            };

            let mut found_in_archive = false;
            for (mbox_name, path) in files {
                let content = read_file_opt(&path).unwrap_or_default();
                let messages = parse_mailbox(&content);
                for msg in &messages {
                    let status = msg.get("status");
                    let priority = msg.get("priority");

                    let status_ok = status_filter == "all" || status == status_filter;
                    let priority_ok = priority_filter == "all" || priority == priority_filter;

                    if status_ok && priority_ok {
                        found_in_archive = true;
                        let mut m = msg.to_json(include_body);
                        m["archive"] = serde_json::Value::String(archive_name.clone());
                        m["mailbox"] = serde_json::Value::String(mbox_name.to_string());
                        all_messages.push(m);
                    }
                }
            }

            if found_in_archive {
                archives_with_pending.push(archive_name.clone());
            }
        }

        let total_matched = all_messages.len();
        all_messages.truncate(limit);

        // Option C: backfill SQLite index from messages parsed out of .md files.
        // Only rows with a non-empty msg-id are backfilled (idempotent via UNIQUE index).
        if !all_messages.is_empty() {
            let db_path = self.mailbox_db_path.clone();
            let rows: Vec<[String; 9]> = all_messages
                .iter()
                .filter_map(|m| {
                    let msg_id = m["msg_id"].as_str().unwrap_or("").to_string();
                    if msg_id.is_empty() {
                        return None;
                    }
                    Some([
                        msg_id,
                        m["archive"].as_str().unwrap_or("").to_string(),
                        m.get("mailbox")
                            .and_then(|v| v.as_str())
                            .unwrap_or("inbox")
                            .to_string(),
                        m["from"].as_str().unwrap_or("").to_string(),
                        m["to"].as_str().unwrap_or("").to_string(),
                        m["re"].as_str().unwrap_or("").to_string(),
                        m["priority"].as_str().unwrap_or("normal").to_string(),
                        m["status"].as_str().unwrap_or("pending").to_string(),
                        m["created"].as_str().unwrap_or("").to_string(),
                    ])
                })
                .collect();
            if !rows.is_empty() {
                let _ = tokio::task::spawn_blocking(move || {
                    for row in &rows {
                        let _ = mailbox_db_insert(
                            &db_path, &row[1], &row[2], &row[0], &row[3],
                            &row[4], &row[5], &row[6], &row[7], &row[8], "",
                        );
                    }
                })
                .await;
            }
        }

        serde_json::to_string_pretty(&serde_json::json!({
            "total_matched": total_matched,
            "returned": all_messages.len(),
            "messages": all_messages,
            "archives_scanned": archives_scanned,
            "archives_with_pending": archives_with_pending,
        }))
        .unwrap_or_else(|e| format!("[ERROR] {e}"))
    }

    // ── Sprint 5: Apprenticeship verdict + service status ─────────────────────

    /// Cast a signed apprenticeship verdict on a shadow-captured attempt.
    ///
    /// Builds the YAML verdict body, signs it with the active staging identity key
    /// via `ssh-keygen -Y sign`, base64-encodes the signature, and POSTs the
    /// `VerdictWireBody` to `POST /v1/verdict` on the Doorman.
    ///
    /// On success, the Doorman writes a DPO corpus tuple and optionally promotes
    /// the attempt to LoRA training queue. Returns the dispatch outcome.
    #[tool(
        description = "Cast a signed apprenticeship verdict (accept|refine|reject|defer-tier-c) \
        on a shadow-captured attempt. Signs the verdict body with the active staging identity key \
        and POSTs to Doorman POST /v1/verdict. Returns DPO pair status and promotion outcome. \
        Use query_mailbox or get_session_brief to find pending brief_id/attempt_id values."
    )]
    async fn cast_apprenticeship_verdict(
        &self,
        rmcp::handler::server::wrapper::Parameters(p): rmcp::handler::server::wrapper::Parameters<
            CastVerdictInput,
        >,
    ) -> String {
        use base64::Engine as _;

        const VALID_VERDICTS: &[&str] = &["accept", "refine", "reject", "defer-tier-c"];
        if !VALID_VERDICTS.contains(&p.verdict.as_str()) {
            return format!(
                "[ERROR] invalid verdict '{}'; must be one of: accept, refine, reject, defer-tier-c",
                p.verdict
            );
        }

        // Determine senior identity from toggle or override
        let identity: String = if let Some(ref id) = p.senior_identity {
            if id != "jwoodfine" && id != "pwoodfine" {
                return format!(
                    "[ERROR] invalid senior_identity '{}'; must be 'jwoodfine' or 'pwoodfine'",
                    id
                );
            }
            id.clone()
        } else {
            let toggle = read_file_opt(&self.foundry_root.join("identity").join(".toggle"))
                .unwrap_or_default();
            if toggle.trim() == "1" {
                "pwoodfine".into()
            } else {
                "jwoodfine".into()
            }
        };

        let key_path = self
            .foundry_root
            .join("identity")
            .join(&identity)
            .join(format!("id_{}", identity));
        if !key_path.exists() {
            return format!("[ERROR] identity key not found at {}", key_path.display());
        }

        // Build ISO 8601 timestamp
        let created = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

        // Build YAML body matching parse_verdict_body in slm-doorman/src/verdict.rs
        let notes = p.notes.as_deref().unwrap_or("");
        let sha = p.final_diff_sha.as_deref().unwrap_or("");
        let mut body = format!(
            "---\nbrief_id: {}\nattempt_id: {}\nverdict: {}\ncreated: {}\n\
             senior_identity: {}\nfinal_diff_sha: {}\nnotes: {}\n---\n",
            p.brief_id, p.attempt_id, p.verdict, created, identity, sha, notes
        );
        if let Some(ref prose) = p.prose {
            body.push_str(prose);
            body.push('\n');
        }

        // Write body to temp file (ssh-keygen reads from a file, not stdin)
        let tmp_body = std::env::temp_dir().join(format!("slm-verdict-{}.txt", std::process::id()));
        if let Err(e) = std::fs::write(&tmp_body, body.as_bytes()) {
            return format!("[ERROR] failed to write body to temp file: {e}");
        }

        // Sign with ssh-keygen — creates <tmp_body>.sig
        let sign_out = tokio::process::Command::new("ssh-keygen")
            .arg("-Y")
            .arg("sign")
            .arg("-f")
            .arg(&key_path)
            .arg("-n")
            .arg("apprenticeship-verdict-v1")
            .arg(&tmp_body)
            .output()
            .await;

        let sign_ok = match sign_out {
            Err(e) => {
                let _ = std::fs::remove_file(&tmp_body);
                return format!("[ERROR] failed to spawn ssh-keygen: {e}");
            }
            Ok(out) => out,
        };

        if !sign_ok.status.success() {
            let _ = std::fs::remove_file(&tmp_body);
            let stderr = String::from_utf8_lossy(&sign_ok.stderr);
            return format!("[ERROR] ssh-keygen -Y sign failed: {stderr}");
        }

        // Read the .sig file ssh-keygen created
        let mut sig_path_str = tmp_body.to_string_lossy().to_string();
        sig_path_str.push_str(".sig");
        let sig_path = PathBuf::from(&sig_path_str);

        let pem_bytes = match std::fs::read(&sig_path) {
            Ok(b) => b,
            Err(e) => {
                let _ = std::fs::remove_file(&tmp_body);
                return format!(
                    "[ERROR] could not read sig file {}: {e}",
                    sig_path.display()
                );
            }
        };

        let _ = std::fs::remove_file(&tmp_body);
        let _ = std::fs::remove_file(&sig_path);

        let signature_b64 = base64::engine::general_purpose::STANDARD.encode(&pem_bytes);

        // POST VerdictWireBody to Doorman
        let wire = serde_json::json!({
            "body": body,
            "signature": signature_b64,
            "senior_identity": identity,
        });

        match self
            .client
            .post(self.url("/v1/verdict"))
            .header("X-Foundry-Module-ID", &self.module_id)
            .json(&wire)
            .send()
            .await
        {
            Err(e) => format!("[ERROR] POST /v1/verdict failed: {e}"),
            Ok(resp) => {
                let status = resp.status().as_u16();
                match resp.json::<serde_json::Value>().await {
                    Ok(v) => {
                        format!(
                            "HTTP {status}\n{}",
                            serde_json::to_string_pretty(&v).unwrap_or_else(|_| "{}".into())
                        )
                    }
                    Err(e) => format!("[ERROR] HTTP {status} — response parse error: {e}"),
                }
            }
        }
    }

    /// Get Foundry service status: apprenticeship queue, extraction state, audit ledger.
    ///
    /// Calls `GET /v1/status/queue` for Doorman-side queue counts.
    /// Optionally reads filesystem directory entry counts as verification.
    /// Optionally counts audit-ledger entries for the current calendar month.
    #[tool(
        description = "Get Foundry service status: apprenticeship queue counts from \
        Doorman GET /v1/status/queue, optional filesystem directory verification, \
        and optional audit-ledger entry count for the current month. \
        Set include_fs_counts=true to cross-check Doorman counts against disk."
    )]
    async fn get_service_status(
        &self,
        rmcp::handler::server::wrapper::Parameters(p): rmcp::handler::server::wrapper::Parameters<
            GetServiceStatusInput,
        >,
    ) -> String {
        let include_apprenticeship = p.include_apprenticeship.unwrap_or(true);
        let include_fs = p.include_fs_counts.unwrap_or(false);
        let include_audit = p.include_audit_summary.unwrap_or(false);

        let mut result = serde_json::json!({});

        if include_apprenticeship {
            let resp = self
                .client
                .get(self.url("/v1/status/queue"))
                .timeout(std::time::Duration::from_secs(5))
                .send()
                .await;
            result["apprenticeship_queue"] = match resp {
                Ok(r) => r
                    .json::<serde_json::Value>()
                    .await
                    .unwrap_or_else(|e| serde_json::json!({ "parse_error": e.to_string() })),
                Err(e) => serde_json::json!({ "error": e.to_string() }),
            };
        }

        if include_fs {
            let base = self.foundry_root.join("data").join("apprenticeship");
            let count_dir = |sub: &str| -> usize {
                std::fs::read_dir(base.join(sub))
                    .map(|e| e.count())
                    .unwrap_or(0)
            };
            result["fs_queue_counts"] = serde_json::json!({
                "queue": count_dir("queue"),
                "queue_done": count_dir("queue-done"),
                "queue_poison": count_dir("queue-poison"),
                "queue_in_flight": count_dir("queue-in-flight"),
                "queue_paused": count_dir("queue-paused"),
            });
        }

        if include_audit {
            let month = chrono::Utc::now().format("%Y-%m").to_string();
            let ledger = self
                .foundry_root
                .join("data")
                .join("audit-ledger")
                .join("workspace")
                .join(format!("{}.jsonl", month));
            let count = read_file_opt(&ledger)
                .map(|c| c.lines().filter(|l| !l.trim().is_empty()).count())
                .unwrap_or(0);
            result["audit_ledger"] = serde_json::json!({
                "month": month,
                "entry_count": count,
                "path": ledger.display().to_string(),
            });
        }

        serde_json::to_string_pretty(&result).unwrap_or_else(|e| format!("[ERROR] {e}"))
    }
}

#[tool_handler(
    name = "slm-mcp-server",
    version = "0.3.0",
    instructions = "Foundry MCP server — DataGraph, corpus, Doorman, mailbox, and apprenticeship tools"
)]
impl ServerHandler for FoundryServer {}

// ── Entry point ───────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    // Parse --doorman flag
    let args: Vec<String> = std::env::args().collect();
    let doorman_url = args
        .windows(2)
        .find(|w| w[0] == "--doorman")
        .map(|w| w[1].clone())
        .or_else(|| std::env::var("SLM_DOORMAN_URL").ok())
        .unwrap_or_else(|| "http://127.0.0.1:9080".to_string());

    let module_id = std::env::var("SLM_MODULE_ID").unwrap_or_else(|_| "mcp-foundry".to_string());
    let foundry_root: PathBuf = std::env::var("FOUNDRY_ROOT")
        .unwrap_or_else(|_| "/srv/foundry".to_string())
        .into();

    tracing::info!(
        doorman_url = %doorman_url,
        module_id = %module_id,
        foundry_root = %foundry_root.display(),
        "slm-mcp-server starting"
    );

    let server = FoundryServer::new(doorman_url, module_id, foundry_root);
    let transport = (tokio::io::stdin(), tokio::io::stdout());
    server.serve(transport).await?.waiting().await?;

    Ok(())
}
