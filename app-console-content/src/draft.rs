use std::io::{BufRead, BufReader};
use std::sync::mpsc::Sender;
use std::time::Duration;

use serde::Deserialize;

pub enum DraftEvent {
    Token(String),
    Done,
    Error(String),
}

#[derive(Deserialize)]
struct SseDelta {
    content: Option<String>,
}

#[derive(Deserialize)]
struct SseChoice {
    delta: SseDelta,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct SseChunk {
    choices: Vec<SseChoice>,
}

/// POST to `<slm_endpoint>/v1/chat/completions` with streaming enabled.
/// Sends `DraftEvent::Token` for each streamed token, then `DraftEvent::Done`.
/// Runs synchronously — call from a background thread.
pub fn stream_draft(
    prompt: &str,
    protocol: &str,
    tenant: &str,
    slm_endpoint: &str,
    tx: Sender<DraftEvent>,
) {
    let url = format!("{}/v1/chat/completions", slm_endpoint.trim_end_matches('/'));

    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            let _ = tx.send(DraftEvent::Error(format!("client build: {}", e)));
            return;
        }
    };

    let body = serde_json::json!({
        "model": "local",
        "stream": true,
        "messages": [
            {
                "role": "system",
                "content": format!(
                    "You are a professional writer composing a {}. \
                     Tenant: {}. Follow the foundry prose-protocol standard for this \
                     document type. Write the document directly without preamble or \
                     meta-commentary.",
                    protocol, tenant
                )
            },
            {
                "role": "user",
                "content": prompt
            }
        ]
    });

    let resp = match client.post(&url).json(&body).send() {
        Ok(r) => r,
        Err(e) => {
            let _ = tx.send(DraftEvent::Error(format!("request: {}", e)));
            return;
        }
    };

    if !resp.status().is_success() {
        let status = resp.status();
        let body_text = resp.text().unwrap_or_default();
        let _ = tx.send(DraftEvent::Error(format!("HTTP {}: {}", status, body_text)));
        return;
    }

    let reader = BufReader::new(resp);
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                let _ = tx.send(DraftEvent::Error(format!("read: {}", e)));
                return;
            }
        };
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }
        let Some(data) = line.strip_prefix("data: ") else {
            continue;
        };
        if data == "[DONE]" {
            let _ = tx.send(DraftEvent::Done);
            return;
        }
        let Ok(chunk) = serde_json::from_str::<SseChunk>(data) else {
            continue;
        };
        for choice in &chunk.choices {
            if let Some(content) = &choice.delta.content {
                if !content.is_empty() && tx.send(DraftEvent::Token(content.clone())).is_err() {
                    return; // receiver dropped — user navigated away
                }
            }
            if choice.finish_reason.is_some() {
                let _ = tx.send(DraftEvent::Done);
                return;
            }
        }
    }
    let _ = tx.send(DraftEvent::Done);
}
