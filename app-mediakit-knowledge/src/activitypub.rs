//! ActivityPub federation — Phase 7 scaffold.
//!
//! Emits `Create/Article` Activities to a configurable outbox URL
//! when an article is created or edited. Follows AS2 / ActivityStreams 2.0.
//!
//! SYS-ADR-19 compliance: this module DOES NOT auto-publish to verified ledgers.
//! It emits to a configurable HTTP endpoint; the operator controls whether that
//! endpoint is a public ActivityPub inbox or an internal relay.
//!
//! Phase 7 implementation — Sprint H (2026-06-16):
//!   1. `[federation].outbox_url` added to knowledge.toml + `AppConfig` (config.rs).
//!   2. `AppState.activitypub_outbox_url` populated from config at startup (main.rs).
//!   3. `on_article_saved()` wired into the content-dir file watcher (main.rs).
//!      Note: git-only contribution workflow means no HTTP POST edit handler exists;
//!      the file watcher fires on any `.md` change to the content directory.

use serde::{Deserialize, Serialize};

/// AS2 Actor representing one wiki instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Actor {
    #[serde(rename = "@context")]
    pub context: String,
    pub id: String,
    #[serde(rename = "type")]
    pub actor_type: String,
    pub name: String,
    pub inbox: String,
    pub outbox: String,
}

impl Actor {
    pub fn new(base_url: &str, instance_label: &str) -> Self {
        Actor {
            context: "https://www.w3.org/ns/activitystreams".to_string(),
            id: format!("{}/actor", base_url),
            actor_type: "Service".to_string(),
            name: instance_label.to_string(),
            inbox: format!("{}/activitypub/inbox", base_url),
            outbox: format!("{}/activitypub/outbox", base_url),
        }
    }
}

/// AS2 Article object representing one wiki article.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Article {
    #[serde(rename = "@context")]
    pub context: String,
    pub id: String,
    #[serde(rename = "type")]
    pub object_type: String,
    pub name: String,
    pub url: String,
    pub content: String,
    pub published: String,
}

impl Article {
    pub fn new(base_url: &str, slug: &str, title: &str, summary: &str, published: &str) -> Self {
        Article {
            context: "https://www.w3.org/ns/activitystreams".to_string(),
            id: format!("{}/wiki/{}", base_url, slug),
            object_type: "Article".to_string(),
            name: title.to_string(),
            url: format!("{}/wiki/{}", base_url, slug),
            content: summary.to_string(),
            published: published.to_string(),
        }
    }
}

/// AS2 Create activity wrapping an Article.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateActivity {
    #[serde(rename = "@context")]
    pub context: String,
    pub id: String,
    #[serde(rename = "type")]
    pub activity_type: String,
    pub actor: String,
    pub object: Article,
}

impl CreateActivity {
    pub fn new(actor_id: &str, activity_id: &str, article: Article) -> Self {
        CreateActivity {
            context: "https://www.w3.org/ns/activitystreams".to_string(),
            id: activity_id.to_string(),
            activity_type: "Create".to_string(),
            actor: actor_id.to_string(),
            object: article,
        }
    }
}

/// Emit a `Create/Article` Activity to `outbox_url` when an article is saved.
/// Best-effort: logs on error, never panics, never blocks the caller.
///
/// Called from the content-dir file watcher after a successful `.md` reindex.
/// `outbox_url` comes from `knowledge.toml` `[federation].outbox_url`; if absent,
/// this is a no-op.
pub async fn on_article_saved(
    outbox_url: Option<&str>,
    actor_id: &str,
    base_url: &str,
    slug: &str,
    title: &str,
    summary: &str,
    published: &str,
) {
    let Some(url) = outbox_url else { return };
    let article = Article::new(base_url, slug, title, summary, published);
    let activity_id = format!("{}/activitypub/activities/{}", base_url, slug);
    let activity = CreateActivity::new(actor_id, &activity_id, article);

    let client = reqwest::Client::new();
    if let Err(e) = client
        .post(url)
        .json(&activity)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
    {
        tracing::warn!(outbox = %url, slug = %slug, err = %e, "activitypub: delivery failed");
    }
}
