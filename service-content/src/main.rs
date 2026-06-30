mod config_http;
mod entity_filter;
mod er;
mod graph;
mod http;
mod pairing;
#[cfg(test)]
mod pipeline_tests;
mod taxonomy;

use graph::{GraphEntity, GraphStore, LbugGraphStore};
use notify::{Event, RecursiveMode, Result as NotifyResult, Watcher};
use serde_json::Value;
use std::fs;
use std::io::{BufRead, Write};
use std::path::Path;
use std::sync::mpsc::{RecvTimeoutError, SyncSender};
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

/// Job queued for the async Tier A (OLMo 7B) training pass.
/// Every document processed by Tier 0 (GLiNER) is also queued here so the
/// (GLiNER output, OLMo output) delta can be written as a DPO training pair.
/// The worker runs at background priority and never blocks the drain loop.
struct TierAJob {
    corpus_text: String,
    worm_id: String,
    module_id: String,
    tier_0_entities: Vec<serde_json::Value>,
    feedback_dir: String,
    doorman_endpoint: String,
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
  - Systemd unit names ending in .service or .timer: local-content.service, llama-server.service, lora-update.timer, nightly-build.timer — these are process managers, not projects. OMIT.\n\
  - Mailbox message identifiers: hyphenated all-lowercase slugs containing an 8-digit date segment (e.g. command-20260520-stage6-rebase-required, project-totebox-20260622-stage6-d9-d8-p8-fixes). These are message IDs, not entities. OMIT.\n\
  - Operational status phrases joined by \" + \": \"service-content rebuilt + deployed\", \"Yo-Yo env IP update + Doorman restart\" — these describe events, not named entities. OMIT.\n\
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
    let graph_dir_for_http = graph_dir.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to build HTTP tokio runtime");
        rt.block_on(http::run_server(
            graph_for_http,
            http_bind,
            doorman_for_http,
            ontology_for_http,
            corpus_dir_for_http,
            graph_dir_for_http,
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
    let tier_progress_path = Arc::new(Path::new(&graph_dir).join("tier_progress.jsonl"));
    let backpressure_threshold: u64 = std::env::var("SERVICE_CONTENT_QUEUE_BACKPRESSURE_THRESHOLD")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(50);
    let mut tier_b_enrichment_queue: std::collections::VecDeque<String> =
        load_tier_b_pending(&tier_progress_path)
            .into_iter()
            .filter(|f| Path::new(&corpus_dir).join(f).exists())
            .collect();
    if !tier_b_enrichment_queue.is_empty() {
        println!(
            "[SYSTEM] {} doc(s) with tier_a_done=true, tier_b_done=false — queued for Tier B enrichment on recovery.",
            tier_b_enrichment_queue.len()
        );
    }
    let max_defer_retries: u32 = std::env::var("SLM_MAX_DEFER_RETRIES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);
    let mut deferred_counts: std::collections::HashMap<String, u32> =
        std::collections::HashMap::new();
    // Per-file backoff: (retry_not_before, current_delay_ms).
    // Sequence: 5s → 10s → 20s → 40s → 80s → 120s with ±25% deterministic jitter.
    let mut backoff_state: std::collections::HashMap<String, (std::time::Instant, u64)> =
        std::collections::HashMap::new();
    let dead_letter_dir = format!("{}/dead-letter", corpus_dir);
    if let Err(e) = std::fs::create_dir_all(&dead_letter_dir) {
        eprintln!(
            "[WARN] Could not create dead-letter dir {}: {}",
            dead_letter_dir, e
        );
    }

    // ── Async Tier A training queue ───────────────────────────────────────────
    // Every document processed by Tier 0 (GLiNER Found or Empty) is enqueued
    // here for a secondary OLMo 7B extraction pass.  The worker does two things
    // per document:
    //   1. Write a DPO training pair (GLiNER=chosen teacher, OLMo=rejected student)
    //   2. Write source-grounded OLMo entities to the graph — since we are already
    //      paying for the OLMo call, the marginal cost of a graph upsert is negligible.
    //      OLMo may find entities GLiNER missed; on GlinerOutcome::Empty docs these
    //      are the only entities in the graph at all.
    // Guard: OLMo entities are only written if the entity name appears verbatim in
    // the corpus text (source grounding), preventing hallucinated names from polluting
    // the graph.  Written at confidence 0.65 (vs GLiNER's 0.75).
    // Bounded: 200 jobs ≈ 2 MB pending corpus. A full channel silently drops
    // the DPO job (drain continues). Prevents unbounded growth when OLMo is slow.
    let (tier_a_tx, tier_a_rx) = std::sync::mpsc::sync_channel::<TierAJob>(200);
    {
        let gs_worker = Arc::clone(&graph_store);
        thread::spawn(move || {
            for job in tier_a_rx {
                // Single combined OLMo call: entity extraction + open IE relation triples.
                let (tier_a_ents, tier_a_rels) =
                    call_tier_a_combined(&job.corpus_text, &job.doorman_endpoint);

                // 1. DPO training pair (entity comparison only — same format as before)
                if write_gliner_olmo_dpo_pair(
                    &job.worm_id,
                    &job.corpus_text,
                    &job.tier_0_entities,
                    &tier_a_ents,
                    &job.feedback_dir,
                ) {
                    println!(
                        "[TIER-A-TRAIN] DPO pair written — {} (gliner:{} olmo:{} entities)",
                        job.worm_id,
                        job.tier_0_entities.len(),
                        tier_a_ents.len(),
                    );
                }

                let corpus_lower = job.corpus_text.to_lowercase();

                // 2. Write source-grounded OLMo entities to graph
                if !tier_a_ents.is_empty() {
                    let grounded: Vec<serde_json::Value> = tier_a_ents
                        .iter()
                        .filter(|e| {
                            e.get("entity_name")
                                .and_then(|v| v.as_str())
                                .map(|name| corpus_lower.contains(&name.to_lowercase()))
                                .unwrap_or(false)
                        })
                        .cloned()
                        .collect();
                    if !grounded.is_empty() {
                        let mut ge = raw_entities_to_graph(&grounded, &job.module_id, 0.65);
                        for e in &mut ge {
                            e.source_doc = Some(job.worm_id.clone());
                        }
                        match gs_worker.upsert_entities(&job.module_id, &ge) {
                            Ok(n) => {
                                if n > 0 {
                                    println!(
                                        "[TIER-A] {} new entities written to graph — {} (olmo, grounded)",
                                        n, job.worm_id
                                    );
                                }
                            }
                            Err(e) => {
                                eprintln!("[TIER-A] Graph write failed for {}: {}", job.worm_id, e)
                            }
                        }
                    }
                }

                // 3. Write source-grounded relation triples to RelatedTo
                if !tier_a_rels.is_empty() {
                    use crate::graph::RelatedToEdge;
                    let edges: Vec<RelatedToEdge> = tier_a_rels
                        .iter()
                        .filter_map(|r| {
                            let subj = r.get("subject")?.as_str()?;
                            let pred = r.get("predicate")?.as_str()?;
                            let obj = r.get("object")?.as_str()?;
                            // Source-grounding: both endpoints must appear verbatim in corpus
                            if corpus_lower.contains(&subj.to_lowercase())
                                && corpus_lower.contains(&obj.to_lowercase())
                            {
                                Some(RelatedToEdge {
                                    src_entity_name: subj.to_string(),
                                    tgt_entity_name: obj.to_string(),
                                    relation_type: pred.to_string(),
                                })
                            } else {
                                None
                            }
                        })
                        .collect();
                    if !edges.is_empty() {
                        match gs_worker.upsert_edges(&job.module_id, &edges) {
                            Ok(n) => {
                                if n > 0 {
                                    println!(
                                        "[TIER-A] {} relation triples written — {} (open IE)",
                                        n, job.worm_id
                                    );
                                }
                            }
                            Err(e) => eprintln!(
                                "[TIER-A] Relation write failed for {}: {}",
                                job.worm_id, e
                            ),
                        }
                    }
                }
            }
        });
    }

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

        // Optional env var: comma-separated module IDs whose CORPUS files drain first.
        // Uses a 1024-byte header scan — fast heuristic, no full JSON parse.
        let priority_ids: std::collections::HashSet<String> =
            std::env::var("CORPUS_PRIORITY_MODULE_IDS")
                .unwrap_or_default()
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect();

        let corpus_files: Vec<std::path::PathBuf> = fs::read_dir(Path::new(&corpus_dir))
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
            .collect();

        // Partition into priority-first if CORPUS_PRIORITY_MODULE_IDS is set.
        let queue_deque: VecDeque<std::path::PathBuf> = if priority_ids.is_empty() {
            corpus_files.into()
        } else {
            let is_priority = |p: &std::path::PathBuf| -> bool {
                use std::io::Read;
                let mut buf = [0u8; 1024];
                let Ok(mut f) = std::fs::File::open(p) else {
                    return false;
                };
                let n = f.read(&mut buf).unwrap_or(0);
                let head = std::str::from_utf8(&buf[..n]).unwrap_or("");
                priority_ids.iter().any(|id| head.contains(id.as_str()))
            };
            let (hi, lo): (Vec<_>, Vec<_>) = corpus_files.into_iter().partition(is_priority);
            hi.into_iter().chain(lo).collect()
        };

        let queue: Arc<Mutex<VecDeque<std::path::PathBuf>>> = Arc::new(Mutex::new(queue_deque));

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
            let tp = Arc::clone(&tier_progress_path);
            let bpt = backpressure_threshold;
            let tx_clone = tier_a_tx.clone();
            handles.push(thread::spawn(move || loop {
                let Some(path) = q.lock().unwrap().pop_front() else {
                    break;
                };
                let fname = match path.file_name().and_then(|n| n.to_str()) {
                    Some(n) => n.to_string(),
                    None => continue,
                };
                let mut tier_b_used = false;
                let result = process_corpus(
                    &path,
                    &cd,
                    &de,
                    &mid,
                    &gs,
                    &fd,
                    &mut tier_b_used,
                    bpt,
                    Some(&tx_clone),
                );
                if matches!(result, ExtractResult::Success) {
                    write_tier_progress(
                        &tp,
                        serde_json::json!({
                            "corpus_filename": fname,
                            "tier_a_done": true,
                            "tier_b_done": tier_b_used,
                            "processed_at": std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs(),
                        }),
                    );
                }
                match result {
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
                                let mut tier_b_used = false;
                                let watcher_result = process_corpus(
                                    &path,
                                    &crm_dir,
                                    &doorman_endpoint,
                                    &module_id,
                                    &graph_store,
                                    &feedback_dir,
                                    &mut tier_b_used,
                                    backpressure_threshold,
                                    Some(&tier_a_tx),
                                );
                                if matches!(watcher_result, ExtractResult::Success) {
                                    write_tier_progress(
                                        &tier_progress_path,
                                        serde_json::json!({
                                            "corpus_filename": filename,
                                            "tier_a_done": true,
                                            "tier_b_done": tier_b_used,
                                            "processed_at": std::time::SystemTime::now()
                                                .duration_since(std::time::UNIX_EPOCH)
                                                .unwrap_or_default()
                                                .as_secs(),
                                        }),
                                    );
                                }
                                match watcher_result {
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
                let now = std::time::Instant::now();
                for filename in retry_queue {
                    // Respect per-file backoff — push back if window not elapsed.
                    if let Some((retry_not_before, _)) = backoff_state.get(&filename) {
                        if now < *retry_not_before {
                            deferred_ledgers.push(filename);
                            continue;
                        }
                    }
                    let path = Path::new(&corpus_dir).join(&filename);
                    let mut tier_b_used = false;
                    let retry_result = process_corpus(
                        &path,
                        &crm_dir,
                        &doorman_endpoint,
                        &module_id,
                        &graph_store,
                        &feedback_dir,
                        &mut tier_b_used,
                        backpressure_threshold,
                        Some(&tier_a_tx),
                    );
                    if matches!(retry_result, ExtractResult::Success) {
                        write_tier_progress(
                            &tier_progress_path,
                            serde_json::json!({
                                "corpus_filename": filename,
                                "tier_a_done": true,
                                "tier_b_done": tier_b_used,
                                "processed_at": std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs(),
                            }),
                        );
                    }
                    match retry_result {
                        ExtractResult::Success | ExtractResult::Failed => {
                            backoff_state.remove(&filename);
                            deferred_counts.remove(&filename);
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
                                // Dead-letter quarantine: move file out of corpus dir so it is
                                // not re-queued. Do NOT add to processed_ledger — the file
                                // failed to process and must remain operator-recoverable.
                                let src = Path::new(&corpus_dir).join(&filename);
                                // Append failure reason before the extension so operators can
                                // batch-replay dead-letters by failure category without reading logs.
                                let dead_filename = {
                                    let stem = Path::new(&filename)
                                        .file_stem()
                                        .and_then(|s| s.to_str())
                                        .unwrap_or(&filename);
                                    let ext = Path::new(&filename)
                                        .extension()
                                        .and_then(|s| s.to_str())
                                        .map(|e| format!(".{e}"))
                                        .unwrap_or_default();
                                    format!("{stem}_FAILED_transient{ext}")
                                };
                                let dst = Path::new(&dead_letter_dir).join(&dead_filename);
                                match std::fs::rename(&src, &dst) {
                                    Ok(()) => eprintln!(
                                        "[WARN] {} quarantined after {} retries → dead-letter/{} — replay by copying back to corpus dir",
                                        filename, count, dead_filename
                                    ),
                                    Err(e) => {
                                        eprintln!(
                                            "[WARN] {} max retries reached but rename failed ({}); adding to processed_ledger as fallback",
                                            filename, e
                                        );
                                        append_processed_ledger(&processed_ledgers_path, &filename);
                                        processed_ledgers.insert(filename.clone());
                                    }
                                }
                                backoff_state.remove(&filename);
                                deferred_counts.remove(&filename);
                            } else {
                                // Exponential backoff: 5s → 10s → 20s → 40s → 80s → 120s.
                                // Deterministic ±25% jitter keyed on filename byte-sum.
                                let exponent = (*count).saturating_sub(1).min(4);
                                let base_ms: u64 = (5_000u64 << exponent).min(120_000);
                                let char_sum: u32 = filename.bytes().map(|b| b as u32).sum();
                                let jitter_num = char_sum % 100; // 0..99
                                                                 // jitter_factor: 0.75 to 1.25
                                let jittered_ms = base_ms * (150 + jitter_num as u64 / 2) / 200;
                                let delay_ms = jittered_ms.clamp(5_000, 120_000);
                                let retry_not_before =
                                    std::time::Instant::now() + Duration::from_millis(delay_ms);
                                backoff_state
                                    .insert(filename.clone(), (retry_not_before, delay_ms));
                                println!(
                                    "[DEFER] {} retry #{} in {:.1}s",
                                    filename,
                                    count,
                                    delay_ms as f64 / 1000.0
                                );
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
                    let mut probe_tier_b = false;
                    let probe_result = process_corpus(
                        &probe_path,
                        &crm_dir,
                        &doorman_endpoint,
                        &module_id,
                        &graph_store,
                        &feedback_dir,
                        &mut probe_tier_b,
                        backpressure_threshold,
                        Some(&tier_a_tx),
                    );
                    if matches!(probe_result, ExtractResult::Success) {
                        write_tier_progress(
                            &tier_progress_path,
                            serde_json::json!({
                                "corpus_filename": probe,
                                "tier_a_done": true,
                                "tier_b_done": probe_tier_b,
                                "processed_at": std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs(),
                            }),
                        );
                    }
                    match probe_result {
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
                            // Promote any Tier-A-only docs for Tier B enrichment.
                            if !tier_b_enrichment_queue.is_empty() {
                                println!(
                                    "[RECOVERY] Promoting {} tier_b_pending docs for Tier B enrichment.",
                                    tier_b_enrichment_queue.len()
                                );
                                for f in tier_b_enrichment_queue.drain(..) {
                                    processed_ledgers.remove(&f);
                                    deferred_ledgers.push(f);
                                }
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
/// System prompt for open IE relation extraction — runs alongside entity extraction in
/// a single combined Tier A OLMo call. Returns (subject, predicate, object) triples.
/// Both subject and object must be entity names from the text (source-grounding enforced
/// at write time before upsert_edges). Predicate is a short English verb phrase.
const RELATION_EXTRACTION_ADDITION: &str = "\n\nAFTER the entities array, also extract relationships between named entities.\n\
Return them under a \"relations\" key as an array of triples:\n\
  {\"subject\": \"<entity_name>\", \"predicate\": \"<verb phrase>\", \"object\": \"<entity_name>\"}\n\
Rules:\n\
- subject and object must be entity names that appear verbatim in the text.\n\
- predicate is a short English verb phrase (e.g., \"acquired\", \"employed by\", \"invested in\", \"owns\").\n\
- omit any triple where subject or object does not name a specific entity from the text.\n\
- return [] for relations when no clear relationships are stated.\n\
Return format: {\"entities\": [...], \"relations\": [...]}";

fn relation_extraction_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "entities": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "entity_name": {"type": "string"},
                        "classification": {
                            "type": "string",
                            "enum": ["Person", "Company", "Project", "Account", "Location"]
                        },
                        "role_vector":     {"type": ["string", "null"]},
                        "location_vector": {"type": ["string", "null"]},
                        "contact_vector":  {"type": ["string", "null"]}
                    },
                    "required": ["entity_name", "classification"],
                    "additionalProperties": false
                }
            },
            "relations": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "subject":   {"type": "string"},
                        "predicate": {"type": "string"},
                        "object":    {"type": "string"}
                    },
                    "required": ["subject", "predicate", "object"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["entities", "relations"],
        "additionalProperties": false
    })
}

/// Combined entity + relation extraction in one Tier A OLMo call.
/// Returns `(entities, relations)` parsed from the combined JSON object.
/// Falls back to `(tier_a_entities_only, [])` on parse failure.
fn call_tier_a_combined(
    corpus_text: &str,
    doorman_endpoint: &str,
) -> (Vec<serde_json::Value>, Vec<serde_json::Value>) {
    let combined_prompt = format!(
        "{}{}",
        EXTRACTION_SYSTEM_PROMPT, RELATION_EXTRACTION_ADDITION
    );
    let schema = relation_extraction_schema();
    let use_grammar = std::env::var("SERVICE_CONTENT_TIER_A_GRAMMAR")
        .map(|v| v == "json_schema")
        .unwrap_or(false);

    let chat_body = if use_grammar {
        serde_json::json!({
            "messages": [
                {"role": "system", "content": combined_prompt},
                {"role": "user",   "content": corpus_text}
            ],
            "grammar": {"type": "json-schema", "value": schema},
            "temperature": 0.0,
            "max_tokens": 1536,
            "cache_prompt": true
        })
    } else {
        serde_json::json!({
            "messages": [
                {"role": "system", "content": combined_prompt},
                {"role": "user",   "content": corpus_text},
                {"role": "assistant", "content": "{\"entities\": [{\""}
            ],
            "temperature": 0.0,
            "max_tokens": 1536,
            "cache_prompt": true
        })
    };

    let url = format!("{}/v1/chat/completions", doorman_endpoint);
    let client = reqwest::blocking::Client::new();
    let raw = match client
        .post(&url)
        .header("X-Foundry-Complexity", "low")
        .header("X-Foundry-Background", "true")
        .json(&chat_body)
        .timeout(Duration::from_secs(180))
        .send()
    {
        Ok(r) if r.status().is_success() => r.json::<serde_json::Value>().ok(),
        _ => None,
    };

    let content = raw.as_ref().and_then(|v| {
        v["content"]
            .as_str()
            .or_else(|| v["choices"][0]["message"]["content"].as_str())
            .map(|s| s.to_string())
    });

    if let Some(mut content) = content {
        // Re-attach pre-fill prefix when the model returned only the continuation.
        if !content.trim_start().starts_with('{') {
            content = format!("{{\"entities\": [{{\"{}\"", content);
        }
        let content = content
            .trim()
            .strip_prefix("```json")
            .unwrap_or(content.trim())
            .strip_prefix("```")
            .unwrap_or(content.trim());
        let content = content.strip_suffix("```").unwrap_or(content).trim();
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(content) {
            let entities = v["entities"].as_array().cloned().unwrap_or_default();
            let relations = v["relations"].as_array().cloned().unwrap_or_default();
            return (entities, relations);
        }
    }
    (vec![], vec![])
}

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
            "grammar": {"type": "json-schema", "value": entity_schema},
            "temperature": 0.0,
            "max_tokens": 1024,
            "cache_prompt": true
        })
    } else {
        serde_json::json!({
            "messages": [
                {"role": "system", "content": EXTRACTION_SYSTEM_PROMPT},
                {"role": "user",   "content": corpus_text},
                {"role": "assistant", "content": "[{\""}
            ],
            "temperature": 0.0,
            "max_tokens": 1024,
            "cache_prompt": true
        })
    };
    let url = format!("{}/v1/chat/completions", doorman_endpoint);
    let client = reqwest::blocking::Client::new();
    match client
        .post(&url)
        .header("X-Foundry-Complexity", "low")
        // Route via complete_background() so interactive chat can preempt this
        // extraction call. Without this header, Tier A competes with interactive
        // on equal footing inside OLMo, causing 4-minute batch delays.
        .header("X-Foundry-Background", "true")
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

/// Remove `<https://…>` and `<http://…>` inline URL fragments from a string.
fn strip_inline_urls(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '<' {
            let rest: String = chars.clone().take(8).collect();
            if rest.starts_with("https://") || rest.starts_with("http://") {
                for c2 in chars.by_ref() {
                    if c2 == '>' {
                        break;
                    }
                }
                continue;
            }
        }
        out.push(c);
    }
    out
}

/// Clean web-scraped corpus text before entity extraction.
/// Removes nav links, inline URLs, and Unicode box-drawing/block characters.
/// Clean Bloomberg/PDF text passes through unchanged (fast path).
fn preprocess_corpus_text(text: &str) -> std::borrow::Cow<'_, str> {
    let needs_clean = text.contains('<')
        || text.contains('\u{fffd}')
        || text.chars().any(|c| matches!(c as u32, 0x2500..=0x25FF));
    if !needs_clean {
        return std::borrow::Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        let trimmed = line.trim();
        // Drop short navigation UI lines (nav link pattern: keyword + inline URL)
        if trimmed.len() < 80
            && (trimmed.starts_with("Home<")
                || trimmed.starts_with("Blog<")
                || trimmed.starts_with("Tweet")
                || trimmed.starts_with("Share")
                || trimmed.starts_with("Tenant Portal"))
        {
            continue;
        }
        // Strip inline URLs then box-drawing chars
        let cleaned = strip_inline_urls(trimmed);
        let cleaned: String = cleaned
            .chars()
            .filter(|c| !matches!(*c as u32, 0x2500..=0x259F | 0x25A0..=0x25FF | 0xFFFD))
            .collect();
        if !cleaned.trim().is_empty() {
            out.push_str(&cleaned);
            out.push('\n');
        }
    }
    std::borrow::Cow::Owned(out)
}

const GLINER_ENDPOINT: &str = "http://127.0.0.1:9085";

/// Call GLiNER Tier 0 microservice for entity extraction.
/// Extractive (cannot hallucinate), 150x faster than OLMo.
/// Returns None when GLiNER is unavailable — caller falls back to Tier A OLMo.
/// Max chars per GLiNER chunk. BERT encoder limit is ~512 tokens (~2000 chars).
/// Long documents are split into consecutive chunks; entities are merged + deduped.
const GLINER_MAX_CHARS: usize = 2000;

/// Split `text` into consecutive slices of at most `max_chars` bytes,
/// cutting at the last sentence-ending punctuation within each window.
/// Falls back to a hard byte cut when no sentence boundary is found.
/// All returned slices are valid UTF-8 (cuts are aligned to char boundaries).
fn chunk_for_gliner(text: &str, max_chars: usize) -> Vec<&str> {
    if text.len() <= max_chars {
        return vec![text];
    }
    let mut chunks = Vec::new();
    let mut start = 0usize;
    while start < text.len() {
        // Align raw_end to a valid UTF-8 char boundary (handles multi-byte chars).
        let mut raw_end = (start + max_chars).min(text.len());
        if raw_end < text.len() {
            while raw_end > start && !text.is_char_boundary(raw_end) {
                raw_end -= 1;
            }
        }
        // Prefer cutting at the last sentence boundary within the window.
        let end = if raw_end < text.len() {
            text[start..raw_end]
                .rfind(['.', '!', '?'])
                .map(|rel| start + rel + 1)
                .unwrap_or(raw_end)
        } else {
            raw_end
        };
        // Safety: never produce a zero-length chunk that stalls the loop.
        let end = end.max(start + 1).min(text.len());
        chunks.push(&text[start..end]);
        // 150-char overlap so entities at chunk boundaries are not split across two
        // incomplete windows. Guard: only overlap when it still advances the cursor.
        start = if end > start + 150 { end - 150 } else { end };
    }
    chunks
}

enum GlinerOutcome {
    /// GLiNER found named entities — use them, skip Tier A.
    Found(Vec<serde_json::Value>),
    /// GLiNER is reachable but found nothing (structured data, contentless text).
    /// Tier A OLMo won't improve on this — mark the file done with 0 entities.
    Empty,
    /// GLiNER service is unreachable or returned an unexpected response.
    /// Fall through to Tier A with backpressure gate.
    Unavailable,
}

fn call_tier_0_gliner(corpus_text: &str, domain_id: Option<&str>) -> GlinerOutcome {
    let chunks = chunk_for_gliner(corpus_text, GLINER_MAX_CHARS);

    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("  -> [TIER-0] Client build failed: {}", e);
            return GlinerOutcome::Unavailable;
        }
    };

    let domain = domain_id.unwrap_or("projects");
    let url = format!("{GLINER_ENDPOINT}/v1/extract");
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut all_entities: Vec<serde_json::Value> = Vec::new();

    for chunk in &chunks {
        let body = serde_json::json!({
            "text": chunk,
            "domain_id": domain,
        });
        let resp = match client.post(&url).json(&body).send() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("  -> [TIER-0] Request failed: {}", e);
                return GlinerOutcome::Unavailable;
            }
        };
        if !resp.status().is_success() {
            eprintln!("  -> [TIER-0] Non-2xx status: {}", resp.status());
            return GlinerOutcome::Unavailable;
        }
        let r: serde_json::Value = match resp.json() {
            Ok(v) => v,
            Err(e) => {
                eprintln!("  -> [TIER-0] JSON parse failed: {}", e);
                return GlinerOutcome::Unavailable;
            }
        };
        let entities = match r["entities"].as_array() {
            Some(a) => a,
            None => {
                eprintln!("  -> [TIER-0] No 'entities' array in response");
                return GlinerOutcome::Unavailable;
            }
        };
        for ent in entities {
            let key = format!(
                "{}__{}",
                ent["entity_name"].as_str().unwrap_or("").to_lowercase(),
                ent["classification"].as_str().unwrap_or(""),
            );
            if seen.insert(key) {
                all_entities.push(ent.clone());
            }
        }
    }

    if all_entities.is_empty() {
        eprintln!(
            "  -> [TIER-0] No entities in {} chunk(s) — file done (GLiNER available)",
            chunks.len()
        );
        return GlinerOutcome::Empty;
    }
    if chunks.len() > 1 {
        eprintln!(
            "  -> [TIER-0] {} unique entities from {} chunks",
            all_entities.len(),
            chunks.len()
        );
    }
    GlinerOutcome::Found(all_entities)
}

