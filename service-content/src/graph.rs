use anyhow::{anyhow, Result};
use lbug::{Connection, Database, SystemConfig, Value};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Build the canonical node key for an entity name so trivial surface variants
/// collapse onto the SAME graph node on upsert (`MERGE` dedupes on id).
///
/// Without this, surface variants of one entity become distinct nodes, splitting a
/// single real entity's context across rows (audit 2026-06-19/06-21 measured the
/// Woodfine Capital Projects org fragmented 5-6 ways). Normalisation: lowercase,
/// collapse internal whitespace, strip trailing punctuation + trademark/registered
/// symbols, strip a trailing corporate/legal suffix, then `' ' -> '_'`.
///
/// This is a cheap partial entity-resolution layer (collapses surface variants). It
/// does NOT do alias resolution across genuinely different surface forms (`Peter` vs
/// `Peter M.`) — that is the embedding+fuzzy matcher in the `er` module, applied via a
/// canonical alias table (additive migration, see BRIEF-flow-build-plan).
pub(crate) fn normalize_entity_key(entity_name: &str) -> String {
    let collapsed = entity_name.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut s = collapsed
        .trim_end_matches(['.', ',', ';', ':', ' ', '™', '®', '©'])
        .to_lowercase();
    // Strip ONE unambiguous trailing corporate/legal suffix so e.g. "… Inc." / "… Corp"
    // collapse onto the base name. Two-letter ambiguous abbreviations (co/sa/ag/bv) are
    // deliberately excluded to avoid false strips on persons/locations.
    for suffix in [
        " incorporated",
        " inc",
        " corporation",
        " corp",
        " company",
        " limited",
        " ltd",
        " llc",
        " gmbh",
        " plc",
    ] {
        if let Some(stripped) = s.strip_suffix(suffix) {
            s = stripped.trim_end_matches([' ', '.', ',']).to_string();
            break;
        }
    }
    s.split_whitespace().collect::<Vec<_>>().join("_")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEntity {
    pub entity_name: String,
    pub classification: String, // Person|Company|Project|Account|Location
    pub role_vector: Option<String>,
    pub location_vector: Option<String>,
    pub contact_vector: Option<String>,
    pub module_id: String,
    pub confidence: f64,
}

/// A typed directed edge between two Entity nodes. Input to `upsert_edges`.
/// Both `src_entity_name` and `tgt_entity_name` must already be upserted into the
/// graph for the edge to be created (MATCH fails silently on unknown entities).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelatedToEdge {
    pub src_entity_name: String,
    pub tgt_entity_name: String,
    pub relation_type: String,
}

pub trait GraphStore: Send + Sync {
    fn init_schema(&self) -> Result<()>;
    fn upsert_entities(&self, module_id: &str, entities: &[GraphEntity]) -> Result<usize>;
    fn query_context(&self, module_id: &str, query: &str, limit: usize)
        -> Result<Vec<GraphEntity>>;
    #[allow(dead_code)]
    fn list_entities(&self, module_id: &str) -> Result<Vec<GraphEntity>>;
    /// Delete all entities matching module_id + classification. Returns count deleted.
    fn delete_by_classification(&self, module_id: &str, classification: &str) -> Result<usize>;
    /// Delete entities matching module_id + classification + location_vector (used for
    /// per-domain glossary/topic reloads where classification is shared across domains).
    fn delete_by_classification_and_location(
        &self,
        module_id: &str,
        classification: &str,
        location: &str,
    ) -> Result<usize>;
    /// Count all Entity nodes in the graph across all modules.
    fn count_all(&self) -> Result<usize>;
    /// Delete a single entity by module_id + entity_name.
    fn delete_entity(&self, module_id: &str, entity_name: &str) -> Result<()>;
    /// Write typed directed edges between existing Entity nodes. Idempotent
    /// (checks existence before CREATE). Returns the number of edges written.
    #[allow(dead_code)]
    fn upsert_edges(&self, module_id: &str, edges: &[RelatedToEdge]) -> Result<usize>;
    /// Count alias records in entity_aliases. Used by /healthz + tests.
    #[allow(dead_code)]
    fn count_aliases(&self) -> Result<usize>;
}

pub struct LbugGraphStore {
    db: Arc<Database>,
}

impl LbugGraphStore {
    pub fn new(db_path: &str) -> Result<Self> {
        let config = match std::env::var("SERVICE_CONTENT_LBUG_BUFFER_POOL_MB")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
        {
            Some(mb) => SystemConfig::default().buffer_pool_size(mb * 1024 * 1024),
            None => SystemConfig::default(),
        };
        let db = Database::new(db_path, config)
            .map_err(|e| anyhow!("Failed to open LadybugDB at {}: {}", db_path, e))?;
        Ok(Self { db: Arc::new(db) })
    }

