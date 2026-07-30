use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::PathBuf;

fn db_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".local/share/proof/ingest.db")
}

fn open_db() -> Result<Connection> {
    let path = db_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(&path)?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS ingest_log (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            created_at  TEXT    NOT NULL,
            username    TEXT    NOT NULL,
            tenant      TEXT    NOT NULL,
            path        TEXT    NOT NULL,
            ledger_id   TEXT,
            status      TEXT    NOT NULL
        );",
    )?;
    Ok(conn)
}

pub struct IngestRecord {
    pub created_at: String,
    pub username: String,
    pub tenant: String,
    pub path: String,
    pub ledger_id: Option<String>,
    pub status: String,
}

pub fn append(rec: &IngestRecord) -> Result<()> {
    let conn = open_db()?;
    conn.execute(
        "INSERT INTO ingest_log (created_at, username, tenant, path, ledger_id, status)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            rec.created_at,
            rec.username,
            rec.tenant,
            rec.path,
            rec.ledger_id,
            rec.status,
        ],
    )?;
    Ok(())
}

/// Read the most recent `limit` ingest records, newest first.
/// Returns an empty Vec if the table is empty or the DB does not exist yet.
pub fn query_recent(limit: usize) -> Result<Vec<IngestRecord>> {
    let conn = open_db()?;
    let mut stmt = conn.prepare(
        "SELECT created_at, username, tenant, path, ledger_id, status
         FROM ingest_log ORDER BY id DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit as i64], |row| {
        Ok(IngestRecord {
            created_at: row.get(0)?,
            username: row.get(1)?,
            tenant: row.get(2)?,
            path: row.get(3)?,
            ledger_id: row.get(4)?,
            status: row.get(5)?,
        })
    })?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}