/// Convert raw entity JSON values into `GraphEntity` structs.
/// Logs per-stage rejection telemetry at INFO level for every batch so
/// operators can tune filter thresholds without reading LadybugDB directly.
fn raw_entities_to_graph(
    raw: &[serde_json::Value],
    module_id: &str,
    confidence: f64,
) -> Vec<GraphEntity> {
    let (
        mut drop_empty,
        mut drop_noise,
        mut drop_word_count,
        mut drop_coerce,
        mut drop_oov,
        mut drop_field_missing,
    ) = (0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
    let result = raw
        .iter()
        .filter_map(|ent| {
            let entity_name = match ent["entity_name"].as_str() {
                Some(s) => s.to_string(),
                None => {
                    drop_field_missing += 1;
                    return None;
                }
            };
            let classification = match ent["classification"].as_str() {
                Some(s) => s.to_string(),
                None => {
                    drop_field_missing += 1;
                    return None;
                }
            };
            if entity_name.is_empty() || classification.is_empty() {
                drop_empty += 1;
                return None;
            }
            // Change 2: deterministic noise filter — rejects env vars, file paths,
            // snake_case identifiers, call expressions, fragments, and placeholders.
            if entity_filter::is_noise_entity_name(&entity_name) {
                drop_noise += 1;
                return None;
            }
            // Change 5: word-count gate — sentences and clauses are not entity names.
            if entity_name.split_whitespace().count() > 8 {
                drop_word_count += 1;
                return None;
            }
            // Change 4: type-coherence validation — corrects or rejects misclassified
            // entities (country-as-Company, path-as-Project, CAPS-as-Account).
            let classification =
                match entity_filter::coerce_classification(&entity_name, &classification) {
                    Some(cls) => cls,
                    None => {
                        drop_coerce += 1;
                        return None;
                    }
                };
            // Reject out-of-vocabulary classifications. OLMo may emit values such as
            // "Licence" or "Technology" when the prompt omit list is insufficient.
            // Dropping them here prevents bad data from landing in LadybugDB.
            if !entity_filter::ALLOWED_CLASSIFICATIONS.contains(&classification.as_str()) {
                drop_oov += 1;
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
                source_doc: None, // callers that know the worm_id may set this post-construction
            })
        })
        .collect::<Vec<_>>();
    let total_in = raw.len();
    let kept = result.len();
    let _dropped = total_in - kept;
    if total_in > 0 {
        println!(
            "[entity_filter] module={module_id} kept={kept}/{total_in} \
             drop=field_missing:{drop_field_missing} empty:{drop_empty} noise:{drop_noise} \
             word_count:{drop_word_count} coerce:{drop_coerce} oov:{drop_oov}"
        );
    }
    result
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
    tier_0_entities: &[serde_json::Value],
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
    if tier_0_entities.is_empty() {
        return false; // no rejected signal — DPO pair would teach verbosity, not accuracy
    }
    // DPO pre-save validator — applies the SAME filter chain as raw_entities_to_graph:
    // noise rejection + word-count gate + coerce_classification + ALLOWED_CLASSIFICATIONS.
    // Ensures the chosen side of the DPO pair matches what actually lands in LadybugDB.
    let tier_b_clean = entity_filter::clean_dpo_side(tier_b_raw);
    let tier_a_clean = entity_filter::clean_dpo_side(tier_0_entities);
    if tier_b_clean.is_empty() {
        return false; // all Tier B entities were noise — no training signal after cleaning
    }
    if tier_b_clean.len() < tier_a_clean.len() {
        return false; // cleaning made chosen worse than rejected — degenerate pair
    }
    // Shadow-rebind: rest of function operates on cleaned slices.
    let tier_b_raw = tier_b_clean.as_slice();
    let tier_0_entities = tier_a_clean.as_slice();
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
    let tier_a_normalized: Vec<serde_json::Value> = tier_0_entities
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

/// JSON Schema shared by both Tier A (OLMo 7B) and the Tier A training worker.
fn entity_extraction_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "array",
        "items": {
            "type": "object",
            "properties": {
                "entity_name": {"type": "string"},
                "classification": {
                    "type": "string",
                    "enum": ["Person", "Company", "Project", "Account", "Location"]
                },
                "role_vector":     {"type": ["string", "null"]},
                "location_vector": {"type": ["string", "null"]},
                "contact_vector":  {"type": ["string", "null"]}
            },
            "required": ["entity_name", "classification"],
            "additionalProperties": false
        }
    })
}