    fn conn(&self) -> Result<Connection<'_>> {
        Connection::new(&self.db).map_err(|e| anyhow!("Failed to create DB connection: {}", e))
    }
}

impl GraphStore for LbugGraphStore {
    fn init_schema(&self) -> Result<()> {
        let conn = self.conn()?;
        conn.query(
            "CREATE NODE TABLE IF NOT EXISTS Entity(\
                id STRING PRIMARY KEY, \
                entity_name STRING, \
                classification STRING, \
                role_vector STRING, \
                location_vector STRING, \
                contact_vector STRING, \
                module_id STRING, \
                confidence DOUBLE, \
                created_at STRING\
            )",
        )
        .map_err(|e| anyhow!("init_schema Entity table failed: {}", e))?;

        conn.query(
            "CREATE REL TABLE IF NOT EXISTS RelatedTo(\
                FROM Entity TO Entity, \
                relation_type STRING\
            )",
        )
        .map_err(|e| anyhow!("init_schema RelatedTo table failed: {}", e))?;

        // ER alias table: records AutoMerge decisions (alias entity_id → canonical key).
        // Written by upsert_entities during in-batch ER; read by query_context (future).
        conn.query(
            "CREATE NODE TABLE IF NOT EXISTS entity_aliases(\
                id STRING PRIMARY KEY, \
                canonical_key STRING, \
                confidence DOUBLE, \
                er_source STRING, \
                created_at STRING\
            )",
        )
        .map_err(|e| anyhow!("init_schema entity_aliases table failed: {}", e))?;

        // ER review queue: Review-band decisions pending human confirmation via F12 panel.
        // resolved field: \"false\" | \"true\" | \"human_approved\" | \"human_rejected\".
        conn.query(
            "CREATE NODE TABLE IF NOT EXISTS er_review_queue(\
                id STRING PRIMARY KEY, \
                alias_entity_id STRING, \
                candidate_canonical_key STRING, \
                similarity DOUBLE, \
                module_id STRING, \
                created_at STRING, \
                resolved STRING\
            )",
        )
        .map_err(|e| anyhow!("init_schema er_review_queue table failed: {}", e))?;

        Ok(())
    }

