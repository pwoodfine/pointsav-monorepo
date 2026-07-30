use anyhow::Result;
use rusqlite::{params, Connection};

use crate::pairing::PairRequestBody;

pub fn insert_request(
    conn: &Connection,
    request_id: &str,
    code: &str,
    body: &PairRequestBody,
    created_at: &str,
    expires_at: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO pairing_requests
         (request_id, code, username, tenant, fingerprint, public_key, state, created_at, expires_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending', ?7, ?8)",
        params![
            request_id,
            code,
            body.username,
            body.tenant,
            body.fingerprint,
            body.public_key,
            created_at,
            expires_at
        ],
    )?;
    Ok(())
}

pub fn get_state_by_id(conn: &Connection, request_id: &str) -> Result<Option<String>> {
    match conn.query_row(
        "SELECT state FROM pairing_requests WHERE request_id = ?1",
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
    // Returns (request_id, username, tenant, fingerprint, public_key)
    match conn.query_row(
        "SELECT request_id, username, tenant, fingerprint, public_key
         FROM pairing_requests WHERE code = ?1 AND state = 'pending'",
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
        "UPDATE pairing_requests SET state = ?1 WHERE request_id = ?2",
        params![state, request_id],
    )?;
    Ok(n)
}

#[allow(clippy::type_complexity)]
pub fn list_pending(conn: &Connection) -> Result<Vec<(String, String, String, String, String)>> {
    // Returns (request_id, code, username, tenant, created_at)
    let mut stmt = conn.prepare(
        "SELECT request_id, code, username, tenant, created_at
         FROM pairing_requests WHERE state = 'pending'
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
        "UPDATE pairing_requests SET state = 'expired'
         WHERE state = 'pending' AND expires_at < ?1",
        params![now],
    )?;
    Ok(n)
}