/// Write a DPO training pair from the GLiNER vs OLMo comparison.
/// GLiNER is the teacher (extractive, zero hallucinations); OLMo is the student.
/// chosen = GLiNER output (when non-empty); OLMo=chosen only when GLiNER returned [].
/// Returns true if a pair was written.
fn write_gliner_olmo_dpo_pair(
    worm_id: &str,
    corpus_text: &str,
    tier_0_entities: &[serde_json::Value],
    tier_a_entities: &[serde_json::Value],
    feedback_dir: &str,
) -> bool {
    if worm_id.starts_with("DOC_sweep-") {
        return false; // git commit text — hallucination risk too high
    }
    // Both empty → both models agree nothing is here; no training signal
    if tier_0_entities.is_empty() && tier_a_entities.is_empty() {
        return false;
    }
    // Normalize + sort both sides to {classification, entity_name} for stable comparison
    let normalize_sorted = |ents: &[serde_json::Value]| -> Vec<serde_json::Value> {
        let mut v = entity_filter::clean_dpo_side(ents);
        v.sort_by_key(|e| {
            format!(
                "{}__{}",
                e.get("classification")
                    .and_then(|v| v.as_str())
                    .unwrap_or(""),
                e.get("entity_name").and_then(|v| v.as_str()).unwrap_or(""),
            )
        });
        v.iter()
            .map(|e| {
                serde_json::json!({
                    "classification": e.get("classification").unwrap_or(&serde_json::Value::Null),
                    "entity_name":    e.get("entity_name").unwrap_or(&serde_json::Value::Null),
                })
            })
            .collect()
    };
    let t0_norm = normalize_sorted(tier_0_entities);
    let ta_norm = normalize_sorted(tier_a_entities);
    // Identical → no training delta
    if serde_json::to_string(&t0_norm).unwrap_or_default()
        == serde_json::to_string(&ta_norm).unwrap_or_default()
    {
        return false;
    }
    // GLiNER non-empty → GLiNER=chosen; GLiNER empty → OLMo=chosen (caught a miss)
    let (chosen, rejected, pair_type) = if !t0_norm.is_empty() {
        (&t0_norm, &ta_norm, "gliner-distillation")
    } else {
        (&ta_norm, &t0_norm, "gliner-empty-olmo-found")
    };
    // Source grounding: verify chosen entities appear in corpus text
    let corpus_lower = corpus_text.to_lowercase();
    let all_grounded = chosen.iter().all(|e| {
        e.get("entity_name")
            .and_then(|v| v.as_str())
            .map(|name| corpus_lower.contains(&name.to_lowercase()))
            .unwrap_or(false)
    });
    if !all_grounded {
        return false; // hallucinated entity in chosen side — discard
    }
    let chosen_json = serde_json::to_string(chosen).unwrap_or_default();
    let rejected_json = serde_json::to_string(rejected).unwrap_or_default();
    let prompt = format!("{}\n\nText:\n{}", EXTRACTION_SYSTEM_PROMPT, corpus_text);
    let now = chrono::Utc::now();
    let pair = serde_json::json!({
        "prompt":      prompt,
        "chosen":      chosen_json,
        "rejected":    rejected_json,
        "source_type": pair_type,
        "worm_id":     worm_id,
        "timestamp":   now.to_rfc3339(),
    });
    let _ = fs::create_dir_all(feedback_dir);
    let filename = format!(
        "{}/gliner-distill-{}-{}.jsonl",
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

/// Returns true if Doorman queue depth exceeds `threshold`.
/// Returns false (conservative) if Doorman is unreachable — do not gate on unavailable info.
fn check_doorman_backpressure(doorman_endpoint: &str, threshold: u64) -> bool {
    if threshold == 0 {
        return false;
    }
    let url = format!("{}/readyz", doorman_endpoint);
    let client = reqwest::blocking::Client::new();
    let Ok(resp) = client.get(&url).timeout(Duration::from_secs(3)).send() else {
        return false;
    };
    let Ok(body) = resp.json::<serde_json::Value>() else {
        return false;
    };
    body["queue_pending"]
        .as_u64()
        .map(|p| p > threshold)
        .unwrap_or(false)
}

fn write_tier_progress(path: &Path, entry: serde_json::Value) {
    use std::io::Write;
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{}", entry);
    }
}

/// Read tier_progress.jsonl; return corpus filenames where tier_a_done=true AND tier_b_done=false.
/// Last entry per corpus_filename wins (append-only; newer entries supersede older ones).
fn load_tier_b_pending(path: &Path) -> Vec<String> {
    let Ok(file) = fs::File::open(path) else {
        return Vec::new();
    };
    use std::io::BufRead;
    let mut latest: std::collections::HashMap<String, serde_json::Value> =
        std::collections::HashMap::new();
    for line in std::io::BufReader::new(file).lines().map_while(Result::ok) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
            if let Some(fname) = v["corpus_filename"].as_str() {
                latest.insert(fname.to_string(), v);
            }
        }
    }
    latest
        .into_values()
        .filter(|v| {
            v["tier_a_done"].as_bool().unwrap_or(false)
                && !v["tier_b_done"].as_bool().unwrap_or(false)
        })
        .filter_map(|v| v["corpus_filename"].as_str().map(str::to_string))
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn process_corpus(
    filepath: &Path,
    crm_dir: &str,
    doorman_endpoint: &str,
    module_id: &str,
    graph_store: &Arc<dyn GraphStore>,
    feedback_dir: &str,
    tier_b_used: &mut bool,
    backpressure_threshold: u64,
    tier_a_tx: Option<&SyncSender<TierAJob>>,
) -> ExtractResult {
    *tier_b_used = false;
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
    // Explicit domain_id in CORPUS JSON wins. If absent, infer from worm_id prefix:
    // - DOC_session-* / DOC_sweep-* are engineering session transcripts and git commit
    //   text → "documentation" domain (developer/service/library entity labels).
    //   "documentation" is the Foundry ontology domain for technical content; it maps
    //   directly to domain_documentation.csv in the ontology directory.
    // - Everything else falls to service-gliner DEFAULT_DOMAIN ("projects")
    let domain_id: Option<&str> = payload["domain_id"].as_str().or_else(|| {
        if worm_id.starts_with("DOC_session-") || worm_id.starts_with("DOC_sweep-") {
            Some("documentation")
        } else {
            None
        }
    });

    if corpus_text.is_empty() {
        return ExtractResult::Failed;
    }

    // Preprocess: strip nav links, inline URLs, OCR artifacts before extraction.
    let corpus_text_owned = preprocess_corpus_text(corpus_text);
    let corpus_text = corpus_text_owned.as_ref();

    // ── Shared entity schema used by both tiers ───────────────────────────────
    let entity_schema = entity_extraction_schema();

    // ── Step 1: Tier 0 (GLiNER) with Tier A (OLMo) fallback ─────────────────
    // GLiNER (Tier 0) is direct HTTP to port 9085 — no Doorman involvement.
    // Backpressure gate only applies when GLiNER is down (Unavailable).
    //
    // Every document that passes Tier 0 is also queued for an async Tier A
    // pass via tier_a_tx (fire-and-forget).  The worker writes a DPO training
    // pair: GLiNER=chosen (extractive, no hallucinations), OLMo=rejected (student).
    // When GLiNER returns Empty and OLMo finds entities the roles reverse so we
    // capture GLiNER's blind spots.  This fire-and-forget never blocks the drain.
    let tier_0_entities: Option<Vec<serde_json::Value>> =
        match call_tier_0_gliner(corpus_text, domain_id) {
            GlinerOutcome::Found(ents) => {
                println!(
                    "  -> [TIER-0/A] {} entities extracted (module: {}).",
                    ents.len(),
                    effective_module_id
                );
                // Queue for async Tier A training pass (non-blocking, drops if queue full)
                if let Some(tx) = tier_a_tx {
                    tx.try_send(TierAJob {
                        corpus_text: corpus_text.to_string(),
                        worm_id: worm_id.to_string(),
                        module_id: effective_module_id.to_string(),
                        tier_0_entities: ents.clone(),
                        feedback_dir: feedback_dir.to_string(),
                        doorman_endpoint: doorman_endpoint.to_string(),
                    })
                    .ok();
                }
                Some(ents)
            }
            GlinerOutcome::Empty => {
                // GLiNER reachable, no entities — queue for Tier A to catch GLiNER blind spots,
                // then mark done immediately (production path unblocked).
                if let Some(tx) = tier_a_tx {
                    tx.try_send(TierAJob {
                        corpus_text: corpus_text.to_string(),
                        worm_id: worm_id.to_string(),
                        module_id: effective_module_id.to_string(),
                        tier_0_entities: vec![],
                        feedback_dir: feedback_dir.to_string(),
                        doorman_endpoint: doorman_endpoint.to_string(),
                    })
                    .ok();
                }
                return ExtractResult::Success;
            }
            GlinerOutcome::Unavailable => {
                // GLiNER down — check Doorman backpressure before using Tier A.
                if check_doorman_backpressure(doorman_endpoint, backpressure_threshold) {
                    eprintln!(
                        "[BACKPRESSURE] GLiNER down + queue_pending > {} — deferring {}",
                        backpressure_threshold,
                        filepath.display()
                    );
                    return ExtractResult::DeferTransient;
                }
                let tier_a = call_tier_a_extract(corpus_text, &entity_schema, doorman_endpoint);
                match &tier_a {
                    Some(ents) => println!(
                        "  -> [TIER-0/A] {} entities extracted via Tier A fallback (module: {}).",
                        ents.len(),
                        effective_module_id
                    ),
                    None => println!("  -> [TIER-0/A] Unavailable — proceeding to Tier B."),
                }
                tier_a
            }
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
    // None  → Tier A was unreachable (no-SLM deployment) → DeferTransient (retry soon)
    // Some([]) → Tier A reachable but empty → DeferCircuitOpen (wait for circuit recovery)
    let flush_tier_a = |tier_a: &Option<Vec<serde_json::Value>>, reason: &str| -> ExtractResult {
        if let Some(ta_ents) = tier_a {
            if !ta_ents.is_empty() {
                let mut ge = raw_entities_to_graph(ta_ents, effective_module_id, 0.75);
                for e in &mut ge {
                    e.source_doc = Some(worm_id.to_string());
                }
                match graph_store.upsert_entities(effective_module_id, &ge) {
                    Ok(n) => {
                        println!("  -> [TIER-A] {} entities written ({}).", n, reason);
                        return ExtractResult::Success;
                    }
                    Err(e) => eprintln!("  -> [TIER-A] Graph write failed: {}", e),
                }
            }
            ExtractResult::DeferCircuitOpen
        } else {
            println!(
                "  -> [TIER-A] Unavailable — deferring transiently ({}).",
                reason
            );
            ExtractResult::DeferTransient
        }
    };

    match res {
        Ok(response) => {
            if !response.status().is_success() {
                println!(
                    "  -> [SYS_HALT] Doorman rejected payload: {}",
                    response.status()
                );
                return flush_tier_a(&tier_0_entities, "Tier B rejected");
            }

            let extract_resp = match response.json::<serde_json::Value>() {
                Ok(v) => v,
                Err(_) => {
                    println!("  -> [SYS_HALT] Doorman returned invalid JSON.");
                    return flush_tier_a(&tier_0_entities, "Tier B parse failed");
                }
            };

            // SC-2: differentiate defer reasons.
            if extract_resp["deferred"].as_bool().unwrap_or(false) {
                let reason = extract_resp["defer_reason"].as_str().unwrap_or("unknown");
                return match reason {
                    "yoyo-circuit-open" => {
                        println!("  -> [TIER-B] Circuit open — using Tier A results.");
                        flush_tier_a(&tier_0_entities, "Tier B circuit-open")
                    }
                    _ => {
                        // Transient: use Tier A if available, otherwise retry
                        if let Some(ta_ents) = &tier_0_entities {
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
                return flush_tier_a(&tier_0_entities, "Tier B extraction_ok=false");
            }

            // ── Tier B succeeded ─────────────────────────────────────────────
            let semantic_entities = extract_resp["entities"]
                .as_array()
                .cloned()
                .unwrap_or_default();

            let mut graph_entities =
                raw_entities_to_graph(&semantic_entities, effective_module_id, 0.95);
            for e in &mut graph_entities {
                e.source_doc = Some(worm_id.to_string());
            }
            *tier_b_used = true;

            // If Tier B succeeded but all entities failed the filter, use GLiNER Tier 0 as
            // fallback. This handles the case where OLMo returns field_missing entities
            // (grammar constraint not enforced on the Doorman path).
            let graph_entities = if graph_entities.is_empty() {
                if let Some(ta_ents) = &tier_0_entities {
                    let mut gliner_ge = raw_entities_to_graph(ta_ents, effective_module_id, 0.75);
                    for e in &mut gliner_ge {
                        e.source_doc = Some(worm_id.to_string());
                    }
                    if !gliner_ge.is_empty() {
                        println!(
                            "  -> [TIER-0 RESCUE] Tier B 0 valid — using {} GLiNER entities.",
                            gliner_ge.len()
                        );
                        *tier_b_used = false;
                        gliner_ge
                    } else {
                        graph_entities
                    }
                } else {
                    graph_entities
                }
            } else {
                graph_entities
            };

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
            if let Some(ref ta_ents) = tier_0_entities {
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
            let result = flush_tier_a(&tier_0_entities, &format!("Tier B transport error: {}", e));
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