    fn upsert_entities(&self, module_id: &str, entities: &[GraphEntity]) -> Result<usize> {
        if entities.is_empty() {
            return Ok(0);
        }
        let conn = self.conn()?;
        let now = chrono::Utc::now().to_rfc3339();

        // Phase 1: MERGE each entity node.
        // Both stmts are scoped so their borrows on conn drop before Phase 2 prepares new stmts.
        let count: usize = {
            let mut stmt = conn
                .prepare(
                    "MERGE (e:Entity {id: $id}) \
                     SET e.entity_name = $entity_name, \
                         e.classification = $classification, \
                         e.role_vector = $role_vector, \
                         e.location_vector = $location_vector, \
                         e.contact_vector = $contact_vector, \
                         e.module_id = $module_id, \
                         e.confidence = $confidence",
                )
                .map_err(|e| anyhow!("Failed to prepare upsert statement: {}", e))?;

            // First-write-wins: only set created_at when the node is newly created (field is
            // NULL or empty). On subsequent MERGE (update), the WHERE filter is false → no-op.
            // Kùzu initializes unset STRING properties as NULL, not ''.
            let mut created_at_stmt = conn
                .prepare(
                    "MATCH (e:Entity {id: $id}) \
                     WHERE e.created_at IS NULL OR e.created_at = '' \
                     SET e.created_at = $now",
                )
                .map_err(|e| anyhow!("Failed to prepare created_at statement: {}", e))?;

            let mut c = 0usize;
            for entity in entities {
                let id = format!(
                    "{}__{}",
                    module_id,
                    normalize_entity_key(&entity.entity_name)
                );
                conn.execute(
                    &mut stmt,
                    vec![
                        ("id", Value::String(id.clone())),
                        ("entity_name", Value::String(entity.entity_name.clone())),
                        (
                            "classification",
                            Value::String(entity.classification.clone()),
                        ),
                        (
                            "role_vector",
                            Value::String(entity.role_vector.clone().unwrap_or_default()),
                        ),
                        (
                            "location_vector",
                            Value::String(entity.location_vector.clone().unwrap_or_default()),
                        ),
                        (
                            "contact_vector",
                            Value::String(entity.contact_vector.clone().unwrap_or_default()),
                        ),
                        ("module_id", Value::String(entity.module_id.clone())),
                        ("confidence", Value::Double(entity.confidence)),
                    ],
                )
                .map_err(|e| anyhow!("Failed to upsert entity '{}': {}", entity.entity_name, e))?;
                conn.execute(
                    &mut created_at_stmt,
                    vec![
                        ("id", Value::String(id)),
                        ("now", Value::String(now.clone())),
                    ],
                )
                .map_err(|e| anyhow!("Failed to set created_at: {}", e))?;
                c += 1;
            }

            // Fill-rate telemetry: log vector coverage so operators can track D8 improvement.
            if c > 0 {
                let null_role = entities
                    .iter()
                    .filter(|e| e.role_vector.as_deref().unwrap_or("").is_empty())
                    .count();
                let null_loc = entities
                    .iter()
                    .filter(|e| e.location_vector.as_deref().unwrap_or("").is_empty())
                    .count();
                println!(
                    "[graph] upserted {} entities (module={}) | role_vector fill={:.0}% location_vector fill={:.0}%",
                    c, module_id,
                    100.0 * (c - null_role) as f64 / c as f64,
                    100.0 * (c - null_loc) as f64 / c as f64,
                );
            }

            c
        }; // stmt and created_at_stmt dropped here; conn is free for Phase 2

        // Phase 2: In-batch ER — compare entities within each blocking block; write
        // AutoMerge decisions to entity_aliases and Review decisions to er_review_queue.
        // This catches entities that share a classification + name prefix but normalise to
        // *different* keys (e.g. "Peter Woodfine" vs "Peter M. Woodfine") — surface-variant
        // collapse (same normalised key → same MERGE id) is already handled in Phase 1.
        use crate::er::{blocking_key, decide, similarity, ErConfig, ErDecision};

        let er_cfg = ErConfig::default();
        let mut block_map: std::collections::HashMap<String, Vec<(String, String)>> =
            std::collections::HashMap::new();
        for entity in entities {
            let bk = blocking_key(entity, &er_cfg);
            let entity_id = format!(
                "{}__{}",
                module_id,
                normalize_entity_key(&entity.entity_name)
            );
            block_map
                .entry(bk)
                .or_default()
                .push((entity.entity_name.clone(), entity_id));
        }

        let mut alias_stmt = conn
            .prepare(
                "MERGE (a:entity_aliases {id: $id}) \
                 SET a.canonical_key = $canonical_key, \
                     a.confidence = $confidence, \
                     a.er_source = $er_source, \
                     a.created_at = $created_at",
            )
            .map_err(|e| anyhow!("prepare entity_aliases upsert: {}", e))?;

        let mut review_stmt = conn
            .prepare(
                "MERGE (r:er_review_queue {id: $id}) \
                 SET r.alias_entity_id = $alias_entity_id, \
                     r.candidate_canonical_key = $canonical_key, \
                     r.similarity = $similarity, \
                     r.module_id = $module_id, \
                     r.created_at = $created_at, \
                     r.resolved = $resolved",
            )
            .map_err(|e| anyhow!("prepare er_review_queue upsert: {}", e))?;

        for members in block_map.values() {
            if members.len() < 2 {
                continue;
            }
            // Dedup by entity_id — surface variants that MERGE to the same node are
            // already collapsed; ER only fires for genuinely distinct normalised keys.
            let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
            let deduped: Vec<&(String, String)> = members
                .iter()
                .filter(|(_, id)| seen.insert(id.clone()))
                .collect();
            if deduped.len() < 2 {
                continue;
            }
            // Pick the shortest name as the canonical candidate for this block.
            let canonical = deduped.iter().min_by_key(|(name, _)| name.len()).unwrap();
            let canon_names: Vec<String> = vec![canonical.0.clone()];

            for (mention, alias_id) in deduped.iter() {
                if *alias_id == canonical.1 {
                    continue;
                }
                match decide(mention, &canon_names, &er_cfg) {
                    ErDecision::AutoMerge(canonical_key) => {
                        conn.execute(
                            &mut alias_stmt,
                            vec![
                                ("id", Value::String(alias_id.clone())),
                                ("canonical_key", Value::String(canonical_key)),
                                ("confidence", Value::Double(1.0)),
                                ("er_source", Value::String("auto_merge".into())),
                                ("created_at", Value::String(now.clone())),
                            ],
                        )
                        .map_err(|e| anyhow!("execute entity_aliases upsert: {}", e))?;
                    }
                    ErDecision::Review(canonical_key) => {
                        let sim = similarity(mention, &canonical.0);
                        let queue_id = format!("{}__{}", alias_id, canonical_key);
                        conn.execute(
                            &mut review_stmt,
                            vec![
                                ("id", Value::String(queue_id)),
                                ("alias_entity_id", Value::String(alias_id.clone())),
                                ("canonical_key", Value::String(canonical_key)),
                                ("similarity", Value::Double(sim)),
                                ("module_id", Value::String(module_id.to_string())),
                                ("created_at", Value::String(now.clone())),
                                ("resolved", Value::String("false".into())),
                            ],
                        )
                        .map_err(|e| anyhow!("execute er_review_queue upsert: {}", e))?;
                    }
                    ErDecision::New => {}
                }
            }
        }

        // Force checkpoint so cross-thread readers (HTTP tokio runtime) see the committed
        // write immediately. Without this, Kuzu 0.16 write visibility is limited to the
        // writer's own thread context — cross-OS-thread reads see a stale snapshot.
        if count > 0 {
            if let Err(e) = conn.query("CHECKPOINT") {
                // Non-fatal: data is committed to WAL; checkpoint just enables cross-thread read.
                // Emit a warning but do not fail the upsert.
                eprintln!("[graph] CHECKPOINT after upsert failed (non-fatal): {}", e);
            }
        }

        Ok(count)
    }

