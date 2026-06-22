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
        " incorporated", " inc", " corporation", " corp", " company",
        " limited", " ltd", " llc", " gmbh", " plc",
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
    /// Count all Entity nodes in the graph across all modules. Used by /healthz to
    /// surface the real entity count rather than always reporting 0.
    fn count_all(&self) -> Result<usize>;
    /// Delete a single entity by module_id + entity_name. Returns Ok(()) on success
    /// or if the entity did not exist. Used by the /v1/graph/cleanup endpoint.
    fn delete_entity(&self, module_id: &str, entity_name: &str) -> Result<()>;
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

    /// Create a new connection from the stored database reference.
    /// Connection borrows `&Database`; since `Arc<Database>` keeps the Database alive
    /// and we hold a reference for the duration of the call, this is safe.
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

        Ok(())
    }

    fn upsert_entities(&self, module_id: &str, entities: &[GraphEntity]) -> Result<usize> {
        let conn = self.conn()?;

        // Prepare the MERGE statement once, then execute per entity.
        // LadybugDB MERGE semantics: create-or-match on primary key; SET updates fields on match.
        let mut stmt = conn
            .prepare(
                "MERGE (e:Entity {id: $id}) \
                 SET e.entity_name = $entity_name, \
                     e.classification = $classification, \
                     e.role_vector = $role_vector, \
                     e.location_vector = $location_vector, \
                     e.contact_vector = $contact_vector, \
                     e.module_id = $module_id, \
                     e.confidence = $confidence, \
                     e.created_at = $created_at",
            )
            .map_err(|e| anyhow!("Failed to prepare upsert statement: {}", e))?;

        let now = chrono::Utc::now().to_rfc3339();
        let mut count = 0usize;

        for entity in entities {
            let id = format!(
                "{}__{}",
                module_id,
                normalize_entity_key(&entity.entity_name)
            );

            conn.execute(
                &mut stmt,
                vec![
                    ("id", Value::String(id)),
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
                    ("created_at", Value::String(now.clone())),
                ],
            )
            .map_err(|e| anyhow!("Failed to upsert entity '{}': {}", entity.entity_name, e))?;

            count += 1;
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
                    ("query", Value::String(q_lower)),
                    ("limit", Value::Int64(limit as i64)),
                ],
            )
            .map_err(|e| anyhow!("Failed to execute query_context: {}", e))?;

        rows_to_entities(result)
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
        // Result is a single row with one column: the integer count.
        if let Some(row) = result.into_iter().next() {
            if let Some(Value::Int64(n)) = row.into_iter().next() {
                return Ok(n as usize);
            }
        }
        Ok(0)
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
    use super::normalize_entity_key;

    #[test]
    fn normalize_collapses_trailing_period_and_whitespace() {
        // The Corp./Corp duplication the audit flagged collapses to one key.
        assert_eq!(
            normalize_entity_key("Woodfine Management Corp."),
            normalize_entity_key("Woodfine Management Corp")
        );
        // Internal whitespace + case normalised; the "Corp" suffix is now stripped.
        assert_eq!(
            normalize_entity_key("  Woodfine   Management  Corp  "),
            "woodfine_management"
        );
        // Distinct real entities are NOT merged (surface-variant collapse only).
        assert_ne!(
            normalize_entity_key("Peter Woodfine"),
            normalize_entity_key("Peter M. Woodfine")
        );
    }

    #[test]
    fn normalize_collapses_trademark_and_legal_suffix_variants() {
        // The Woodfine Capital Projects 5-6-way Company fragmentation collapses to one key.
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
            assert_eq!(normalize_entity_key(variant), canonical, "variant: {variant}");
        }
        // Two-letter ambiguous abbreviations are NOT stripped (no false merges).
        assert_eq!(normalize_entity_key("Costco"), "costco");
    }
}
