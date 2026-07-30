// ClaudeCloud adapter — relays to api.anthropic.com/v1/messages with streaming.
// Credential comes from the per-request X-Api-Key header; never stored in AppState.

use super::{AiChunk, AiRequest, ChunkStream};
use futures_util::{stream, StreamExt};

pub async fn stream_completion(api_key: &str, req: AiRequest) -> ChunkStream {
    let body = format!(
        r#"{{"model":"claude-sonnet-4-6","max_tokens":1024,"stream":true,"messages":[{{"role":"user","content":{}}}]}}"#,
        crate::ai::json_str_pub(&format!(
            "You are a design system expert reviewing a {} element.\n\nContext:\n{}\n\nTask:\n{}",
            req.schema, req.context, req.selection
        ))
    );

    let client = reqwest::Client::new();
    let result = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .body(body)
        .send()
        .await;

    match result {
        Err(e) => Box::pin(stream::once(async move {
            AiChunk::Error(format!("Claude API unreachable: {}", e))
        })),
        Ok(resp) => {
            let s = resp.bytes_stream().flat_map(|chunk| {
                let items: Vec<AiChunk> = match chunk {
                    Err(e) => vec![AiChunk::Error(e.to_string())],
                    Ok(bytes) => parse_anthropic_sse(&bytes),
                };
                stream::iter(items)
            });
            Box::pin(s)
        }
    }
}

/// Parse Anthropic SSE stream into AiChunks.
/// Anthropic sends `event: content_block_delta` with `{"delta":{"type":"text_delta","text":"..."}}`.
fn parse_anthropic_sse(bytes: &[u8]) -> Vec<AiChunk> {
    let text = std::str::from_utf8(bytes).unwrap_or("");
    let mut out = Vec::new();
    let mut last_event = "";
    for line in text.lines() {
        if let Some(ev) = line.strip_prefix("event: ") {
            last_event = ev.trim();
            continue;
        }
        let data = match line.strip_prefix("data: ") {
            Some(d) => d.trim(),
            None => continue,
        };
        match last_event {
            "content_block_delta" => {
                if let Some(t) = extract_text_delta(data) {
                    if !t.is_empty() {
                        out.push(AiChunk::Delta(t));
                    }
                }
            }
            "message_stop" => out.push(AiChunk::Done),
            _ => {}
        }
    }
    out
}

fn extract_text_delta(json: &str) -> Option<String> {
    let after = json.find("\"text\":")?.saturating_add(7);
    let rest = json.get(after..)?.trim_start();
    if let Some(inner) = rest.strip_prefix('"') {
        let end = inner.find('"')?;
        Some(
            inner[..end]
                .replace("\\n", "\n")
                .replace("\\\"", "\"")
                .replace("\\\\", "\\"),
        )
    } else {
        None
    }
}
