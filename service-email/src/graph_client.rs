use reqwest::Client;
use serde_json::{Value, json};
use std::error::Error;

pub struct GraphBridge {
    client: Client,
    token: String,
}

impl GraphBridge {
    pub fn new(token: String) -> Self {
        Self {
            client: Client::new(),
            token,
        }
    }

    // Phase 1: High-Velocity URL Extraction (Accepts initial query or nextLink)
    pub async fn fetch_url(&self, url: &str) -> Result<Value, Box<dyn Error>> {
        let res = self.client.get(url)
            .bearer_auth(&self.token)
            .send()
            .await?;

        if !res.status().is_success() {
            return Err(format!("Graph API extraction failed: {}", res.status()).into());
        }

        let body = res.json::<Value>().await?;
        Ok(body)
    }

    // Phase 2: State Mutation
    pub async fn mutate_state(&self, target_user: &str, message_id: &str) -> Result<(), Box<dyn Error>> {
        let url = format!(
            "https://graph.microsoft.com/v1.0/users/{}/messages/{}",
            target_user, message_id
        );
        let res = self.client.patch(&url)
            .bearer_auth(&self.token)
            .json(&json!({"isRead": true}))
            .send()
            .await?;

        if !res.status().is_success() {
            return Err(format!("State mutation failed: {}", res.status()).into());
        }
        Ok(())
    }
}
