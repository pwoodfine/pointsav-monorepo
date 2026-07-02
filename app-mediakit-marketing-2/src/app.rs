//! Application state and axum Router. Grows each phase — P0 mounts
//! `/healthz` and `/static/*path`; P1 adds bare (unstyled) page routes to
//! prove the content pipeline end to end. Real chrome/design system lands in
//! P2/P3 (see `ui` module, added then).

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path as AxumPath, State};
use axum::routing::get;
use axum::Router;
use maud::{html, Markup};

use crate::config::Config;
use crate::content::{self, Page, Section};
use crate::error::MarketingError;

pub struct AppStateInner {
    pub content_dir: PathBuf,
    pub module_id: String,
}

pub type AppState = Arc<AppStateInner>;

pub fn build_state(cfg: &Config) -> AppState {
    Arc::new(AppStateInner {
        content_dir: cfg.content_dir.clone(),
        module_id: cfg.module_id.clone(),
    })
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/static/{*path}", get(crate::assets::serve))
        .route("/", get(home))
        .route("/es", get(home_es))
        .route("/page/{slug}", get(page))
        .route("/es/page/{slug}", get(page_es))
        .with_state(state)
}

async fn healthz() -> &'static str {
    "ok"
}

async fn home(State(state): State<AppState>) -> Result<Markup, MarketingError> {
    render_slug(&state, "home", None)
}

async fn home_es(State(state): State<AppState>) -> Result<Markup, MarketingError> {
    render_slug(&state, "home", Some("es"))
}

async fn page(
    State(state): State<AppState>,
    AxumPath(slug): AxumPath<String>,
) -> Result<Markup, MarketingError> {
    render_slug(&state, &slug, None)
}

async fn page_es(
    State(state): State<AppState>,
    AxumPath(slug): AxumPath<String>,
) -> Result<Markup, MarketingError> {
    render_slug(&state, &slug, Some("es"))
}

fn render_slug(state: &AppStateInner, slug: &str, lang: Option<&str>) -> Result<Markup, MarketingError> {
    let page = content::load_page(&state.content_dir, slug, lang)?;
    Ok(render_bare(&page))
}

/// Bare (unstyled) page render — proves the content pipeline works end to
/// end. No chrome, no tokens, no brand dispatch: those land in P2/P3.
fn render_bare(page: &Page) -> Markup {
    html! {
        (maud::DOCTYPE)
        html lang=(page.lang) {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (page.title) }
                meta name="description" content=(page.description);
                link rel="stylesheet" href="/static/app.css";
            }
            body {
                main {
                    @for section in &page.sections {
                        (render_section(section))
                    }
                }
                script src="/static/app.js" {}
            }
        }
    }
}

fn render_section(section: &Section) -> Markup {
    match section {
        Section::Hero { headline, subhead } => html! {
            section.hero {
                h1 { (headline) }
                @if let Some(sub) = subhead {
                    p { (sub) }
                }
            }
        },
        Section::CardGrid { columns: _, cards } => html! {
            section.card-grid {
                @for card in cards {
                    div.card {
                        @if let Some(href) = &card.href {
                            a href=(href) { (card.title.clone()) }
                        } @else {
                            (card.title.clone())
                        }
                        @if let Some(body) = &card.body {
                            p { (body) }
                        }
                    }
                }
            }
        },
        Section::Prose { body } => {
            let rendered = content::render_markdown(body);
            html! {
                section.prose {
                    (maud::PreEscaped(rendered))
                }
            }
        }
    }
}
