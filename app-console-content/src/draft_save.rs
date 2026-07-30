use chrono::Utc;
use rusqlite::{params, Connection, Result as SqlResult};
use std::path::PathBuf;

const DB_FILE: &str = ".local/share/proof/content_session.db";
const MAX_AGE_HOURS: i64 = 24;

pub struct DraftSave {
    path: PathBuf,
}

pub struct SavedDraft {
    pub protocol: String,
    pub content: String,
}

impl DraftSave {
    pub fn open() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        DraftSave {
            path: PathBuf::from(home).join(DB_FILE),
        }
    }

    fn connect(&self) -> SqlResult<Connection> {
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(&self.path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS content_session (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                created_at  TEXT    NOT NULL,
                protocol    TEXT    NOT NULL,
                content     TEXT    NOT NULL
            );",
        )?;
        Ok(conn)
    }

    /// Persist the current draft — replaces any prior saved draft.
    pub fn save(&self, protocol: &str, content: &str) {
        if let Ok(conn) = self.connect() {
            let now = Utc::now().to_rfc3339();
            let _ = conn.execute("DELETE FROM content_session;", []);
            let _ = conn.execute(
                "INSERT INTO content_session (created_at, protocol, content) VALUES (?1, ?2, ?3);",
                params![now, protocol, content],
            );
        }
    }

    /// Load the most recent saved draft if it was saved within MAX_AGE_HOURS.
    pub fn load(&self) -> Option<SavedDraft> {
        let conn = self.connect().ok()?;
        let mut stmt = conn
            .prepare(
                "SELECT created_at, protocol, content \
                 FROM content_session ORDER BY id DESC LIMIT 1;",
            )
            .ok()?;
        let row = stmt
            .query_row([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .ok()?;

        let created: chrono::DateTime<chrono::Utc> = row.0.parse().ok()?;
        if (Utc::now() - created).num_hours() > MAX_AGE_HOURS {
            return None;
        }
        if row.2.trim().is_empty() {
            return None;
        }
        Some(SavedDraft {
            protocol: row.1,
            content: row.2,
        })
    }

    /// Clear the saved draft (called on successful submission or explicit /clear).
    pub fn clear(&self) {
        if let Ok(conn) = self.connect() {
            let _ = conn.execute("DELETE FROM content_session;", []);
        }
    }
}
