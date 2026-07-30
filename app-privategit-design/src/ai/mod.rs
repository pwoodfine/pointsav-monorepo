// D5 — AI bridge: upstream model trait + adapters.
// POST /ai/session relays to DoormanOlmo (local) or ClaudeCloud (remote via X-Api-Key header).

use futures_util::Stream;
use std::pin::Pin;

pub mod claude;
pub mod doorman;

/// Normalised chunk from any upstream model.
pub enum AiChunk {
    Delta(String),
    Done,
    Error(String),
}

/// SSE-encoded representation of a chunk.
impl AiChunk {
    pub fn to_sse(&self) -> String {
        match self {
            AiChunk::Delta(t) => {
                format!("data: {{\"type\":\"delta\",\"text\":{}}}\n\n", json_str(t))
            }
            AiChunk::Done => "data: {\"type\":\"done\"}\n\n".to_string(),
            AiChunk::Error(e) => {
                format!("data: {{\"type\":\"error\",\"msg\":{}}}\n\n", json_str(e))
            }
        }
    }
}

pub type ChunkStream = Pin<Box<dyn Stream<Item = AiChunk> + Send>>;

pub struct AiRequest {
    pub selection: String,
    pub schema: String,
    pub context: String,
}

pub fn json_str_pub(s: &str) -> String {
    json_str(s)
}

fn json_str(s: &str) -> String {
    let e = s
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r");
    format!("\"{}\"", e)
}
