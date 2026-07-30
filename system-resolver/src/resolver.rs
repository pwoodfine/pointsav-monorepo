use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Command {
    pub intent: String,
    pub target: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct ErrorResponse {
    pub error: String,
}

pub fn resolve(intent: &str) -> Result<Command, ErrorResponse> {
    let lower_intent = intent.to_lowercase();

    if lower_intent.contains("ping") || lower_intent.contains("status") || lower_intent.contains("test") {
        Ok(Command {
            intent: "PING".to_string(),
            target: "ALL".to_string(),
        })
    } else if lower_intent.contains("isolate") || lower_intent.contains("lockdown") || lower_intent.contains("disconnect") {
        Ok(Command {
            intent: "ISOLATE".to_string(),
            target: "ALL".to_string(),
        })
    } else {
        Err(ErrorResponse {
            error: "Unknown Intent".to_string(),
        })
    }
}
