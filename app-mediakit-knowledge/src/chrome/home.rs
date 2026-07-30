//! Homepage chrome — per-instance differentiated home page layouts.
//!
//! Phase 3: three distinct homepage layouts per §17.2 of BRIEF-knowledge-platform-master.md.
//!
//! - **documentation.pointsav.com** (brand="pointsav"): Category grid (9 tiles,
//!   article count, scope description) + featured article strip + recently-updated.
//!
//! - **projects.woodfinegroup.com** (brand="woodfine", title contains "Projects"):
//!   Thematic cluster cards with editorial headlines and 3–4 article cards per cluster.
//!   "Start here" card per cluster.
//!
//! - **corporate.woodfinegroup.com** (brand="woodfine", title contains "Corporate"):
//!   Two-column layout — "Due Diligence Path" (ordered 5-article sequence) +
//!   "Browse by subject" (category links with counts).

use crate::chrome::{t, Locale};
use maud::{html, Markup};

/// A category tile for the documentation homepage grid.
pub struct CategoryTile {
    pub name: String,
    pub slug: String,
    pub article_count: usize,
    pub description: String,
}

/// A minimal article card (used in clusters, featured strips, and due-diligence lists).
pub struct ArticleCard {
    pub slug: String,
    pub title: String,
    pub summary: String,
    pub status: String,
    pub position: Option<u32>,
    pub featured: bool,
}

/// A thematic cluster group (for projects.woodfinegroup.com).
pub struct ClusterGroup {
    pub headline: String,
    pub articles: Vec<ArticleCard>,
}

/// A leapfrog facts entry (rotating strip).
pub struct LeapfrogFact {
    pub text: String,
}

/// Determine homepage layout variant from brand + site title.
pub fn homepage_variant(brand: &str, site_title: &str) -> &'static str {
    if brand != "woodfine" {
        return "documentation";
    }
    if site_title.to_lowercase().contains("corporate") {
        "corporate"
    } else {
        "projects"
    }
}

/// Render the documentation homepage (category grid + featured + recently updated).
pub fn documentation_home(
    categories: &[CategoryTile],
    featured: Option<&ArticleCard>,
    recently_updated: &[ArticleCard],
    leapfrog: &[LeapfrogFact],
    locale: Locale,
) -> Markup {
    html! {
        div class="home-page home-page--documentation" {
            // Leapfrog facts rotating strip (if present)
            @if !leapfrog.is_empty() {
                div class="leapfrog-strip" aria-label="Platform facts" {
                    @for fact in leapfrog {
                        span class="leapfrog-strip__fact" { (fact.text) }
                    }
                }
            }

            // Category grid
            section class="home-section" aria-labelledby="cats-heading" {
                div class="section-head" {
                    h2 id="cats-heading" { (t(locale, "categories")) }
                }
                div class="cat-grid" {
                    @for tile in categories {
                        a href=(format!("/category/{}", tile.slug)) class="cat-card" {
                            div class="cat-card__head" {
                                span class="cat-card__name" { (tile.name) }
                                span class="cat-card__count" { (tile.article_count) }
                            }
                            p class="cat-card__desc" { (tile.description) }
                        }
                    }
                }
            }

            // Featured article strip
            @if let Some(art) = featured {
                section class="home-section" aria-labelledby="featured-heading" {
                    div class="section-head" {
                        h2 id="featured-heading" { "Featured article" }
                    }
                    div class="featured" {
                        div class="featured__row" {
                            span class="dot" {}
                            "Featured"
                        }
                        h3 class="featured__title" {
                            a href=(format!("/wiki/{}", art.slug)) { (art.title) }
                        }
                        p class="featured__excerpt" { (art.summary) }
                        a href=(format!("/wiki/{}", art.slug)) class="featured__cta" {
                            "Read article →"
                        }
                    }
                }
            }

            // Recently updated strip (L27: title and date in separate elements)
            @if !recently_updated.is_empty() {
                section class="home-section" aria-labelledby="recent-heading" {
                    div class="section-head" {
                        h2 id="recent-heading" { (t(locale, "recently_changed")) }
                    }
                    div class="recent" {
                        @for art in recently_updated {
                            a href=(format!("/wiki/{}", art.slug)) class="recent__item" {
                                div class="recent__info" {
                                    // L27: title in its own element, not concatenated with date
                                    span class="recent__title" { (art.title) }
                                    span class="recent__crumb" { (art.summary) }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Render the projects homepage (thematic cluster cards).
pub fn projects_home(
    clusters: &[ClusterGroup],
    leapfrog: &[LeapfrogFact],
    locale: Locale,
) -> Markup {
    let start_here = t(locale, "start_here");

    html! {
        div class="home-page home-page--projects" {
            // Leapfrog strip
            @if !leapfrog.is_empty() {
                div class="leapfrog-strip" aria-label="Platform facts" {
                    @for fact in leapfrog {
                        span class="leapfrog-strip__fact" { (fact.text) }
                    }
                }
            }

            // Thematic cluster groups
            @for cluster in clusters {
                section class="home-section cluster-group" {
                    div class="section-head" {
                        h2 class="cluster-group__headline" { (cluster.headline) }
                    }
                    div class="cluster-cards" {
                        @for art in &cluster.articles {
                            div class=(if art.featured { "cluster-card cluster-card--start-here" } else { "cluster-card" }) {
                                @if art.featured {
                                    span class="cluster-card__start-badge" { (start_here) }
                                }
                                h3 class="cluster-card__title" {
                                    a href=(format!("/wiki/{}", art.slug)) { (art.title) }
                                }
                                p class="cluster-card__summary" { (art.summary) }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Render the corporate homepage (due-diligence path + browse by subject).
pub fn corporate_home(
    due_diligence: &[ArticleCard],
    categories: &[CategoryTile],
    locale: Locale,
) -> Markup {
    html! {
        div class="home-page home-page--corporate" {
            // "If this is your first visit" link at the top
            @if let Some(first) = due_diligence.first() {
                p class="corporate-first-visit" {
                    "If this is your first visit, "
                    a href=(format!("/wiki/{}", first.slug)) { "start here" }
                    "."
                }
            }

            div class="corporate-layout" {
                // Left column: Due Diligence Path
                section class="corporate-dd-path" aria-labelledby="dd-heading" {
                    h2 id="dd-heading" { "Due Diligence Path" }
                    ol class="dd-list" {
                        @for art in due_diligence {
                            li class="dd-list__item" {
                                div class="dd-list__header" {
                                    @if let Some(pos) = art.position {
                                        span class="dd-list__pos" { (pos) }
                                    }
                                    a href=(format!("/wiki/{}", art.slug)) class="dd-list__title" {
                                        (art.title)
                                    }
                                    span class=(format!("status-badge status-badge--{}", crate::chrome::article::status_css_class_pub(&art.status))) {
                                        (crate::chrome::article::status_badge_text(&art.status, locale))
                                    }
                                }
                                @if !art.summary.is_empty() {
                                    p class="dd-list__desc" { (art.summary) }
                                }
                            }
                        }
                    }
                    @if due_diligence.len() < 5 {
                        a href="/category/corporate" class="dd-more-link" {
                            "More topics below"
                        }
                    }
                }

                // Right column: Browse by subject
                section class="corporate-browse" aria-labelledby="browse-heading" {
                    h2 id="browse-heading" { "Browse by subject" }
                    ul class="browse-list" {
                        @for cat in categories {
                            li class="browse-list__item" {
                                a href=(format!("/category/{}", cat.slug)) {
                                    span class="browse-list__name" { (cat.name) }
                                    span class="browse-list__count" { (cat.article_count) }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
