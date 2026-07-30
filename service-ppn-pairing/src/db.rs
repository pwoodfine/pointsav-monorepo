use anyhow::Result;
use rusqlite::{params, Connection};

use system_pairing_codes::NodeJoinRequestBody;

pub fn open_db() -> Result<Connection> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let dir = std::path::PathBuf::from(home).join(".local/share/ppn");
    std::fs::create_dir_all(&dir)?;
    let conn = Connection::open(dir.join("ppn-pairing.db"))?;
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS node_join_requests (
            request_id      TEXT PRIMARY KEY,
            code            TEXT NOT NULL,
            node_id         TEXT NOT NULL,
            wireguard_pubkey TEXT NOT NULL,
            bottom          TEXT NOT NULL,
            arch            TEXT NOT NULL,
            state           TEXT NOT NULL DEFAULT 'pending',
            created_at      TEXT NOT NULL,
            expires_at      TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_code ON node_join_requests(code);",
    )?;
    Ok(())
}

pub fn insert_request(
    conn: &Connection,
    request_id: &str,
    code: &str,
    body: &NodeJoinRequestBody,
    created_at: &str,
    expires_at: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO node_join_requests
         (request_id, code, node_id, wireguard_pubkey, bottom, arch, state, created_at, expires_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending', ?7, ?8)",
        params![
            request_id,
            code,
            body.node_id,
            body.wireguard_pubkey,
            body.bottom,
            body.arch,
            created_at,
            expires_at
        ],
    )?;
    Ok(())
}

pub fn get_state_by_id(conn: &Connection, request_id: &str) -> Result<Option<String>> {
    match conn.query_row(
        "SELECT state FROM node_join_requests WHERE request_id = ?1",
        params![request_id],
        |row| row.get::<_, String>(0),
    ) {
        Ok(s) => Ok(Some(s)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

#[allow(clippy::type_complexity)]
pub fn get_by_code(
    conn: &Connection,
    code: &str,
) -> Result<Option<(String, String, String, String, String)>> {
    // Returns (request_id, node_id, wireguard_pubkey, bottom, arch)
    match conn.query_row(
        "SELECT request_id, node_id, wireguard_pubkey, bottom, arch
         FROM node_join_requests WHERE code = ?1 AND state = 'pending'",
        params![code],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        },
    ) {
        Ok(r) => Ok(Some(r)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn set_state(conn: &Connection, request_id: &str, state: &str) -> Result<usize> {
    let n = conn.execute(
        "UPDATE node_join_requests SET state = ?1 WHERE request_id = ?2",
        params![state, request_id],
    )?;
    Ok(n)
}

#[allow(clippy::type_complexity)]
pub fn list_pending(conn: &Connection) -> Result<Vec<(String, String, String, String, String)>> {
    // Returns (request_id, code, node_id, bottom, created_at)
    let mut stmt = conn.prepare(
        "SELECT request_id, code, node_id, bottom, created_at
         FROM node_join_requests WHERE state = 'pending'
         ORDER BY created_at",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
        ))
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn sweep_expired(conn: &Connection) -> Result<usize> {
    let now = chrono::Utc::now().to_rfc3339();
    let n = conn.execute(
        "UPDATE node_join_requests SET state = 'expired'
         WHERE state = 'pending' AND expires_at < ?1",
        params![now],
    )?;
    Ok(n)
}

/// Append an approved node record to ~/.local/share/ppn/nodes.jsonl.
/// Phase 1: simple append-only log. WireGuard mesh provisioning reads this file.
pub fn register_approved_node(
    node_id: &str,
    wireguard_pubkey: &str,
    bottom: &str,
    arch: &str,
    request_id: &str,
) -> Result<()> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let dir = std::path::PathBuf::from(home).join(".local/share/ppn");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("nodes.jsonl");
    let entry = serde_json::json!({
        "node_id": node_id,
        "wireguard_pubkey": wireguard_pubkey,
        "bottom": bottom,
        "arch": arch,
        "request_id": request_id,
        "approved_at": chrono::Utc::now().to_rfc3339(),
    });
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    writeln!(file, "{}", entry)?;
    Ok(())
}
