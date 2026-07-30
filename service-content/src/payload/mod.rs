//! Payload Construction for the Linguistic Compiler (LLM API).

use crate::engines::OperationalRules;
use serde::Serialize;

/// Represents a parsed piece of research or donor text from the Fleet wikis.
#[derive(Debug, Clone, Serialize)]
pub struct ContextSnippet {
    pub source_id: String,
    pub content: String,
    pub tags: Vec<String>,
}

/// Builder to construct the strict prompt payload for the Gemini API.
pub struct PayloadBuilder {
    system_instructions: String,
    context_data: Vec<ContextSnippet>,
    target_theme: String,
}

impl PayloadBuilder {
    /// Initializes the payload by translating Fleet rules into system prompts.
    pub fn new(rules: &OperationalRules, theme: &str) -> Self {
        let mut instructions = String::from(
            "You are an automated institutional synthesis engine executing a Data Mesh Synthesis.\n\
            Adhere strictly to ISO 24495-1 Plain Language standards.\n"
        );

        // Dynamically inject operational rules into the system prompt
        if rules.factuality_enforcement.unwrap_or(false) {
            instructions.push_str("MANDATE: Remove all subjective qualifiers. State facts exactly without embellishment.\n");
        }
        if rules.anti_puffery_verbs.unwrap_or(false) {
            instructions.push_str("MANDATE: Forbid absolute accomplishment verbs for active engineering. Use 'is engineered to'.\n");
        }
        if let Some(ref banned) = rules.banned_buzzwords {
            instructions.push_str(&format!("STRICTLY FORBIDDEN WORDS: {:?}\n", banned));
        }

        Self {
            system_instructions: instructions,
            context_data: Vec::new(),
            target_theme: theme.to_string(),
        }
    }

    /// Injects verified research/donor data into the payload.
    pub fn add_context(&mut self, snippet: ContextSnippet) {
        self.context_data.push(snippet);
    }

    /// Finalizes the payload string to be transmitted to the API via secure outbound capability.
    pub fn build(&self) -> String {
        let mut payload = self.system_instructions.clone();
        payload.push_str(&format!("\nTARGET THEME: {}\n\nRAW DATA SUBSTRATE:\n", self.target_theme));
        
        for snippet in &self.context_data {
            payload.push_str(&format!("[{}] {}\n", snippet.source_id, snippet.content));
        }
        
        payload.push_str("\nSYNTHESIS COMMAND: Generate the compliant artifact using ONLY the provided data. Do not invent facts.");
        payload
    }
}
