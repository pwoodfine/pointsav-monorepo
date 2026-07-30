use reqwest::Client;
use serde::Deserialize;
use std::error::Error;

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
}

pub async fn negotiate_token(
    tenant_id: &str,
    client_id: &str,
    client_secret: &str,
) -> Result<String, Box<dyn Error>> {
    let url = format!("https://login.microsoftonline.com/{}/oauth2/v2.0/token", tenant_id);
    let client = Client::new();

    let params = [
        ("grant_type", "client_credentials"),
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("scope", "https://graph.microsoft.com/.default"),
    ];

    let response = client.post(&url).form(&params).send().await?;

    if response.status().is_success() {
        let token_res: TokenResponse = response.json().await?;
        Ok(token_res.access_token)
    } else {
        let error_text = response.text().await?;
        Err(format!("OAuth2 Negotiation Failed: {}", error_text).into())
    }
}
