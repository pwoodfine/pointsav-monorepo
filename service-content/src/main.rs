mod config_http;
mod entity_filter;
mod er;
mod graph;
mod http;
mod taxonomy;

use graph::{GraphEntity, GraphStore, LbugGraphStore};
use notify::{Event, RecursiveMode, Result as NotifyResult, Watcher};
use serde_json::Value;
use std::fs;
use std::io::{BufRead, Write};
use std::path::Path;
use std::sync::mpsc::RecvTimeoutError;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[derive(Debug)]
enum ExtractResult {
    Success,
    DeferTransient,
    DeferCircuitOpen,
    Failed,
}

// Classification vocabulary defined in entity_filter::ALLOWED_CLASSIFICATIONS — single source of truth.

/// System prompt shared between Tier A and Tier B extraction calls.
const EXTRACTION_SYSTEM_PROMPT: &str = "Extract named entities from the text below. Classify each entity into exactly one category.\n\
Categories:\n\
  Person    — a named human individual who appears in the text.\n\
  Company   — a registered organisation or business named in the text.\n\
  Project   — a named software crate, infrastructure service, or engineering initiative named in the text.\n\
  Account   — a named financial account, service account reference, or contract identifier in the text.\n\
  Location  — a specific named geographic place or address (city, region, street address). NOT a generic spatial noun.\n\
Omit:\n\
  - Software licences and SPDX identifiers (Apache-2.0, MIT, GPL-3.0, BSL-1.1). These are not companies.\n\
  - Programming languages, file formats, and protocol names (Rust, JSON, HTTP) unless they name a specific product.\n\
  - Shell environment variables and config symbols: $VAR_NAME, SLM_DATA_DIR, FOUNDRY_ARCHIVE_NAME — OMIT.\n\
  - Code identifiers: backtick-quoted terms (`ghi_kwh_m2_yr`), snake_case names without spaces (service_content), file paths (./build.sh, src/main.rs, create-snapshot.sh), call expressions (log(x), ops(slm), func()), and build tool commands (cargo, npm, make, git). OMIT ALL, including any project name appearing as a CLI argument (-p slm-doorman-server, --crate service-content).\n\
  - Commit-message prefixes of the form type(scope): ops(slm), feat(cache), fix(auth), chore(db) — these are NOT projects or accounts. OMIT.\n\
  - Statistical notation (α, β, γ, R², p-value) and mathematical symbols.\n\
  - Laws, regulations, and dates.\n\
  - Generic technical concepts not attached to a proper name: \"software-as-a-service (SaaS)\", \"Hyperscaler\", \"real-time operating system (RTOS)\", \"distributed ledger technology\". These are descriptors, not entities. OMIT.\n\
  - Placeholder values: \"not specified\", \"N/A\", \"unknown\", \"TBD\", \"none\", \"null\". OMIT.\n\
  - Generic spatial or role phrases that are not proper place names. A Location must be a specific named place.\n\
    EXCLUDE: \"retail anchor location\", \"downtown core\", \"the site\", \"trade area\".\n\
    INCLUDE: \"Murfreesboro, Tennessee\", \"Billings, Montana\", \"Chicago\".\n\
  - Sentence fragments, clauses, or lists: any name containing a comma, \" and \", or starting with \"the\", \"a\", \"an\", \"this\". OMIT.\n\
  - Any entity whose name appears only in these instructions, not in the text.\n\
Country names: when a country appears as an entity, classify it as Location, NEVER as Company. \"Portugal\" → Location.\n\
Hard constraint: entity_name must be a short proper noun or proper-noun phrase. Maximum eight words.\n\
A token that looks like a proper noun is not automatically an entity. If it is a licence, a format, a generic descriptor, or a code identifier, omit it rather than forcing it into Company or Location.\n\
If an entity does not clearly fit one category, omit it rather than guessing.\n\
Return only a JSON array. Each element MUST have \"classification\" and \"entity_name\". You MAY add \"role_vector\" (a person's stated title or role), \"location_vector\" (a stated place of work or residence), or \"contact_vector\" (a stated email or phone) — but ONLY when the text explicitly states that attribute for that entity. Omit the field otherwise. NEVER invent a vector value; an absent attribute is omitted, not guessed.\n\
\n\
Examples:\n\
Text: Jennifer Woodfine is managing director at Woodfine Management Corp. in Vancouver, Canada.\n\
Output: [{\"classification\":\"Person\",\"entity_name\":\"Jennifer Woodfine\",\"role_vector\":\"managing director\"},{\"classification\":\"Company\",\"entity_name\":\"Woodfine Management Corp.\"},{\"classification\":\"Location\",\"entity_name\":\"Vancouver\"}]\n\
\n\
Text: The cluster contains service-fs, not service-research. Let me explore the actual structure.\n\
Output: [{\"classification\":\"Project\",\"entity_name\":\"service-fs\"}]\n\
\n\
Text: ops(slm): update drain predicate — remove !tier_a_first guard\n\
Output: []\n\
\n\
Text: Woodfine Management Corp. uses service-content and service-slm for extraction; service-bim is not yet active.\n\
Output: [{\"classification\":\"Company\",\"entity_name\":\"Woodfine Management Corp.\"},{\"classification\":\"Project\",\"entity_name\":\"service-content\"},{\"classification\":\"Project\",\"entity_name\":\"service-slm\"}]\n\
\n\
Text: The panic is at service-slm/crates/slm-doorman-server/src/http.rs:1302:9.\n\
Output: []\n\
\n\
Text: Run cargo clippy -p slm-doorman-server -- -D warnings to check for lint errors.\n\
Output: []\n\
\n\
Text: Peter Woodfine approved moving the yoyo-batch instance to us-central1-b.\n\
Output: [{\"classification\":\"Person\",\"entity_name\":\"Peter Woodfine\"},{\"classification\":\"Location\",\"entity_name\":\"us-central1-b\"}]\n\
\n\
Text: The automation bot triggered the outbox status check and corpus pipeline.\n\
Output: []\n\
\n\
If no entities are found, return an empty array [].";

