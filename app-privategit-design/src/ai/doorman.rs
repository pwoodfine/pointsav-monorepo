// DoormanOlmo adapter — relays to local OLMo via Doorman /v1/chat/completions.
// Uses HTTP chunked streaming; Doorman speaks OpenAI-compatible SSE format.

use super::{AiChunk, AiRequest, ChunkStream};
use futures_util::{stream, StreamExt};

pub async fn stream_completion(doorman_url: &str, req: AiRequest) -> ChunkStream {
    let url = format!("{}/v1/chat/completions", doorman_url.trim_end_matches('/'));
    let body = format!(
        r#"{{"model":"local","stream":true,"messages":[{{"role":"user","content":{}}}]}}"#,
        crate::ai::json_str_pub(&format!(
            "You are a design system expert reviewing a {} element.\n\nContext:\n{}\n\nTask:\n{}",
            req.schema, req.context, req.selection
        ))
    );

    let client = reqwest::Client::new();
    let result = client
        .post(&url)
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .await;

    match result {
        Err(e) => Box::pin(stream::once(async move {
            AiChunk::Error(format!("Doorman unreachable: {}", e))
        })),
        Ok(resp) => {
            let s = resp.bytes_stream().flat_map(|chunk| {
                let items: Vec<AiChunk> = match chunk {
                    Err(e) => vec![AiChunk::Error(e.to_string())],
                    Ok(bytes) => parse_sse_chunks(&bytes),
                };
                stream::iter(items)
            });
            Box::pin(s)
        }
    }
}

/// Parse OpenAI-compatible SSE data lines into AiChunks.
fn parse_sse_chunks(bytes: &[u8]) -> Vec<AiChunk> {
    let text = std::str::from_utf8(bytes).unwrap_or("");
    let mut out = Vec::new();
    for line in text.lines() {
        let data = line.strip_prefix("data: ").unwrap_or(line).trim();
        if data == "[DONE]" {
            out.push(AiChunk::Done);
            continue;
        }
        if data.is_empty() {
            continue;
        }
        // Extract delta.content from {"choices":[{"delta":{"content":"..."}}]}
        if let Some(content) = extract_delta_content(data) {
            if !content.is_empty() {
                out.push(AiChunk::Delta(content));
            }
        }
    }
    out
}

fn extract_delta_content(json: &str) -> Option<String> {
    // Hand-rolled extraction — avoids serde dep for this path.
    let after = json.find("\"content\":")?.saturating_add(10);
    let rest = json.get(after..)?.trim_start();
    if rest.starts_with("null") {
        return None;
    }
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
