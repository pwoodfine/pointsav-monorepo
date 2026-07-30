use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::PathBuf;

use crate::user::{Tenant, User};

pub fn db_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".local/share/proof/proof.db")
}

pub fn open_db() -> Result<Connection> {
    let path = db_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(&path)?;
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS users (
            id          INTEGER PRIMARY KEY,
            fingerprint TEXT    UNIQUE NOT NULL,
            username    TEXT    NOT NULL,
            tenant      TEXT    NOT NULL CHECK(tenant IN ('pointsav','woodfine')),
            role        TEXT    NOT NULL DEFAULT 'editor',
            active      INTEGER NOT NULL DEFAULT 1,
            created_at  TEXT    NOT NULL
        );
        CREATE TABLE IF NOT EXISTS pairing_requests (
            request_id  TEXT PRIMARY KEY,
            code        TEXT UNIQUE NOT NULL,
            username    TEXT NOT NULL,
            tenant      TEXT NOT NULL,
            fingerprint TEXT NOT NULL,
            public_key  TEXT NOT NULL,
            role        TEXT NOT NULL DEFAULT 'editor',
            state       TEXT NOT NULL DEFAULT 'pending',
            attempts    INTEGER NOT NULL DEFAULT 0,
            created_at  TEXT NOT NULL,
            expires_at  TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_pairing_code ON pairing_requests(code);",
    )?;
    Ok(())
}

pub fn find_user(conn: &Connection, fingerprint: &str) -> Result<Option<User>> {
    let result = conn.query_row(
        "SELECT username, tenant, role FROM users WHERE fingerprint = ?1 AND active = 1",
        params![fingerprint],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        },
    );
    match result {
        Ok((username, tenant_str, role)) => {
            let tenant = Tenant::from_str(&tenant_str).unwrap_or(Tenant::Pointsav);
            Ok(Some(User {
                username,
                tenant,
                role,
            }))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn add_user(
    conn: &Connection,
    fingerprint: &str,
    username: &str,
    tenant: &str,
    role: &str,
) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO users (fingerprint, username, tenant, role, active, created_at)
         VALUES (?1, ?2, ?3, ?4, 1, ?5)",
        params![fingerprint, username, tenant, role, now],
    )?;
    Ok(())
}

#[allow(clippy::type_complexity)]
pub fn list_users(conn: &Connection) -> Result<Vec<(String, String, String, String, bool)>> {
    let mut stmt = conn.prepare(
        "SELECT fingerprint, username, tenant, role, active FROM users ORDER BY created_at",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, i32>(4)? == 1,
        ))
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn disable_user(conn: &Connection, username: &str) -> Result<usize> {
    let n = conn.execute(
        "UPDATE users SET active = 0 WHERE username = ?1",
        params![username],
    )?;
    Ok(n)
}

pub fn rotate_key(conn: &Connection, username: &str, new_fingerprint: &str) -> Result<usize> {
    let n = conn.execute(
        "UPDATE users SET fingerprint = ?1 WHERE username = ?2 AND active = 1",
        params![new_fingerprint, username],
    )?;
    Ok(n)
}