fn main() -> NotifyResult<()> {
    println!("================================================================");
    println!("[SYSTEM] PointSav Semantic Watcher (Rust Edition) Activated");
    println!("[SYSTEM] Protocol: Schema Expansion Routing");
    println!("================================================================");

    let doorman_endpoint = std::env::var("SLM_DOORMAN_ENDPOINT")
        .unwrap_or_else(|_| "http://127.0.0.1:9080".to_string());
    let infrastructure_root =
        std::env::var("INFRASTRUCTURE_ROOT").unwrap_or_else(|_| "/srv/foundry".to_string());
    let base_dir = std::env::var("SERVICE_CONTENT_BASE_DIR")
        .unwrap_or_else(|_| format!("{}/data", infrastructure_root));
    let module_id =
        std::env::var("SERVICE_CONTENT_MODULE_ID").unwrap_or_else(|_| "woodfine".to_string());

    // Ontology directory: service-content/ontology/ relative to the binary's parent,
    // or overridden via SERVICE_CONTENT_ONTOLOGY_DIR.
    let ontology_dir = std::env::var("SERVICE_CONTENT_ONTOLOGY_DIR").unwrap_or_else(|_| {
        // Default: sibling ontology/ directory relative to the binary location.
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .map(|p| p.join("ontology").to_string_lossy().to_string())
            .unwrap_or_else(|| "ontology".to_string())
    });

    let feedback_dir = std::env::var("SERVICE_CONTENT_FEEDBACK_DIR")
        .unwrap_or_else(|_| format!("{}/training-corpus/feedback", base_dir));

    println!("[SYSTEM] Doorman endpoint: {}", doorman_endpoint);
    println!("[SYSTEM] Base dir: {}", base_dir);
    println!("[SYSTEM] Module ID: {}", module_id);
    println!("[SYSTEM] Ontology dir: {}", ontology_dir);
    println!("[SYSTEM] Feedback dir (enrichment DPO): {}", feedback_dir);

    let corpus_dir = format!("{}/service-content/ledgers", base_dir);
    let crm_dir = format!("{}/service-people/ledgers", base_dir);

    if !Path::new(&corpus_dir).exists() {
        fs::create_dir_all(&corpus_dir).unwrap_or_else(|e| {
            eprintln!("[FATAL] Cannot create corpus dir {:?}: {e}", corpus_dir);
            std::process::exit(1);
        });
    }
    if !Path::new(&crm_dir).exists() {
        fs::create_dir_all(&crm_dir).unwrap_or_else(|e| {
            eprintln!("[FATAL] Cannot create CRM dir {:?}: {e}", crm_dir);
            std::process::exit(1);
        });
    }

    // ── Graph store initialisation ────────────────────────────────────────────
    let graph_dir = std::env::var("SERVICE_CONTENT_GRAPH_DIR")
        .unwrap_or_else(|_| format!("{}/service-content/graph", base_dir));
    fs::create_dir_all(&graph_dir).unwrap_or_else(|e| {
        eprintln!("[FATAL] Cannot create graph dir {:?}: {e}", graph_dir);
        std::process::exit(1);
    });
    let graph_db_path = format!("{}/entities.lbug", graph_dir);

    let graph_store: Arc<dyn GraphStore> = Arc::new(
        LbugGraphStore::new(&graph_db_path).expect("[SYSTEM] Failed to open LadybugDB graph store"),
    );
    graph_store
        .init_schema()
        .expect("[SYSTEM] Failed to initialise graph schema");
    println!("[SYSTEM] Graph store ready: {}", graph_db_path);

    // ── Startup taxonomy load ─────────────────────────────────────────────────
    match taxonomy::load_taxonomy_from_dir(&ontology_dir) {
        Ok(bundle) => {
            let entities = taxonomy::bundle_to_entities(&bundle);
            let total = entities.len();
            match graph_store.upsert_entities("__taxonomy__", &entities) {
                Ok(n) => println!(
                    "[TAXONOMY] Loaded: {} archetypes, {} coa-profiles, {} domains, \
                     {} glossary-terms, {} themes, {} topics, {} guides → {} entities upserted",
                    bundle.archetypes.len(),
                    bundle.coa.len(),
                    bundle.domains.len(),
                    bundle.glossary.len(),
                    bundle.themes.len(),
                    bundle.topics.len(),
                    bundle.guides.len(),
                    n
                ),
                Err(e) => println!("[TAXONOMY] Graph write failed: {}", e),
            }
            let _ = total;
        }
        Err(e) => println!("[TAXONOMY] Load failed (non-fatal): {}", e),
    }

    // ── HTTP server (dedicated thread + own tokio runtime) ───────────────────
    // Cannot use reqwest::blocking inside a #[tokio::main] context (nested
    // runtime panic). Keep main synchronous; HTTP server owns its own runtime.
    let http_bind =
        std::env::var("SERVICE_CONTENT_HTTP_BIND").unwrap_or_else(|_| "127.0.0.1:9081".to_string());
    let graph_for_http = Arc::clone(&graph_store);
    let doorman_for_http = doorman_endpoint.clone();
    let ontology_for_http = ontology_dir.clone();
    let corpus_dir_for_http = corpus_dir.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to build HTTP tokio runtime");
        rt.block_on(http::run_server(
            graph_for_http,
            http_bind,
            doorman_for_http,
            ontology_for_http,
            corpus_dir_for_http,
        ));
    });

    // ── SC-3: Doorman startup health-check ───────────────────────────────────
    // Poll /healthz for up to 30 s before the first drain. If Doorman is
    // unreachable at boot, all CORPUS files would be deferred and lost until
    // restart. A brief wait here avoids that silent data-loss pattern.
    {
        let health_url = format!("{}/healthz", doorman_endpoint);
        let client = reqwest::blocking::Client::new();
        let mut wait_secs = 1u64;
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        let mut ready = false;
        while std::time::Instant::now() < deadline {
            match client
                .get(&health_url)
                .timeout(Duration::from_secs(2))
                .send()
            {
                Ok(r) if r.status().is_success() => {
                    ready = true;
                    break;
                }
                _ => {}
            }
            thread::sleep(Duration::from_secs(wait_secs));
            wait_secs = (wait_secs * 2).min(8);
        }
        if ready {
            println!("[SYSTEM] Doorman ready.");
        } else {
            println!("[SYSTEM] Warning: Doorman unreachable after 30 s — CORPUS drain will proceed (files may defer to retry queue).");
        }
    }

    // ── Process any pre-existing CORPUS_* files ───────────────────────────────
    // processed_ledgers: permanently done (success, or a hard parse/extract failure
    //   that retrying cannot fix)
    // deferred_ledgers: transient defer — retried every 30 s by the watcher timeout
    // circuit_deferred_ledgers: deferred because the Tier B (Yo-Yo) circuit is open.
    //   Held dormant — NOT retried every tick (that would storm the Doorman with the
    //   whole backlog during a GPU stockout). A single recovery probe per tick drains
    //   this list back into deferred_ledgers once Tier B is reachable again, so the
    //   backlog resumes WITHOUT a service restart. (Preemption-safe: nothing skipped.)
    let processed_ledgers_path = Path::new(&graph_dir).join("processed_ledgers.jsonl");
    let mut processed_ledgers = load_processed_ledgers(&processed_ledgers_path);
    if !processed_ledgers.is_empty() {
        println!(
            "[SYSTEM] Loaded {} previously-processed CORPUS entries from ledger — skipping on drain.",
            processed_ledgers.len()
        );
    }
    let max_defer_retries: u32 = std::env::var("SLM_MAX_DEFER_RETRIES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);
    let mut deferred_counts: std::collections::HashMap<String, u32> =
        std::collections::HashMap::new();

    // ── Parallel startup drain ────────────────────────────────────────────────
    // Collects unprocessed CORPUS files, then processes them with N concurrent
    // worker threads. CONTENT_DRAIN_THREADS defaults to 4.
    // LbugGraphStore is Send+Sync (each call opens a fresh Connection from
    // Arc<Database>); the HTTP server thread already calls it concurrently, so
    // N drain workers are safe without additional locking on the store itself.
    let (mut deferred_ledgers, mut circuit_deferred_ledgers) = {
        use std::collections::VecDeque;
        use std::sync::Mutex;

        let drain_threads: usize = std::env::var("CONTENT_DRAIN_THREADS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(4)
            .max(1);

        let queue: Arc<Mutex<VecDeque<std::path::PathBuf>>> = Arc::new(Mutex::new(
            fs::read_dir(Path::new(&corpus_dir))
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("json"))
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n.starts_with("CORPUS_") && !processed_ledgers.contains(n))
                        .unwrap_or(false)
                })
                .collect(),
        ));

        let done_files: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let defer_files: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let circ_files: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        // Serialize append_processed_ledger across workers (O_APPEND + fsync per entry).
        let ledger_lock: Arc<Mutex<()>> = Arc::new(Mutex::new(()));

        let mut handles = Vec::new();
        for _ in 0..drain_threads {
            let q = Arc::clone(&queue);
            let gs = Arc::clone(&graph_store);
            let (cd, de, mid, fd) = (
                crm_dir.clone(),
                doorman_endpoint.clone(),
                module_id.clone(),
                feedback_dir.clone(),
            );
            let (lp, ll) = (processed_ledgers_path.clone(), Arc::clone(&ledger_lock));
            let (d1, d2, d3) = (
                Arc::clone(&done_files),
                Arc::clone(&defer_files),
                Arc::clone(&circ_files),
            );
            handles.push(thread::spawn(move || loop {
                let Some(path) = q.lock().unwrap().pop_front() else {
                    break;
                };
                let fname = match path.file_name().and_then(|n| n.to_str()) {
                    Some(n) => n.to_string(),
                    None => continue,
                };
                match process_corpus(&path, &cd, &de, &mid, &gs, &fd) {
                    ExtractResult::Success | ExtractResult::Failed => {
                        let _g = ll.lock().unwrap();
                        append_processed_ledger(&lp, &fname);
                        drop(_g);
                        d1.lock().unwrap().push(fname);
                    }
                    ExtractResult::DeferCircuitOpen => d3.lock().unwrap().push(fname),
                    ExtractResult::DeferTransient => d2.lock().unwrap().push(fname),
                }
            }));
        }
        for h in handles {
            let _ = h.join();
        }
        processed_ledgers.extend(done_files.lock().unwrap().drain(..));
        let mut defer_guard = defer_files.lock().unwrap();
        let mut circ_guard = circ_files.lock().unwrap();
        (
            std::mem::take(&mut *defer_guard),
            std::mem::take(&mut *circ_guard),
        )
    };

    // ── Watcher loop (blocking — runs on the main task) ───────────────────────
    // Uses recv_timeout so the loop wakes every 30 s to retry deferred files.
    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = notify::recommended_watcher(tx)?;
    watcher.watch(Path::new(&corpus_dir), RecursiveMode::NonRecursive)?;

    println!("================================================================");
    println!("[SYSTEM] Active Kernel Surveillance Engaged on Corpus Plane...");

    loop {
        // Adaptive poll: 3 s when deferred files are waiting (10× faster retry during
        // Doorman recovery), 30 s when idle (no pending backlog).
        let poll_interval = if deferred_ledgers.is_empty() && circuit_deferred_ledgers.is_empty() {
            Duration::from_secs(30)
        } else {
            Duration::from_secs(3)
        };
        match rx.recv_timeout(poll_interval) {
            Ok(Ok(Event { paths, .. })) => {
                for path in paths {
                    if let Some(extension) = path.extension() {
                        if extension == "json" {
                            let Some(filename) = path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .map(|s| s.to_string())
                            else {
                                eprintln!(
                                    "[WARN] Skipping file with non-UTF8 or missing name: {:?}",
                                    path
                                );
                                continue;
                            };
                            if filename.starts_with("CORPUS_")
                                && !processed_ledgers.contains(&filename)
                                && !deferred_ledgers.contains(&filename)
                                && !circuit_deferred_ledgers.contains(&filename)
                            {
                                println!("\n[WATCHER] New Corpus Detected: {}", filename);
                                thread::sleep(Duration::from_millis(250));
                                // Mark in-flight in deferred to prevent double-fire
                                // if the watcher emits multiple events for the same write.
                                deferred_ledgers.push(filename.clone());
                                match process_corpus(
                                    &path,
                                    &crm_dir,
                                    &doorman_endpoint,
                                    &module_id,
                                    &graph_store,
                                    &feedback_dir,
                                ) {
                                    ExtractResult::Success | ExtractResult::Failed => {
                                        deferred_ledgers.retain(|f| f != &filename);
                                        append_processed_ledger(&processed_ledgers_path, &filename);
                                        processed_ledgers.insert(filename);
                                    }
                                    ExtractResult::DeferCircuitOpen => {
                                        deferred_ledgers.retain(|f| f != &filename);
                                        circuit_deferred_ledgers.push(filename);
                                    }
                                    ExtractResult::DeferTransient => {
                                        // stays in deferred_ledgers; retried on next timeout
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Ok(_) => {}
            Err(RecvTimeoutError::Timeout) => {
                // Retry transient-deferred files
                if !deferred_ledgers.is_empty() {
                    println!(
                        "[RETRY] Retrying {} deferred CORPUS file(s)...",
                        deferred_ledgers.len()
                    );
                }
                let retry_queue: Vec<String> = std::mem::take(&mut deferred_ledgers);
                for filename in retry_queue {
                    let path = Path::new(&corpus_dir).join(&filename);
                    match process_corpus(
                        &path,
                        &crm_dir,
                        &doorman_endpoint,
                        &module_id,
                        &graph_store,
                        &feedback_dir,
                    ) {
                        ExtractResult::Success | ExtractResult::Failed => {
                            append_processed_ledger(&processed_ledgers_path, &filename);
                            processed_ledgers.insert(filename);
                        }
                        ExtractResult::DeferCircuitOpen => {
                            // Tier B went down for this file — move it to the
                            // dormant circuit-deferred list (no per-tick storm).
                            circuit_deferred_ledgers.push(filename);
                        }
                        ExtractResult::DeferTransient => {
                            let count = deferred_counts.entry(filename.clone()).or_insert(0);
                            *count += 1;
                            if *count >= max_defer_retries {
                                eprintln!(
                                    "[WARN] {} reached max defer retries ({}); moving to dead-letter",
                                    filename, max_defer_retries
                                );
                                append_processed_ledger(&processed_ledgers_path, &filename);
                                processed_ledgers.insert(filename);
                            } else {
                                deferred_ledgers.push(filename);
                            }
                        }
                    }
                }

                // Tier-B recovery probe. Files deferred because the Yo-Yo circuit
                // is open sit dormant (not retried every tick — that would storm
                // the Doorman with the whole backlog during a GPU stockout). Each
                // tick we attempt exactly ONE of them: while Tier B is down the
                // probe fast-fails (circuit-open) and is returned to the list; the
                // moment Tier B recovers the probe succeeds (or defers transiently)
                // and the entire dormant backlog is promoted back into the active
                // retry queue — extraction resumes with no service restart.
                if !circuit_deferred_ledgers.is_empty() {
                    let probe = circuit_deferred_ledgers.remove(0);
                    let probe_path = Path::new(&corpus_dir).join(&probe);
                    match process_corpus(
                        &probe_path,
                        &crm_dir,
                        &doorman_endpoint,
                        &module_id,
                        &graph_store,
                        &feedback_dir,
                    ) {
                        ExtractResult::DeferCircuitOpen => {
                            // Still down — keep it dormant.
                            circuit_deferred_ledgers.push(probe);
                        }
                        outcome => {
                            // Tier B is reachable again. Record the probe's own
                            // result, then resume the dormant backlog.
                            match outcome {
                                ExtractResult::DeferTransient => deferred_ledgers.push(probe),
                                _ => {
                                    append_processed_ledger(&processed_ledgers_path, &probe);
                                    processed_ledgers.insert(probe);
                                }
                            }
                            if !circuit_deferred_ledgers.is_empty() {
                                println!(
                                    "[RECOVERY] Tier B reachable again — resuming {} circuit-deferred CORPUS file(s).",
                                    circuit_deferred_ledgers.len()
                                );
                                deferred_ledgers.append(&mut circuit_deferred_ledgers);
                            }
                        }
                    }
                }
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }

    Ok(())
}

fn load_processed_ledgers(path: &Path) -> std::collections::HashSet<String> {
    let Ok(file) = fs::File::open(path) else {
        return std::collections::HashSet::new();
    };
    std::io::BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

fn append_processed_ledger(path: &Path, filename: &str) {
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{}", filename);
    }
}

// ── Enrichment cascade helpers ────────────────────────────────────────────────

/// Call Tier A (OLMo 7B via /v1/chat/completions) and return raw entity JSON.
/// Returns None when Tier A is unavailable or response is unparseable.
fn call_tier_a_extract(
    corpus_text: &str,
    entity_schema: &serde_json::Value,
    doorman_endpoint: &str,
) -> Option<Vec<serde_json::Value>> {
    // Grammar mode: SERVICE_CONTENT_TIER_A_GRAMMAR=json_schema enables JSON Schema grammar
    // constraint instead of assistant pre-fill. Default is pre-fill (safer for OLMo 2 7B,
    // which returns [] when grammar overrides the pre-fill). OLMo 3 handles grammar correctly;
    // enable after confirming OLMo 3 is loaded via smoke test. GrammarConstraint::JsonSchema
    // serialises as {"type": "json-schema", "value": ...} (kebab-case tag).
    let use_grammar = std::env::var("SERVICE_CONTENT_TIER_A_GRAMMAR")
        .map(|v| v == "json_schema")
        .unwrap_or(false);

    let chat_body = if use_grammar {
        serde_json::json!({
            "messages": [
                {"role": "system", "content": EXTRACTION_SYSTEM_PROMPT},
                {"role": "user",   "content": corpus_text}
            ],
            "grammar": {"type": "json-schema", "value": entity_schema}
        })
    } else {
        serde_json::json!({
            "messages": [
                {"role": "system", "content": EXTRACTION_SYSTEM_PROMPT},
                {"role": "user",   "content": corpus_text},
                {"role": "assistant", "content": "[{\""}
            ]
        })
    };
    let url = format!("{}/v1/chat/completions", doorman_endpoint);
    let client = reqwest::blocking::Client::new();
    match client
        .post(&url)
        .header("X-Foundry-Complexity", "low")
        .json(&chat_body)
        .timeout(Duration::from_secs(180))
        .send()
    {
        Ok(r) if r.status().is_success() => r.json::<serde_json::Value>().ok().and_then(|v| {
            // Doorman envelope: {"content": "...", "tier_used": "local", ...}
            // OpenAI fallback: {"choices": [{"message": {"content": "..."}}]}
            let content = v["content"]
                .as_str()
                .or_else(|| v["choices"][0]["message"]["content"].as_str())?;
            // Reattach assistant pre-fill when llama-server returns only the continuation.
            let owned;
            let content = if content.trim_start().starts_with('[') {
                content
            } else {
                owned = format!("[{{\"{}", content);
                owned.as_str()
            };
            // Strip markdown fences the model may have added
            let stripped = content
                .trim()
                .strip_prefix("```json")
                .unwrap_or(content.trim())
                .strip_prefix("```")
                .unwrap_or(content.trim());
            let stripped = stripped.strip_suffix("```").unwrap_or(stripped).trim();
            serde_json::from_str(stripped).ok()
        }),
        _ => None,
    }
}

/// Convert raw entity JSON values into `GraphEntity` structs.
fn raw_entities_to_graph(
    raw: &[serde_json::Value],
    module_id: &str,
    confidence: f64,
) -> Vec<GraphEntity> {
    raw.iter()
        .filter_map(|ent| {
            let entity_name = ent["entity_name"].as_str()?.to_string();
            let classification = ent["classification"].as_str()?.to_string();
            if entity_name.is_empty() || classification.is_empty() {
                return None;
            }
            // Change 2: deterministic noise filter — rejects env vars, file paths,
            // snake_case identifiers, call expressions, fragments, and placeholders.
            if entity_filter::is_noise_entity_name(&entity_name) {
                return None;
            }
            // Change 5: word-count gate — sentences and clauses are not entity names.
            if entity_name.split_whitespace().count() > 8 {
                return None;
            }
            // Change 4: type-coherence validation — corrects or rejects misclassified
            // entities (country-as-Company, path-as-Project, CAPS-as-Account).
            let classification =
                match entity_filter::coerce_classification(&entity_name, &classification) {
                    Some(cls) => cls,
                    None => return None,
                };
            // Reject out-of-vocabulary classifications. OLMo may emit values such as
            // "Licence" or "Technology" when the prompt omit list is insufficient.
            // Dropping them here prevents bad data from landing in LadybugDB.
            if !entity_filter::ALLOWED_CLASSIFICATIONS.contains(&classification.as_str()) {
                return None;
            }
            Some(GraphEntity {
                entity_name,
                classification,
                role_vector: ent
                    .get("role_vector")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty() && *s != "null")
                    .map(str::to_string),
                location_vector: ent
                    .get("location_vector")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty() && *s != "null")
                    .map(str::to_string),
                contact_vector: ent
                    .get("contact_vector")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty() && *s != "null")
                    .map(str::to_string),
                module_id: module_id.to_string(),
                confidence,
            })
        })
        .collect()
}

/// Write a DPO training pair when Tier B improves on Tier A's extraction.
/// Skipped when: Tier B empty, Tier A empty (no rejected = degenerate pair),
/// or both sides normalize to the same output (no training signal).
///
/// Normalization strips auxiliary hydration fields (role_vector, location_vector,
/// contact_vector) from Tier A output before comparison and serialization.
/// These fields appear in Tier A's GraphStore-hydrated path but are absent from
/// Tier B's raw JSON response — without stripping them almost all pairs appear
/// to differ even when the core extraction result is identical, polluting the corpus.
/// Returns `true` if a DPO pair was durably written, `false` if skipped.
/// Callers gate `mark_sweep_sha_complete` on this return value so the SHA
/// ledger is only marked complete when a training pair was actually saved.
fn write_enrichment_dpo_pair(
    worm_id: &str,
    corpus_text: &str,
    tier_a_raw: &[serde_json::Value],
    tier_b_raw: &[serde_json::Value],
    feedback_dir: &str,
) -> bool {
    // DOC_sweep docs are git commit text — Tier B hallucination rate is too high for
    // reliable DPO signal (source-grounding check catches these downstream, but early
    // return avoids the file-write path entirely). SHA is marked complete at the call
    // site regardless, preventing per-cycle re-submission of the same commit SHAs.
    if worm_id.starts_with("DOC_sweep-") {
        return false;
    }
    if tier_b_raw.is_empty() {
        return false;
    }
    if tier_a_raw.is_empty() {
        return false; // no rejected signal — DPO pair would teach verbosity, not accuracy
    }
    // DPO pre-save validator — applies the SAME filter chain as raw_entities_to_graph:
    // noise rejection + word-count gate + coerce_classification + ALLOWED_CLASSIFICATIONS.
    // Ensures the chosen side of the DPO pair matches what actually lands in LadybugDB.
    let tier_b_clean = entity_filter::clean_dpo_side(tier_b_raw);
    let tier_a_clean = entity_filter::clean_dpo_side(tier_a_raw);
    if tier_b_clean.is_empty() {
        return false; // all Tier B entities were noise — no training signal after cleaning
    }
    if tier_b_clean.len() < tier_a_clean.len() {
        return false; // cleaning made chosen worse than rejected — degenerate pair
    }
    // Shadow-rebind: rest of function operates on cleaned slices.
    let tier_b_raw = tier_b_clean.as_slice();
    let tier_a_raw = tier_a_clean.as_slice();
    // Source-grounding: reject the pair if any Tier B entity name is absent
    // (case-insensitive) from the source corpus text. Prevents Tier B hallucinations
    // (verified: "Woodfine Management Corp.", "service-slm", "Vancouver" fabricated
    // on git commit text in 8/8 sampled pairs) from entering DPO corpus as the
    // preferred (chosen) side — which would train the model toward fabrication.
    let corpus_lower = corpus_text.to_lowercase();
    let all_grounded = tier_b_raw.iter().all(|e| {
        e.get("entity_name")
            .and_then(|v| v.as_str())
            .map(|name| corpus_lower.contains(&name.to_lowercase()))
            .unwrap_or(false)
    });
    if !all_grounded {
        return false; // hallucinated entity in chosen side — discard pair, write nothing
    }
    // Normalize Tier A to {classification, entity_name} only — strips role_vector,
    // location_vector, contact_vector that are absent in Tier B's raw response.
    let tier_a_normalized: Vec<serde_json::Value> = tier_a_raw
        .iter()
        .map(|e| {
            serde_json::json!({
                "classification": e.get("classification").unwrap_or(&serde_json::Value::Null),
                "entity_name":    e.get("entity_name").unwrap_or(&serde_json::Value::Null),
            })
        })
        .collect();
    let tier_a_json = serde_json::to_string(&tier_a_normalized).unwrap_or_default();
    let tier_b_json = serde_json::to_string(tier_b_raw).unwrap_or_default();
    // Canonical comparison: normalize Tier B to same {classification, entity_name} schema and
    // sort both arrays before equality check. Raw string compare fails when models return the
    // same entities in different order → false DPO pair. Pair content (tier_a_json/tier_b_json)
    // is written as-is so training sees the actual model outputs, not the sorted forms.
    let mut tier_b_normalized_cmp: Vec<serde_json::Value> = tier_b_raw
        .iter()
        .map(|e| {
            serde_json::json!({
                "classification": e.get("classification").unwrap_or(&serde_json::Value::Null),
                "entity_name":    e.get("entity_name").unwrap_or(&serde_json::Value::Null),
            })
        })
        .collect();
    let mut tier_a_sorted_cmp = tier_a_normalized.clone();
    tier_a_sorted_cmp.sort_by_key(|x| x.to_string());
    tier_b_normalized_cmp.sort_by_key(|x| x.to_string());
    if serde_json::to_string(&tier_a_sorted_cmp).unwrap_or_default()
        == serde_json::to_string(&tier_b_normalized_cmp).unwrap_or_default()
    {
        return false; // identical after normalization + sort — no training delta
    }
    let prompt = format!("{}\n\nText:\n{}", EXTRACTION_SYSTEM_PROMPT, corpus_text);
    let now = chrono::Utc::now();
    let pair = serde_json::json!({
        "prompt":      prompt,
        "chosen":      tier_b_json,
        "rejected":    tier_a_json,
        "source_type": "datagraph-enrichment",
        "worm_id":     worm_id,
        "timestamp":   now.to_rfc3339(),
    });
    let _ = fs::create_dir_all(feedback_dir);
    let filename = format!(
        "{}/enrichment-{}-{}.jsonl",
        feedback_dir,
        worm_id,
        now.timestamp_millis()
    );
    if let Ok(mut f) = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&filename)
    {
        let _ = writeln!(f, "{}", pair);
        return true;
    }
    false
}

/// Write the git commit SHA to the sweep completion ledger after enrichment succeeds.
/// Only fires for sweep-sourced documents (worm_id prefix: "DOC_sweep-").
/// Ledger path is read from SERVICE_CONTENT_SWEEP_LEDGER env var; no-op if unset.
fn mark_sweep_sha_complete(worm_id: &str) {
    let Some(rest) = worm_id.strip_prefix("DOC_sweep-") else {
        return;
    };
    // worm_id format: DOC_sweep-<sha>_<ts_ms> — strip the trailing _<ts>
    let sha = match rest.rfind('_') {
        Some(pos) => &rest[..pos],
        None => rest,
    };
    if sha.len() < 7 {
        return; // sanity: git SHAs are at least 7 hex chars
    }
    let ledger_path = std::env::var("SERVICE_CONTENT_SWEEP_LEDGER").unwrap_or_default();
    if ledger_path.is_empty() {
        return;
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&ledger_path)
    {
        use std::io::Write;
        let _ = writeln!(f, "{}", sha);
    }
}

fn process_corpus(
    filepath: &Path,
    crm_dir: &str,
    doorman_endpoint: &str,
    module_id: &str,
    graph_store: &Arc<dyn GraphStore>,
    feedback_dir: &str,
) -> ExtractResult {
    // SC-5: log read failures instead of silently returning
    let content = match fs::read_to_string(filepath) {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "  -> [ERROR] Failed to read CORPUS file {:?}: {}",
                filepath, e
            );
            return ExtractResult::Failed;
        }
    };
    let payload: Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "  -> [ERROR] Malformed CORPUS JSON in {:?}: {}",
                filepath, e
            );
            return ExtractResult::Failed;
        }
    };

    // Envelope contract: accept both the native "worm_id"/"corpus" keys (ingest path)
    // and the claude-session-bridge keys "session_id"/"turn_id"/"text" (turn-capture path).
    // Without this, the entire session-bridge corpus class was silently dropped because
    // corpus_text was always empty → ExtractResult::Failed at the empty-text guard below.
    let worm_id_owned: String = payload["worm_id"]
        .as_str()
        .map(|s| s.to_string())
        .or_else(|| {
            let sid = payload["session_id"].as_str()?;
            let tid = payload["turn_id"].as_str()?;
            Some(format!("DOC_session-{}_turn-{}", sid, tid))
        })
        .unwrap_or_else(|| "UNKNOWN".to_string());
    let worm_id = worm_id_owned.as_str();
    let corpus_text = payload["corpus"]
        .as_str()
        .or_else(|| payload["text"].as_str())
        .unwrap_or("");
    // Per-file module_id override: CORPUS JSON may carry a "module_id" field to
    // route workspace artifacts into a separate graph namespace (e.g. "foundry-workspace")
    // without requiring a separate service-content instance.
    let effective_module_id: &str = payload["module_id"]
        .as_str()
        .filter(|s| !s.is_empty())
        .unwrap_or(module_id);

    if corpus_text.is_empty() {
        return ExtractResult::Failed;
    }

    // ── Shared entity schema used by both tiers ───────────────────────────────
    let entity_schema = serde_json::json!({
        "type": "array",
        "items": {
            "type": "object",
            "properties": {
                "entity_name": {"type": "string"},
                "classification": {
                    "type": "string",
                    "enum": ["Person", "Company", "Project", "Account", "Location"]
                },
                "role_vector": {"type": ["string", "null"]},
                "location_vector": {"type": ["string", "null"]},
                "contact_vector": {"type": ["string", "null"]}
            },
            "required": ["entity_name", "classification"]
        }
    });

    // ── Step 1: Tier A extraction (always first — fast local OLMo) ───────────
    let tier_a_raw: Option<Vec<serde_json::Value>> = {
        let result = call_tier_a_extract(corpus_text, &entity_schema, doorman_endpoint);
        match &result {
            Some(ents) => println!(
                "  -> [TIER-A] {} entities extracted (module: {}).",
                ents.len(),
                effective_module_id
            ),
            None => println!("  -> [TIER-A] Unavailable — proceeding to Tier B."),
        }
        result
    };

    // ── Step 2: Tier B extraction (OLMo 32B via /v1/extract) ─────────────────
    println!(
        "  -> [TIER-B] Routing to Doorman ({})/v1/extract...",
        doorman_endpoint
    );

    let body = serde_json::json!({
        "text": corpus_text,
        "schema": entity_schema,
        "module_id": effective_module_id
    });

    let url = format!("{}/v1/extract", doorman_endpoint);
    let client = reqwest::blocking::Client::new();
    let res = client
        .post(&url)
        .header("X-Foundry-Priority", "p1")
        .json(&body)
        .timeout(Duration::from_secs(300))
        .send();

    // Helper: flush Tier A entities to graph when Tier B is unavailable.
    let flush_tier_a = |tier_a: &Option<Vec<serde_json::Value>>, reason: &str| -> ExtractResult {
        if let Some(ta_ents) = tier_a {
            if !ta_ents.is_empty() {
                let ge = raw_entities_to_graph(ta_ents, effective_module_id, 0.75);
                match graph_store.upsert_entities(effective_module_id, &ge) {
                    Ok(n) => {
                        println!("  -> [TIER-A] {} entities written ({}).", n, reason);
                        return ExtractResult::Success;
                    }
                    Err(e) => eprintln!("  -> [TIER-A] Graph write failed: {}", e),
                }
            }
        }
        ExtractResult::DeferCircuitOpen
    };

    match res {
        Ok(response) => {
            if !response.status().is_success() {
                println!(
                    "  -> [SYS_HALT] Doorman rejected payload: {}",
                    response.status()
                );
                return flush_tier_a(&tier_a_raw, "Tier B rejected");
            }

            let extract_resp = match response.json::<serde_json::Value>() {
                Ok(v) => v,
                Err(_) => {
                    println!("  -> [SYS_HALT] Doorman returned invalid JSON.");
                    return flush_tier_a(&tier_a_raw, "Tier B parse failed");
                }
            };

            // SC-2: differentiate defer reasons.
            if extract_resp["deferred"].as_bool().unwrap_or(false) {
                let reason = extract_resp["defer_reason"].as_str().unwrap_or("unknown");
                return match reason {
                    "yoyo-circuit-open" => {
                        println!("  -> [TIER-B] Circuit open — using Tier A results.");
                        flush_tier_a(&tier_a_raw, "Tier B circuit-open")
                    }
                    _ => {
                        // Transient: use Tier A if available, otherwise retry
                        if let Some(ta_ents) = &tier_a_raw {
                            if !ta_ents.is_empty() {
                                let ge = raw_entities_to_graph(ta_ents, effective_module_id, 0.75);
                                if let Ok(n) = graph_store.upsert_entities(effective_module_id, &ge)
                                {
                                    println!(
                                        "  -> [TIER-A] {} entities written (Tier B transient; module: {}).",
                                        n, effective_module_id
                                    );
                                    return ExtractResult::Success;
                                }
                            }
                        }
                        println!("  -> [TIER-B] Deferred ({}): retry in 30 s.", reason);
                        ExtractResult::DeferTransient
                    }
                };
            }

            if !extract_resp["extraction_ok"].as_bool().unwrap_or(false) {
                println!("  -> [SYS_HALT] Extraction failed: extraction_ok false.");
                return flush_tier_a(&tier_a_raw, "Tier B extraction_ok=false");
            }

            // ── Tier B succeeded ─────────────────────────────────────────────
            let semantic_entities = extract_resp["entities"]
                .as_array()
                .cloned()
                .unwrap_or_default();

            let graph_entities =
                raw_entities_to_graph(&semantic_entities, effective_module_id, 0.95);

            // Build legacy CRM record
            let mut enriched_crm = Vec::new();
            for ent in &semantic_entities {
                let entity_name = ent["entity_name"].as_str().unwrap_or("").to_string();
                let classification = ent["classification"].as_str().unwrap_or("").to_string();
                let role_vector = ent
                    .get("role_vector")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty() && *s != "null")
                    .map(str::to_string);
                let location_vector = ent
                    .get("location_vector")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty() && *s != "null")
                    .map(str::to_string);
                let contact_vector = ent
                    .get("contact_vector")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty() && *s != "null")
                    .map(str::to_string);

                let mut new_ent = serde_json::Map::new();
                new_ent.insert("entity_name".to_string(), serde_json::json!(entity_name));
                new_ent.insert(
                    "classification".to_string(),
                    serde_json::json!(classification),
                );
                new_ent.insert(
                    "role_vector".to_string(),
                    role_vector
                        .as_deref()
                        .map(|s| serde_json::json!(s))
                        .unwrap_or(serde_json::json!("UNVERIFIED")),
                );
                new_ent.insert("confidence".to_string(), serde_json::json!(0.95));
                new_ent.insert(
                    "context_anchor".to_string(),
                    serde_json::json!("SLM NEURAL INFERENCE"),
                );
                let loc = location_vector
                    .as_deref()
                    .map(|s| serde_json::json!(s))
                    .unwrap_or(serde_json::json!("UNVERIFIED"));
                new_ent.insert("location_vector".to_string(), loc);
                let mut latent = Vec::new();
                if let Some(contact) = contact_vector.as_deref() {
                    if contact.contains('@') {
                        latent.push(format!("mailto:{}", contact));
                    } else {
                        latent.push(format!("tel:{}", contact));
                    }
                }
                new_ent.insert("latent_vectors".to_string(), serde_json::json!(latent));
                enriched_crm.push(Value::Object(new_ent));
            }

            let semantic_ledger = serde_json::json!({
                "worm_id": format!("{}_SEMANTIC", worm_id),
                "source_asset": "SLM_INFERENCE",
                "extracted_crm_entities": enriched_crm
            });

            // Graph write first; CRM second — keeps stores consistent.
            if let Err(e) = graph_store.upsert_entities(effective_module_id, &graph_entities) {
                println!("  -> [GRAPH] Write failed: {}", e);
                return ExtractResult::Failed;
            }
            println!(
                "  -> [GRAPH] {} entities written to graph (module: {}).",
                graph_entities.len(),
                effective_module_id
            );

            let out_file = format!("{}/SEMANTIC_{}.json", crm_dir, worm_id);
            if let Err(e) = fs::write(&out_file, semantic_ledger.to_string()) {
                eprintln!("[ERROR] CRM write failed after graph upsert: {e}");
                return ExtractResult::Failed;
            }
            println!(
                "  -> [WATCHER] Semantic Integration Complete: {} Nodes Secured.",
                enriched_crm.len()
            );

            // Write enrichment DPO pair after both graph and CRM are durably committed.
            // For sweep docs (DOC_sweep-*): write_enrichment_dpo_pair returns false early
            // (policy: skip pair generation for commit text), but mark the SHA complete
            // unconditionally — otherwise the same commit SHAs re-submit every nightly cycle.
            if let Some(ref ta_ents) = tier_a_raw {
                let saved = write_enrichment_dpo_pair(
                    worm_id,
                    corpus_text,
                    ta_ents,
                    &semantic_entities,
                    feedback_dir,
                );
                if saved || worm_id.starts_with("DOC_sweep-") {
                    mark_sweep_sha_complete(worm_id);
                }
            }

            ExtractResult::Success
        }
        Err(e) => {
            // Transport error — Tier B unreachable. Use Tier A to avoid losing the document.
            println!("  -> [SYS_HALT] Doorman routing failed (transient): {}", e);
            let result = flush_tier_a(&tier_a_raw, &format!("Tier B transport error: {}", e));
            if matches!(result, ExtractResult::DeferCircuitOpen) {
                ExtractResult::DeferTransient
            } else {
                result
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn tmp_dir(suffix: &str) -> std::path::PathBuf {
        let ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let dir = std::env::temp_dir().join(format!("sc-test-{}-{}", suffix, ms));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn raw_entities_rejects_snake_case_noise() {
        let raw = vec![
            serde_json::json!({"entity_name": "SLM_DATA_DIR", "classification": "Account"}),
            serde_json::json!({"entity_name": "Jennifer Woodfine", "classification": "Person"}),
        ];
        let result = raw_entities_to_graph(&raw, "jennifer", 0.75);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].entity_name, "Jennifer Woodfine");
    }

    #[test]
    fn raw_entities_rejects_overlong_fragment() {
        let raw = vec![serde_json::json!({
            "entity_name": "a system that extracts entities from documents in the pipeline",
            "classification": "Project"
        })];
        let result = raw_entities_to_graph(&raw, "jennifer", 0.75);
        assert!(result.is_empty());
    }

    #[test]
    fn raw_entities_coerces_country_to_location() {
        let raw = vec![serde_json::json!({
            "entity_name": "Portugal",
            "classification": "Company"
        })];
        let result = raw_entities_to_graph(&raw, "jennifer", 0.75);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].classification, "Location");
    }

    #[test]
    fn dpo_pair_drops_commit_prefix_noise() {
        // ops(slm) on Tier B chosen side should cause the pair to be dropped.
        let tier_b = vec![serde_json::json!({
            "classification": "Project",
            "entity_name": "ops(slm)"
        })];
        let tier_a = vec![serde_json::json!({
            "classification": "Person",
            "entity_name": "Peter Woodfine"
        })];
        let dir = tmp_dir("commit-prefix");
        write_enrichment_dpo_pair(
            "DOC_test-dpo_001",
            "Peter Woodfine committed ops(slm) changes.",
            &tier_a,
            &tier_b,
            dir.to_str().unwrap(),
        );
        assert_eq!(
            std::fs::read_dir(&dir).unwrap().count(),
            0,
            "pair whose chosen side is all commit-prefix noise must be dropped"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn source_grounding_drops_hallucinated_entity() {
        // Tier B claims "Vancouver" but corpus text contains no such word.
        // The pair must be silently dropped (no file written).
        let tier_b = vec![serde_json::json!({
            "classification": "Location",
            "entity_name": "Vancouver"
        })];
        let tier_a = vec![serde_json::json!({
            "classification": "Person",
            "entity_name": "Peter Woodfine"
        })];
        let dir = tmp_dir("hallucinated");
        write_enrichment_dpo_pair(
            "DOC_test-abc_123",
            "Peter Woodfine committed a fix to the logging module.",
            &tier_a,
            &tier_b,
            dir.to_str().unwrap(),
        );
        assert_eq!(
            fs::read_dir(&dir).unwrap().count(),
            0,
            "pair with hallucinated entity must be dropped"
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn source_grounding_allows_grounded_entity() {
        // Tier B returns "Peter Woodfine" which IS present in the corpus text.
        // The pair must be written.
        let tier_b = vec![serde_json::json!({
            "classification": "Person",
            "entity_name": "Peter Woodfine"
        })];
        let tier_a = vec![serde_json::json!({
            "classification": "Person",
            "entity_name": "Peter"
        })];
        let dir = tmp_dir("grounded");
        write_enrichment_dpo_pair(
            "DOC_test-abc_456",
            "Peter Woodfine committed a fix to the logging module.",
            &tier_a,
            &tier_b,
            dir.to_str().unwrap(),
        );
        assert_eq!(
            fs::read_dir(&dir).unwrap().count(),
            1,
            "pair with grounded entity must be written"
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn dpo_write_returns_false_on_invalid_classification() {
        // Tier B emits "Technology" — not in ALLOWED_CLASSIFICATIONS.
        // clean_dpo_side rejects it → empty chosen → pair dropped → false.
        let tier_b = vec![serde_json::json!({
            "classification": "Technology",
            "entity_name": "OpenSSL"
        })];
        let tier_a = vec![serde_json::json!({
            "classification": "Project",
            "entity_name": "OpenSSL"
        })];
        let dir = tmp_dir("bad-cls");
        let saved = write_enrichment_dpo_pair(
            "DOC_test-badcls_001",
            "We use OpenSSL for TLS.",
            &tier_a,
            &tier_b,
            dir.to_str().unwrap(),
        );
        assert!(
            !saved,
            "invalid classification must cause pair to be dropped"
        );
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn dpo_write_returns_true_and_corrects_classification() {
        // Tier B says Portugal=Company (wrong) + Jennifer Woodfine=Person (right).
        // Tier A says only Portugal=Location (correct but incomplete).
        // After clean_dpo_side:
        //   Tier B clean: [{Portugal/Location (corrected), Jennifer Woodfine/Person}] — 2 entities
        //   Tier A clean: [{Portugal/Location}] — 1 entity
        // Tier B > Tier A (2 > 1), sides differ after normalization → pair is written.
        // The file must contain "Location" not "Company" for Portugal.
        let tier_b = vec![
            serde_json::json!({"classification": "Company",  "entity_name": "Portugal"}),
            serde_json::json!({"classification": "Person",   "entity_name": "Jennifer Woodfine"}),
        ];
        let tier_a =
            vec![serde_json::json!({"classification": "Location", "entity_name": "Portugal"})];
        let dir = tmp_dir("coerce-cls");
        let saved = write_enrichment_dpo_pair(
            "DOC_test-coerce_001",
            "Jennifer Woodfine works on a project based in Portugal.",
            &tier_a,
            &tier_b,
            dir.to_str().unwrap(),
        );
        assert!(
            saved,
            "coerced pair with more entities than rejected must be written"
        );
        let entries: Vec<_> = fs::read_dir(&dir).unwrap().collect();
        assert_eq!(entries.len(), 1);
        let content = fs::read_to_string(entries[0].as_ref().unwrap().path()).unwrap();
        // Parse the JSONL record and inspect only the `chosen` field.
        // The full `content` now includes few-shot examples in the prompt that themselves
        // contain "Company" as a classification label, so checking `content` as a whole
        // would produce false positives. Only the chosen side matters.
        let record: serde_json::Value =
            serde_json::from_str(&content).expect("DPO pair must be valid JSON");
        let chosen = record["chosen"].as_str().expect("chosen must be a string");
        assert!(
            chosen.contains("\"Location\""),
            "chosen side must contain corrected classification; chosen: {chosen}"
        );
        assert!(
            !chosen.contains("\"Company\""),
            "raw misclassification must not appear in chosen side; chosen: {chosen}"
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn dpo_sweep_docs_never_generate_pairs() {
        // DOC_sweep-* worm IDs come from git commit text where Tier B hallucination
        // rate is too high for reliable DPO signal. Policy: always return false,
        // write nothing — even if the entities are otherwise valid and grounded.
        let tier_b = vec![serde_json::json!({
            "classification": "Person",
            "entity_name": "Peter Woodfine"
        })];
        let tier_a = vec![serde_json::json!({
            "classification": "Person",
            "entity_name": "Peter"
        })];
        let dir = tmp_dir("sweep-skip");
        let saved = write_enrichment_dpo_pair(
            "DOC_sweep-abc123def456_1718500000000",
            "Peter Woodfine committed a fix to the logging module.",
            &tier_a,
            &tier_b,
            dir.to_str().unwrap(),
        );
        assert!(!saved, "sweep docs must never generate DPO pairs");
        assert_eq!(
            fs::read_dir(&dir).unwrap().count(),
            0,
            "no file must be written for sweep-origin docs"
        );
        fs::remove_dir_all(&dir).ok();
    }
}
