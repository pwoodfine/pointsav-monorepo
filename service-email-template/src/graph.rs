use reqwest::{Client, header};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::env;

// --- Authentication Structures ---
#[derive(Deserialize)]
struct AuthResponse {
    access_token: String,
}

// --- Folder Structures ---
#[derive(Deserialize, Debug)]
pub struct Folder {
    pub id: String,
    pub display_name: Option<String>,
}

#[derive(Deserialize)]
struct FolderListResponse {
    value: Vec<Folder>,
}

// --- Message Structures ---
#[derive(Deserialize, Debug)]
pub struct Message {
    pub id: String,
    pub subject: Option<String>,
}

#[derive(Deserialize)]
struct MessageListResponse {
    value: Vec<Message>,
}

pub struct GraphClient {
    client: Client,
    access_token: String,
}

impl GraphClient {
    /// Authenticates with M365 using the Identity.env credentials
    pub async fn authenticate() -> Result<Self, Box<dyn Error>> {
        let tenant_id = env::var("MSFT_TENANT_ID")?;
        let client_id = env::var("MSFT_CLIENT_ID")?;
        let client_secret = env::var("MSFT_CLIENT_SECRET")?;

        let auth_url = format!(
            "https://login.microsoftonline.com/{}/oauth2/v2.0/token",
            tenant_id
        );

        let params = [
            ("client_id", client_id.as_str()),
            ("scope", "https://graph.microsoft.com/.default"),
            ("client_secret", client_secret.as_str()),
            ("grant_type", "client_credentials"),
        ];

        let http_client = Client::new();
        let res = http_client.post(&auth_url).form(&params).send().await?;

        if !res.status().is_success() {
            return Err(format!("Auth Failed: {}", res.status()).into());
        }

        let auth_data: AuthResponse = res.json().await?;

        Ok(Self {
            client: http_client,
            access_token: auth_data.access_token,
        })
    }

    /// Helper to inject the Bearer token into requests
    fn auth_headers(&self) -> header::HeaderMap {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            header::HeaderValue::from_str(&format!("Bearer {}", self.access_token)).unwrap(),
        );
        headers.insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/json"),
        );
        headers
    }

    /// Locates the 'Template Ledger' folder and its sub-folders for a specific user
    pub async fn get_folders(&self, user_email: &str) -> Result<Vec<Folder>, Box<dyn Error>> {
        let url = format!("https://graph.microsoft.com/v1.0/users/{}/mailFolders", user_email);
        
        let res = self.client.get(&url)
            .headers(self.auth_headers())
            .send().await?;

        let data: FolderListResponse = res.json().await?;
        Ok(data.value)
    }

    /// Purges (Deletes) any message containing the [TMPL-] telemetry key in a specific folder
    pub async fn purge_old_templates(&self, user_email: &str, folder_id: &str, telemetry_key: &str) -> Result<(), Box<dyn Error>> {
        // 1. Find the messages with the specific key
        let search_url = format!(
            "https://graph.microsoft.com/v1.0/users/{}/mailFolders/{}/messages?$search=\"{}\"",
            user_email, folder_id, telemetry_key
        );

        let res = self.client.get(&search_url).headers(self.auth_headers()).send().await?;
        let data: MessageListResponse = res.json().await?;

        // 2. Delete them
        for msg in data.value {
            let delete_url = format!(
                "https://graph.microsoft.com/v1.0/users/{}/messages/{}",
                user_email, msg.id
            );
            self.client.delete(&delete_url).headers(self.auth_headers()).send().await?;
            println!("      [PURGE] Deleted old version of {}", telemetry_key);
        }

        Ok(())
    }

    /// Injects a compiled .eml (MIME) directly into the target folder
    pub async fn inject_template(&self, user_email: &str, folder_id: &str, mime_content: Vec<u8>) -> Result<(), Box<dyn Error>> {
        let url = format!(
            "https://graph.microsoft.com/v1.0/users/{}/mailFolders/{}/messages",
            user_email, folder_id
        );

        // Microsoft Graph requires MIME content to be base64 encoded for JSON injection
        let base64_mime = base64::encode(&mime_content);

        let payload = serde_json::json!({
            "@odata.type": "#microsoft.graph.message",
            "isDraft": false, // Injects as a received email so it doesn't get consumed on forward
            "isRead": true,   // Mark as read so it doesn't trigger inbox notifications
            "body": {
                "contentType": "html",
                "content": "This is a base64 MIME injection placeholder. The actual payload will overwrite this."
            }
        });
        
        // Note: For raw MIME injection, Graph API requires a slightly different endpoint structure 
        // using the $value endpoint. We will refine the exact POST structure in the final compiler logic.
        
        Ok(())
    }
}
