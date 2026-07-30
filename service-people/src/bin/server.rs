use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Contact {
    id: String,
    name: String,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    linkedin_url: Option<String>,
    #[serde(default)]
    timezone: Option<String>,
    #[serde(default)]
    communication_history: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct Ledger {
    contacts: Vec<Contact>,
}

// Row from substrate/ledger_personnel.jsonl — email-pipeline entries.
#[derive(Debug, Deserialize)]
struct SubstrateEntry {
    identity_anchor: String,
}

type AppState = Arc<Vec<Contact>>;

/// Parse "Name <email@domain>" → Some(email); handles quoted names and bare emails.
fn extract_email(identity_anchor: &str) -> Option<String> {
    if let (Some(open), Some(close)) = (identity_anchor.find('<'), identity_anchor.rfind('>')) {
        if open < close {
            let email = identity_anchor[open + 1..close].trim().to_string();
            if !email.is_empty() {
                return Some(email);
            }
        }
    }
    // Bare email with no angle-brackets.
    if identity_anchor.contains('@') && !identity_anchor.contains(' ') {
        return Some(identity_anchor.trim().to_string());
    }
    None
}

/// Extract name from "Name <email@domain>" for use as a join key.
fn extract_name(identity_anchor: &str) -> String {
    let raw = if let Some(open) = identity_anchor.find('<') {
        &identity_anchor[..open]
    } else {
        identity_anchor
    };
    raw.trim().trim_matches('"').trim().to_lowercase()
}

/// Build name→email lookup from a JSONL substrate file.
fn load_substrate_emails(path: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return map,
    };
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(entry) = serde_json::from_str::<SubstrateEntry>(line) {
            let name = extract_name(&entry.identity_anchor);
            if let Some(email) = extract_email(&entry.identity_anchor) {
                map.entry(name).or_insert(email);
            }
        }
    }
    map
}

#[tokio::main]
async fn main() {
    let ledger_path = std::env::var("SERVICE_PEOPLE_LEDGER_PATH").unwrap_or_else(|_| {
        let dir = std::path::Path::new(file!())
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        dir.join("ledger_personnel.json")
            .to_string_lossy()
            .into_owned()
    });

    let substrate_path = std::env::var("SERVICE_PEOPLE_SUBSTRATE_PATH").unwrap_or_else(|_| {
        let dir = std::path::Path::new(file!())
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        dir.join("substrate")
            .join("ledger_personnel.jsonl")
            .to_string_lossy()
            .into_owned()
    });

    let port: u16 = std::env::var("SERVICE_PEOPLE_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(9091);

    let substrate_emails = load_substrate_emails(&substrate_path);

    let content = std::fs::read_to_string(&ledger_path)
        .unwrap_or_else(|e| panic!("Cannot read ledger at {ledger_path}: {e}"));
    let ledger: Ledger =
        serde_json::from_str(&content).unwrap_or_else(|e| panic!("Malformed ledger JSON: {e}"));

    // Enrich contacts: if email not already present in JSON, look up from substrate.
    let contacts: Vec<Contact> = ledger
        .contacts
        .into_iter()
        .map(|mut c| {
            if c.email.is_none() {
                let key = c.name.to_lowercase();
                if let Some(email) = substrate_emails.get(&key) {
                    c.email = Some(email.clone());
                }
            }
            c
        })
        .collect();

    let state: AppState = Arc::new(contacts);

    let app = Router::new()
        .route("/v1/people", get(list_people))
        .route("/v1/people/:id", get(get_person))
        .with_state(state);

    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("Cannot bind {addr}: {e}"));

    println!("[service-people] ready on :{port}");
    axum::serve(listener, app).await.unwrap();
}

async fn list_people(State(contacts): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "contacts": *contacts }))
}

async fn get_person(
    State(contacts): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    contacts
        .iter()
        .find(|c| c.id == id)
        .map(|c| Json(serde_json::json!(c)))
        .ok_or(StatusCode::NOT_FOUND)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_email_angle_brackets() {
        assert_eq!(
            extract_email("abhishek kekane <akekane@redhat.com>"),
            Some("akekane@redhat.com".to_string())
        );
    }

    #[test]
    fn extract_email_quoted_name() {
        assert_eq!(
            extract_email("\"david g. johnston\" <david.g.johnston@gmail.com>"),
            Some("david.g.johnston@gmail.com".to_string())
        );
    }

    #[test]
    fn extract_email_bare() {
        assert_eq!(
            extract_email("user@example.com"),
            Some("user@example.com".to_string())
        );
    }

    #[test]
    fn extract_email_no_email() {
        assert_eq!(extract_email("just a name"), None);
    }

    #[test]
    fn extract_name_normalised() {
        assert_eq!(
            extract_name("Abhishek Kekane <akekane@redhat.com>"),
            "abhishek kekane"
        );
        assert_eq!(
            extract_name("\"David G. Johnston\" <d@g.com>"),
            "david g. johnston"
        );
    }
}