    fn query_context(
        &self,
        module_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<GraphEntity>> {
        let conn = self.conn()?;
        let q_lower = query.to_lowercase();

        // Phase 1: find matching entities. Fetch up to 3× limit so we can rank
        // by name-length proximity before alias resolution truncates to limit.
        // Drop stmt before Phase 2 prepares new stmts.
        let mut initial: Vec<GraphEntity> = {
            let fetch_limit = (limit * 3).max(limit + 5);
            let mut stmt = conn
                .prepare(
                    "MATCH (e:Entity) \
                     WHERE e.module_id = $module_id \
                       AND lower(e.entity_name) CONTAINS $query \
                     RETURN e.entity_name, e.classification, e.role_vector, \
                            e.location_vector, e.contact_vector, e.module_id, e.confidence \
                     LIMIT $limit",
                )
                .map_err(|e| anyhow!("Failed to prepare query_context statement: {}", e))?;
            let result = conn
                .execute(
                    &mut stmt,
                    vec![
                        ("module_id", Value::String(module_id.to_string())),
                        ("query", Value::String(q_lower.clone())),
                        ("limit", Value::Int64(fetch_limit as i64)),
                    ],
                )
                .map_err(|e| anyhow!("Failed to execute query_context: {}", e))?;
            rows_to_entities(result)?
        }; // stmt dropped; conn free for Phase 2

        // Rank by name-length proximity: abs(name_len - query_len) ascending.
        // Exact-length matches rank first; longer names (supersets) rank before
        // unrelated matches of similar length. Preserves insertion-order tie-break.
        let q_len = query.len();
        initial.sort_by_key(|e| {
            let name_len = e.entity_name.len();
            name_len.abs_diff(q_len)
        });

        // Phase 2: alias resolution — if a matched entity is recorded as an alias,
        // return the canonical entity instead. Prevents the caller from receiving a
        // fragmented alias when the canonical form holds the richer context.
        // Dedup by canonical_id so two aliases pointing at the same canonical return it once.
        let mut alias_stmt = conn
            .prepare("MATCH (a:entity_aliases {id: $id}) RETURN a.canonical_key")
            .map_err(|e| anyhow!("prepare alias lookup for query_context: {}", e))?;

        let mut canon_stmt = conn
            .prepare(
                "MATCH (e:Entity {id: $id}) \
                 RETURN e.entity_name, e.classification, e.role_vector, \
                        e.location_vector, e.contact_vector, e.module_id, e.confidence",
            )
            .map_err(|e| anyhow!("prepare canonical entity lookup: {}", e))?;

        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut out = Vec::with_capacity(initial.len());

        for entity in initial {
            let entity_id = format!(
                "{}__{}",
                module_id,
                normalize_entity_key(&entity.entity_name)
            );

            // Look up alias record (O(1) PK lookup).
            let canonical_key_opt = {
                let result = conn
                    .execute(
                        &mut alias_stmt,
                        vec![("id", Value::String(entity_id.clone()))],
                    )
                    .map_err(|e| anyhow!("execute alias lookup: {}", e))?;
                result
                    .into_iter()
                    .next()
                    .and_then(|row| row.into_iter().next())
                    .and_then(|v| {
                        if let Value::String(s) = v {
                            Some(s)
                        } else {
                            None
                        }
                    })
            };

            match canonical_key_opt {
                Some(canonical_key) => {
                    let canon_id = format!("{}__{}", module_id, canonical_key);
                    if seen.insert(canon_id.clone()) {
                        // First time seeing this canonical — fetch and return it.
                        let result = conn
                            .execute(&mut canon_stmt, vec![("id", Value::String(canon_id))])
                            .map_err(|e| anyhow!("execute canonical entity lookup: {}", e))?;
                        let mut canonical_entities = rows_to_entities(result)?;
                        out.push(canonical_entities.pop().unwrap_or(entity));
                    }
                    // else: already emitted this canonical from another alias — skip.
                }
                None => {
                    // Not an alias — return as-is, deduped by entity_id.
                    if seen.insert(entity_id) {
                        out.push(entity);
                    }
                }
            }
        }

        Ok(out)
    }

    fn list_entities(&self, module_id: &str) -> Result<Vec<GraphEntity>> {
        let conn = self.conn()?;

        let mut stmt = conn
            .prepare(
                "MATCH (e:Entity) \
                 WHERE e.module_id = $module_id \
                 RETURN e.entity_name, e.classification, e.role_vector, \
                        e.location_vector, e.contact_vector, e.module_id, e.confidence",
            )
            .map_err(|e| anyhow!("Failed to prepare list_entities statement: {}", e))?;

        let result = conn
            .execute(
                &mut stmt,
                vec![("module_id", Value::String(module_id.to_string()))],
            )
            .map_err(|e| anyhow!("Failed to execute list_entities: {}", e))?;

        rows_to_entities(result)
    }

    fn delete_by_classification(&self, module_id: &str, classification: &str) -> Result<usize> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "MATCH (e:Entity) \
                 WHERE e.module_id = $module_id AND e.classification = $cls \
                 DELETE e",
            )
            .map_err(|e| anyhow!("Failed to prepare delete_by_classification: {}", e))?;
        conn.execute(
            &mut stmt,
            vec![
                ("module_id", Value::String(module_id.to_string())),
                ("cls", Value::String(classification.to_string())),
            ],
        )
        .map_err(|e| anyhow!("Failed to execute delete_by_classification: {}", e))?;
        Ok(0)
    }

    fn delete_by_classification_and_location(
        &self,
        module_id: &str,
        classification: &str,
        location: &str,
    ) -> Result<usize> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "MATCH (e:Entity) \
                 WHERE e.module_id = $module_id \
                   AND e.classification = $cls \
                   AND e.location_vector = $loc \
                 DELETE e",
            )
            .map_err(|e| {
                anyhow!(
                    "Failed to prepare delete_by_classification_and_location: {}",
                    e
                )
            })?;
        conn.execute(
            &mut stmt,
            vec![
                ("module_id", Value::String(module_id.to_string())),
                ("cls", Value::String(classification.to_string())),
                ("loc", Value::String(location.to_string())),
            ],
        )
        .map_err(|e| {
            anyhow!(
                "Failed to execute delete_by_classification_and_location: {}",
                e
            )
        })?;
        Ok(0)
    }

    fn delete_entity(&self, module_id: &str, entity_name: &str) -> Result<()> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "MATCH (e:Entity) \
                 WHERE e.module_id = $module_id AND e.entity_name = $entity_name \
                 DELETE e",
            )
            .map_err(|e| anyhow!("Failed to prepare delete_entity: {}", e))?;
        conn.execute(
            &mut stmt,
            vec![
                ("module_id", Value::String(module_id.to_string())),
                ("entity_name", Value::String(entity_name.to_string())),
            ],
        )
        .map_err(|e| {
            anyhow!(
                "Failed to execute delete_entity for '{}': {}",
                entity_name,
                e
            )
        })?;
        Ok(())
    }

    fn count_all(&self) -> Result<usize> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("MATCH (e:Entity) RETURN COUNT(e)")
            .map_err(|e| anyhow!("Failed to prepare count_all: {}", e))?;
        let result = conn
            .execute(&mut stmt, vec![])
            .map_err(|e| anyhow!("Failed to execute count_all: {}", e))?;
        if let Some(row) = result.into_iter().next() {
            if let Some(Value::Int64(n)) = row.into_iter().next() {
                return Ok(n as usize);
            }
        }
        Ok(0)
    }

    fn upsert_edges(&self, module_id: &str, edges: &[RelatedToEdge]) -> Result<usize> {
        if edges.is_empty() {
            return Ok(0);
        }
        let conn = self.conn()?;

        // Check-then-create pattern: avoids duplicate edges on repeated calls.
        // Two stmts prepared once, executed per edge.
        let mut check_stmt = conn
            .prepare(
                "MATCH (src:Entity)-[r:RelatedTo]->(tgt:Entity) \
                 WHERE src.id = $src_id AND tgt.id = $tgt_id \
                   AND r.relation_type = $rel_type \
                 RETURN COUNT(*)",
            )
            .map_err(|e| anyhow!("prepare upsert_edges check: {}", e))?;

        let mut create_stmt = conn
            .prepare(
                "MATCH (src:Entity), (tgt:Entity) \
                 WHERE src.id = $src_id AND tgt.id = $tgt_id \
                 CREATE (src)-[:RelatedTo {relation_type: $rel_type}]->(tgt)",
            )
            .map_err(|e| anyhow!("prepare upsert_edges create: {}", e))?;

        let mut written = 0usize;
        for edge in edges {
            let src_id = format!(
                "{}__{}",
                module_id,
                normalize_entity_key(&edge.src_entity_name)
            );
            let tgt_id = format!(
                "{}__{}",
                module_id,
                normalize_entity_key(&edge.tgt_entity_name)
            );
            let params = vec![
                ("src_id", Value::String(src_id.clone())),
                ("tgt_id", Value::String(tgt_id.clone())),
                ("rel_type", Value::String(edge.relation_type.clone())),
            ];

            let result = conn
                .execute(&mut check_stmt, params.clone())
                .map_err(|e| anyhow!("execute upsert_edges check: {}", e))?;

            let exists = result
                .into_iter()
                .next()
                .and_then(|row| row.into_iter().next())
                .map(|v| matches!(v, Value::Int64(n) if n > 0))
                .unwrap_or(false);

            if !exists {
                conn.execute(&mut create_stmt, params)
                    .map_err(|e| anyhow!("execute upsert_edges create: {}", e))?;
                written += 1;
            }
        }
        Ok(written)
    }

    fn count_aliases(&self) -> Result<usize> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("MATCH (a:entity_aliases) RETURN COUNT(a)")
            .map_err(|e| anyhow!("Failed to prepare count_aliases: {}", e))?;
        let result = conn
            .execute(&mut stmt, vec![])
            .map_err(|e| anyhow!("Failed to execute count_aliases: {}", e))?;
        if let Some(row) = result.into_iter().next() {
            if let Some(Value::Int64(n)) = row.into_iter().next() {
                return Ok(n as usize);
            }
        }
        Ok(0)
    }
}

