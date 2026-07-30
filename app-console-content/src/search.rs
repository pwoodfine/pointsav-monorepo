use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SearchResult {
    pub title: String,
    pub slug: String,
    #[serde(default)]
    pub excerpt: String,
}

#[derive(Deserialize)]
struct SearchResponse {
    results: Vec<SearchResult>,
}

/// GET `{endpoint}/v1/search?q={query}&limit=20` — 3s timeout.
/// Returns Vec<SearchResult>. On HTTP error or service unavailable, returns Err.
pub fn fetch_search(endpoint: &str, query: &str) -> anyhow::Result<Vec<SearchResult>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()?;
    let url = format!("{}/v1/search", endpoint.trim_end_matches('/'));
    let resp = client
        .get(&url)
        .query(&[("q", query), ("limit", "20")])
        .send()?;
    if resp.status().is_success() {
        Ok(resp.json::<SearchResponse>()?.results)
    } else {
        anyhow::bail!("HTTP {}", resp.status())
    }
}