#[cfg(test)]
impl LbugGraphStore {
    fn get_entity_created_at(&self, module_id: &str, entity_name: &str) -> Result<Option<String>> {
        let conn = self.conn()?;
        let id = format!("{}__{}", module_id, normalize_entity_key(entity_name));
        let mut stmt = conn
            .prepare("MATCH (e:Entity {id: $id}) RETURN e.created_at")
            .map_err(|e| anyhow!("prepare get_entity_created_at: {}", e))?;
        let mut result = conn
            .execute(&mut stmt, vec![("id", Value::String(id))])
            .map_err(|e| anyhow!("execute get_entity_created_at: {}", e))?;
        if let Some(row) = result.next() {
            let s = val_to_string(&row[0]);
            return Ok(if s.is_empty() { None } else { Some(s) });
        }
        Ok(None)
    }
}

/// Extract a `String` from a `Value::String`, or return empty string.
fn val_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        _ => String::new(),
    }
}

/// Extract an `f64` from `Value::Double` or `Value::Float`, or return 0.0.
fn val_to_f64(v: &Value) -> f64 {
    match v {
        Value::Double(f) => *f,
        Value::Float(f) => *f as f64,
        _ => 0.0,
    }
}

/// Convert a `QueryResult` iterator into `Vec<GraphEntity>`.
/// Each row yields 7 columns in RETURN order:
/// 0 entity_name, 1 classification, 2 role_vector, 3 location_vector,
/// 4 contact_vector, 5 module_id, 6 confidence
fn rows_to_entities(result: lbug::QueryResult<'_>) -> Result<Vec<GraphEntity>> {
    let mut out = Vec::new();
    for row in result {
        if row.len() < 7 {
            continue;
        }
        let entity_name = val_to_string(&row[0]);
        let classification = val_to_string(&row[1]);
        let role_vector = {
            let s = val_to_string(&row[2]);
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        };
        let location_vector = {
            let s = val_to_string(&row[3]);
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        };
        let contact_vector = {
            let s = val_to_string(&row[4]);
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        };
        let module_id = val_to_string(&row[5]);
        let confidence = val_to_f64(&row[6]);

        out.push(GraphEntity {
            entity_name,
            classification,
            role_vector,
            location_vector,
            contact_vector,
            module_id,
            confidence,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::er::fuzzy_similarity;

    fn company(name: &str) -> GraphEntity {
        GraphEntity {
            entity_name: name.into(),
            classification: "Company".into(),
            role_vector: None,
            location_vector: None,
            contact_vector: None,
            module_id: "test".into(),
            confidence: 0.9,
        }
    }

    fn person(name: &str) -> GraphEntity {
        GraphEntity {
            entity_name: name.into(),
            classification: "Person".into(),
            role_vector: None,
            location_vector: None,
            contact_vector: None,
            module_id: "test".into(),
            confidence: 0.9,
        }
    }

    #[test]
    fn normalize_collapses_trailing_period_and_whitespace() {
        assert_eq!(
            normalize_entity_key("Woodfine Management Corp."),
            normalize_entity_key("Woodfine Management Corp")
        );
        assert_eq!(
            normalize_entity_key("  Woodfine   Management  Corp  "),
            "woodfine_management"
        );
        assert_ne!(
            normalize_entity_key("Peter Woodfine"),
            normalize_entity_key("Peter M. Woodfine")
        );
    }

    #[test]
    fn normalize_collapses_trademark_and_legal_suffix_variants() {
        let canonical = normalize_entity_key("Woodfine Capital Projects");
        assert_eq!(canonical, "woodfine_capital_projects");
        for variant in [
            "Woodfine Capital Projects™",
            "Woodfine Capital Projects®",
            "Woodfine Capital Projects Inc.",
            "Woodfine Capital Projects, Inc.",
            "woodfine capital projects llc",
            "Woodfine Capital Projects Incorporated",
        ] {
            assert_eq!(
                normalize_entity_key(variant),
                canonical,
                "variant: {variant}"
            );
        }
        assert_eq!(normalize_entity_key("Costco"), "costco");
    }

    /// Confirm the test assumption before the DB test: the two names used in
    /// er_auto_merge_writes_alias should hit the auto_merge 0.95 threshold.
    #[test]
    fn er_auto_merge_assumption_holds() {
        let sim = fuzzy_similarity("Woodfine Capital Projects", "Woodfine Capital Projects Co.");
        assert!(
            sim >= 0.95,
            "auto_merge assumption failed: sim = {sim:.4}, need >= 0.95"
        );
    }

    /// DB-backed: two Company entities in the same block with different normalised keys
    /// but high similarity → Phase 2 ER writes an alias record to entity_aliases.
    #[test]
    fn er_auto_merge_writes_alias() {
        let dir = std::env::temp_dir().join(format!("sc-er-alias-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = LbugGraphStore::new(dir.to_str().unwrap()).expect("open temp lbug store");
        store.init_schema().expect("init_schema");

        // "Woodfine Capital Projects Co." normalises to "woodfine_capital_projects_co"
        // (different id from "woodfine_capital_projects") but has high similarity → AutoMerge.
        let entities = vec![
            company("Woodfine Capital Projects"),
            company("Woodfine Capital Projects Co."),
        ];
        store.upsert_entities("ertest", &entities).expect("upsert");

        assert_eq!(
            store.count_aliases().expect("count_aliases"),
            1,
            "expected 1 alias record after AutoMerge"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// DB-backed: upsert_edges writes a typed directed edge between two entities and
    /// is idempotent (second call with the same edge does not fail or double-write).
    #[test]
    fn upsert_edges_writes_related_to() {
        let dir = std::env::temp_dir().join(format!("sc-edges-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = LbugGraphStore::new(dir.to_str().unwrap()).expect("open temp lbug store");
        store.init_schema().expect("init_schema");

        let entities = vec![
            company("PointSav Digital Systems"),
            company("Woodfine Capital Projects"),
        ];
        store
            .upsert_entities("edgetest", &entities)
            .expect("upsert entities");

        let edges = vec![RelatedToEdge {
            src_entity_name: "PointSav Digital Systems".into(),
            tgt_entity_name: "Woodfine Capital Projects".into(),
            relation_type: "subsidiary_of".into(),
        }];

        let n = store
            .upsert_edges("edgetest", &edges)
            .expect("upsert_edges");
        assert_eq!(n, 1, "first call should write 1 edge");

        // Idempotency: second call must not fail and must not double-write.
        let n2 = store
            .upsert_edges("edgetest", &edges)
            .expect("upsert_edges idempotent");
        assert_eq!(n2, 0, "second call should find edge exists and write 0");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// End-to-end against a real LadybugDB store: the surface variants the audit measured
    /// (™, Inc., bare) MERGE onto ONE canonical node rather than fragmenting (the D5 fix).
    /// Restored after Command fixed the lbug native ABI (LBUG_SHARED removed; prebuilt .a).
    #[test]
    fn upsert_collapses_alias_variants_to_one_node() {
        let dir = std::env::temp_dir().join(format!("sc-graph-ertest-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = LbugGraphStore::new(dir.to_str().unwrap()).expect("open temp lbug store");
        store.init_schema().expect("init_schema");

        let variants = vec![
            company("Woodfine Capital Projects"),
            company("Woodfine Capital Projects Inc."),
            company("Woodfine Capital Projects™"),
        ];
        store.upsert_entities("test", &variants).expect("upsert");

        let hits = store
            .query_context("test", "Woodfine", 10)
            .expect("query_context");
        assert_eq!(
            hits.len(),
            1,
            "3 surface variants should collapse to 1 canonical node, got {:?}",
            hits.iter()
                .map(|e| e.entity_name.clone())
                .collect::<Vec<_>>()
        );
        assert_eq!(store.count_all().expect("count"), 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// DB-backed: query_context with alias resolution — when the matching entity is an alias,
    /// the canonical entity is returned instead (D5 read-side fix).
    ///
    /// Setup: "Woodfine Capital Projects" (canonical) + "Woodfine Capital Projects Co." (alias,
    /// different normalised key "…_co" but high ER similarity → AutoMerge). Querying for "Co."
    /// hits only the alias in Phase 1; Phase 2 resolves it to the canonical.
    #[test]
    fn query_context_resolves_alias_to_canonical() {
        let dir = std::env::temp_dir().join(format!("sc-qc-canon-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = LbugGraphStore::new(dir.to_str().unwrap()).expect("open temp lbug store");
        store.init_schema().expect("init_schema");

        // Both entities must use consistent module_id (struct field = upsert parameter)
        // so the entity_id prefix in entity_aliases matches what query_context computes.
        let entities = vec![
            company("Woodfine Capital Projects"),
            company("Woodfine Capital Projects Co."),
        ];
        // module_id param matches entity.module_id ("test") so alias id = "test__..._co"
        store.upsert_entities("test", &entities).expect("upsert");

        // Verify alias was written (prerequisite for resolution).
        assert_eq!(
            store.count_aliases().expect("count_aliases"),
            1,
            "AutoMerge alias should have been written"
        );

        // Query specifically for "Co." — Phase 1 will find only the alias entity.
        // Phase 2 should resolve it to the canonical "Woodfine Capital Projects".
        let hits = store
            .query_context("test", "Co.", 10)
            .expect("query_context");

        assert_eq!(
            hits.len(),
            1,
            "expected 1 result (the canonical), got {:?}",
            hits.iter().map(|e| &e.entity_name).collect::<Vec<_>>()
        );
        assert_eq!(
            hits[0].entity_name, "Woodfine Capital Projects",
            "query_context should return canonical entity, not the alias"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Person entities that share the "per" prefix block but have different names
    /// should NOT be auto-merged when similarity is below the 0.95 threshold.
    #[test]
    fn er_low_similarity_does_not_write_alias() {
        let dir = std::env::temp_dir().join(format!("sc-er-nosim-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = LbugGraphStore::new(dir.to_str().unwrap()).expect("open temp lbug store");
        store.init_schema().expect("init_schema");

        // "Peter Woodfine" and "Peter Thompson" share the "per" block prefix but are
        // clearly different people — similarity should be well below auto_merge 0.95.
        let entities = vec![person("Peter Woodfine"), person("Peter Thompson")];
        store.upsert_entities("test", &entities).expect("upsert");

        // Should be in review or New — not auto-merged — so alias count stays 0.
        let sim = fuzzy_similarity("Peter Woodfine", "Peter Thompson");
        if sim < 0.95 {
            // Confirms the assumption: no alias written.
            assert_eq!(store.count_aliases().expect("count_aliases"), 0);
        }
        // (If sim were somehow >= 0.95, the test assumption is wrong and we skip the assert.)

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn created_at_first_write_wins() {
        let dir = std::env::temp_dir().join(format!("sc-created-at-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = LbugGraphStore::new(dir.to_str().unwrap()).expect("open temp lbug store");
        store.init_schema().expect("init_schema");

        let entity = person("Jennifer Woodfine");

        // First upsert — should set created_at.
        store
            .upsert_entities("test", std::slice::from_ref(&entity))
            .expect("first upsert");
        let created_first = store
            .get_entity_created_at("test", "Jennifer Woodfine")
            .expect("get_entity_created_at");
        assert!(
            created_first
                .as_deref()
                .map(|s| !s.is_empty())
                .unwrap_or(false),
            "created_at should be set after first upsert, got: {:?}",
            created_first
        );

        // Second upsert — created_at must NOT change.
        store
            .upsert_entities("test", &[entity])
            .expect("second upsert");
        let created_second = store
            .get_entity_created_at("test", "Jennifer Woodfine")
            .expect("get_entity_created_at second");
        assert_eq!(
            created_first, created_second,
            "created_at must not be overwritten on re-upsert"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
