async fn bucket_topics_by_category(
    content_dir: &FsPath,
    guide_dir: Option<&FsPath>,
    guide_dir_2: Option<&FsPath>,
) -> std::io::Result<CategoryBuckets> {
    let topic_files = collect_all_topic_files(content_dir, &[guide_dir, guide_dir_2]).await?;
    let mut buckets: CategoryBuckets = BTreeMap::new();

    for tf in topic_files {
        let text = match fs::read_to_string(&tf.path).await {
            Ok(t) => t,
            Err(_) => continue,
        };

        let parsed = match crate::render::parse_page(&text) {
            Ok(p) => p,
            Err(_) => continue,
        };

        let title = parsed
            .frontmatter
            .title
            .clone()
            .unwrap_or_else(|| tf.slug.clone());

        // Category: prefer frontmatter `category:`, fall back to the
        // subdirectory name extracted from the slug.
        let category = match parsed.frontmatter.category.as_deref() {
            None | Some("root") | Some("") => {
                // Infer from slug prefix if file is in a subdirectory.
                if let Some(slash) = tf.slug.find('/') {
                    tf.slug[..slash].to_string()
                } else {
                    "uncategorised".to_string()
                }
            }
            Some(c) => c.to_string(),
        };

        let lede_first_line = first_body_line(&parsed.body_md);
        let last_edited = parsed.frontmatter.last_edited.clone();
        let short_description = parsed.frontmatter.short_description.clone();

        let summary = TopicSummary {
            slug: tf.slug,
            title,
            last_edited,
            short_description,
            status: parsed.frontmatter.status.clone(),
            lede_first_line,
            file_path: tf.path,
        };

        buckets.entry(category).or_default().push(summary);
    }

    // Sort each bucket by slug for deterministic output.
    for topics in buckets.values_mut() {
        topics.sort_by(|a, b| a.slug.cmp(&b.slug));
    }

    Ok(buckets)
}

/// Extract a lede from the first non-blank, non-heading Markdown line.
fn first_body_line(body_md: &str) -> String {
    body_md
        .lines()
        .find(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with('#') && !t.starts_with("---")
        })
        .map(|l| l.trim().to_string())
        .unwrap_or_default()
}

/// Flatten all buckets, sort by `last_edited` descending (filename ascending
/// as tiebreaker), and return the top `n` entries.
///
/// Topics with `last_edited: None` fall back to git-commit-date via
/// filesystem mtime. Topics that cannot produce any date sort last.
fn recent_topics_by_last_edited(buckets: &CategoryBuckets, n: usize) -> Vec<TopicSummary> {
    let mut all: Vec<TopicSummary> = buckets.values().flatten().cloned().collect();

    // Resolve a sort key for each entry: prefer `last_edited` frontmatter,
    // then filesystem mtime. We use a String key so ISO-8601 / unix-seconds
    // lexicographic order == chronological order.
    let key_for = |t: &TopicSummary| -> String {
        if let Some(ref d) = t.last_edited {
            return d.clone();
        }
        // Fall back to filesystem mtime (fast — no subprocess).
        if let Ok(meta) = std::fs::metadata(&t.file_path) {
            if let Ok(modified) = meta.modified() {
                let dur = modified
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default();
                return format!("{}", dur.as_secs());
            }
        }
        String::new()
    };

    all.sort_by(|a, b| {
        let ka = key_for(a);
        let kb = key_for(b);
        // Descending by date, ascending by slug as tiebreaker.
        kb.cmp(&ka).then_with(|| a.slug.cmp(&b.slug))
    });

    all.truncate(n);
    all
}

/// Read and validate `<content_dir>/featured-topic.yaml`.
///
/// Returns `None` silently if the file is absent. Logs a warning via
/// `tracing::warn!` if the file is present but unparseable or if the slug
/// cannot be found in `buckets`.
async fn load_featured(content_dir: &FsPath, buckets: &CategoryBuckets) -> Option<FeaturedArticle> {
    let path = content_dir.join("featured-topic.yaml");
    let text = fs::read_to_string(path).await.ok()?;
    let pin: FeaturedTopicPin = serde_yaml::from_str(&text).ok()?;

    // Find the topic summary in buckets to get title and snippet
    let summary = buckets.values().flatten().find(|t| t.slug == pin.slug)?;

    Some(FeaturedArticle {
        title: summary.title.clone(),
        slug: summary.slug.clone(),
        snippet: summary.short_description.clone().unwrap_or_default(),
    })
}

async fn load_dyk(content_dir: &FsPath) -> Option<LeapfrogFacts> {
    let path = content_dir.join("leapfrog-facts.yaml");
    let text = fs::read_to_string(path).await.ok()?;
    serde_yaml::from_str(&text).ok()
}

async fn load_reference_invariants(content_dir: &FsPath) -> Option<ReferenceInvariants> {
    let path = content_dir.join("reference-invariants.yaml");
    let text = fs::read_to_string(path).await.ok()?;
    serde_yaml::from_str(&text).ok()
}

fn extract_short_description(text: &str) -> Option<String> {
    let after_first = text.strip_prefix("---\n")?;
    let end = after_first.find("\n---")?;
    let fm_text = &after_first[..end];
    let val: serde_yaml::Value = serde_yaml::from_str(fm_text).ok()?;
    val.get("short_description")?
        .as_str()
        .map(|s| s.to_string())
}

async fn load_category_descriptions(
    content_dir: &FsPath,
    categories: &[&str],
) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for cat in categories {
        let path = content_dir.join(cat).join("_index.md");
        if let Ok(text) = fs::read_to_string(&path).await {
            if let Some(desc) = extract_short_description(&text) {
                map.insert(cat.to_string(), desc);
            }
        }
    }
    map
}

async fn load_dyk_localized(content_dir: &FsPath, locale: Locale) -> Option<LeapfrogFacts> {
    if locale == Locale::Es {
        let es_path = content_dir.join(format!("leapfrog-facts{}.yaml", locale.suffix()));
        if es_path.exists() {
            let text = fs::read_to_string(&es_path).await.ok()?;
            if let Ok(facts) = serde_yaml::from_str(&text) {
                return Some(facts);
            }
        }
    }
    load_dyk(content_dir).await
}

/// Compute home-page stats banner contents.
///
/// `article_count` is the total number of bucketed topics across all
/// categories (excludes `index.md`, `_index.md`, and `*.es.md` siblings,
/// matching `bucket_topics_by_category()` discipline).
///
/// `category_count` is `RATIFIED_CATEGORIES.len()`, signalling
/// the platform's intended scope rather than only categories with
/// articles.
///
/// `last_updated` is the maximum `last_edited:` ISO-8601 string across
/// all bucketed topics. Returns `None` if no topic carries the field
/// (the banner suppresses the date in that case rather than rendering an
/// empty value).
fn compute_home_stats(buckets: &CategoryBuckets) -> HomeStats {
    let article_count: usize = buckets.values().map(|v| v.len()).sum();
    let last_updated = buckets
        .values()
        .flatten()
        .filter_map(|t| t.last_edited.as_deref())
        .max()
        .map(|s| s.to_string());
    HomeStats {
        article_count,
        category_count: HOMEPAGE_CATEGORIES.len(),
        last_updated,
    }
}

// ─── Home-page chrome ───────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn home_chrome(
    locale: Locale,
    home_fm: &crate::render::Frontmatter,
    home_html: &str,
    buckets: &CategoryBuckets,
    recent: &[TopicSummary],
    stats: &HomeStats,
    guides: &[TopicSummary],
    featured: Option<FeaturedArticle>,
    _dyk: Option<LeapfrogFacts>,
    _ref_inv: Option<ReferenceInvariants>,
    cat_descriptions: &BTreeMap<String, String>,
    site_title: &str,
    brand_theme: Option<&str>,
    brand_instance: &str,
    peers: &[crate::config::PeerConfig],
    start_here: &[crate::config::StartHereEntry],
    user: Option<&User>,
    _pending_count: i64,
) -> Markup {
    let woodfine_theme = matches!(brand_theme, Some("woodfine") | Some("woodfine-projects"));
    let _title = home_fm.title.as_deref().unwrap_or(site_title);
    let auth_attr = if user.is_some() { "user" } else { "anon" };
    let tenant = Tenant::from_str(brand_instance);
    let lang_href = match locale { Locale::En => "/es/", Locale::Es => "/?noredirect=1" };
    let s = strings(locale);
    let section_featured = if matches!(locale, Locale::En) {
        match brand_instance {
            "projects"  => "Research highlight",
            "corporate" => "Platform overview",
            _           => s.section_featured,
        }
    } else {
        s.section_featured
    };
    let section_start = if matches!(locale, Locale::En) && matches!(brand_instance, "projects" | "corporate") {
        "Key topics"
    } else {
        s.section_start
    };

    // Articles in non-ratified buckets (not already shown as guides) so that
    // every TOPIC and GUIDE is reachable from the home page.
    let guide_slug_set: std::collections::HashSet<&str> =
        guides.iter().map(|g| g.slug.as_str()).collect();
    let mut uncategorised: Vec<&TopicSummary> = buckets
        .iter()
        .filter(|(cat, _)| !RATIFIED_CATEGORIES.contains(&cat.as_str()) && cat.as_str() != "root")
        .flat_map(|(_, topics)| topics.iter())
        .filter(|t| !guide_slug_set.contains(t.slug.as_str()))
        .collect();
    uncategorised.sort_by(|a, b| a.title.cmp(&b.title));

    // Format an integer with comma separators (e.g. 1234 → "1,234").
    fn fmt_commas(n: usize) -> String {
        let s = n.to_string();
        let mut out = String::new();
        let offset = s.len() % 3;
        for (i, ch) in s.chars().enumerate() {
            if i > 0 && (i + 3 - offset) % 3 == 0 {
                out.push(',');
            }
            out.push(ch);
        }
        out
    }

    html! {
        (DOCTYPE)
        html lang=(locale.lang_attr())
             data-auth=(auth_attr)
             data-instance=(brand_instance) {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover";
                title { (site_title) }
                // Font preload — eliminates FOUT on first load
                link rel="preload" as="font" type="font/woff2" crossorigin href="/static/fonts/Inter-400-normal-latin.woff2";
                link rel="preload" as="font" type="font/woff2" crossorigin href="/static/fonts/Source-Serif-4-400-normal-latin.woff2";
                link rel="stylesheet" href="/static/tokens.css";
                link rel="stylesheet" href="/static/style.css";
                @if woodfine_theme {
                    // Brand override loads AFTER style.css so its :root tokens (e.g. --accent)
                    // win over style.css's defaults — otherwise the per-brand theme is dead.
                    link rel="stylesheet" href="/static/tokens-woodfine.css";
                }
                // Anti-FOUT: apply stored theme before first paint
                script { (PreEscaped(r#"(function(){var t=localStorage.getItem('wiki-theme')||'light';document.documentElement.setAttribute('data-theme',t);var w=localStorage.getItem('wiki-width')||'standard';document.documentElement.setAttribute('data-width',w);}());"#)) }
                // hreflang + canonical for bilingual home
                @match locale {
                    Locale::En => {
                        link rel="alternate" hreflang="es" href="/es/";
                        link rel="canonical" href="/";
                    }
                    Locale::Es => {
                        link rel="alternate" hreflang="en" href="/";
                        link rel="canonical" href="/es/";
                    }
                }
            }
            body {
                a class="skip-to-content" href="#mp-main" { "Skip to content" }
                (sovereign_nav(tenant, locale.lang_attr(), site_title, lang_href))
                (sovereign_mobile_nav_drawer(tenant, site_title))
                main class="site-main" id="mp-main" {

                    // ── Editorial front page (Wikipedia-pattern two-column) ──
                    div.wiki-home-editorial #mp-topbanner {
                        div.wiki-home-editorial__left {

                            // Featured article
                            @if let Some(ref featured) = featured {
                                div.featured #mp-tfa {
                                    div.featured__row {
                                        span.dot {}
                                        (section_featured)
                                    }
                                    h2.featured__title {
                                        a href={ "/wiki/" (featured.slug) } { (featured.title) }
                                    }
                                    @if !featured.snippet.is_empty() {
                                        p.featured__excerpt { (featured.snippet) }
                                    }
                                    a.featured__cta href={ "/wiki/" (featured.slug) } {
                                        (s.featured_cta)
                                        (PreEscaped(r#"<svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="M1 7h12M8 2l5 5-5 5"/></svg>"#))
                                    }
                                    div.featured__row style="margin-top:16px;font-size:12px;gap:6px;" {
                                        a href="/special/all-pages" { "Archive" }
                                        " · "
                                        a href="/feed.atom" { "Subscribe" }
                                        " · "
                                        a href="/wiki/about" { "About" }
                                    }
                                }
                            }

                        }

                        div.wiki-home-editorial__right {
                            // Stats block
                            div.wiki-home-stats {
                                strong { (fmt_commas(stats.article_count)) }
                                @if brand_instance == "documentation" {
                                    " articles across "
                                    strong { (HOMEPAGE_CATEGORIES.len()) }
                                    " categories"
                                } @else {
                                    " articles"
                                }
                            }
                            // Home lede / intro text
                            @if !home_html.is_empty() {
                                div.wiki-home-lede { (PreEscaped(home_html)) }
                            }
                            // Recently updated
                            @if !recent.is_empty() {
                                div.section-head #mp-itn {
                                    h2 { (s.section_recent) }
                                    a.section-head__hint href="/special/recent-changes" { (s.recent_all_link) }
                                }
                                ul.recent {
                                    @for t in recent.iter().take(8) {
                                        li.recent__item {
                                            a.recent__title href={ "/wiki/" (t.slug) } { (t.title) }
                                            @if let Some(cat) = t.slug.split_once('/').map(|(c, _)| c) {
                                                span.recent__crumb { (humanize_category(cat)) }
                                            }
                                            span.kind-badge data-type=(item_type_key(&t.slug)) { (item_type_label(&t.slug)) }
                                            @if let Some(ref d) = t.last_edited {
                                                " "
                                                time.recent__date datetime=(d) { (d) }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // ── Start here strip ─────────────────────────────────────
                    // When [[start_here]] entries are present in knowledge.toml,
                    // render them; otherwise fall back to four hardcoded PointSav
                    // documentation chips (documentation instance default).
                    div.section-head { h2 { (section_start) } }
                    div.starthere-row {
                        @if !start_here.is_empty() {
                            @for chip in start_here {
                                @let badge_label = {
                                    let mut s = chip.kind.clone();
                                    if let Some(c) = s.get_mut(0..1) { c.make_ascii_uppercase(); }
                                    s
                                };
                                a.starthere-chip href=(chip.href) {
                                    span.kind-badge data-type=(chip.kind) { (badge_label) }
                                    " " (chip.label)
                                }
                            }
                        } @else {
                            a.starthere-chip href="/wiki/architecture/economic-model" {
                                span.kind-badge data-type="topic" { "Topic" }
                                " Platform business model"
                            }
                            a.starthere-chip href="/wiki/architecture/three-ring-architecture" {
                                span.kind-badge data-type="topic" { "Topic" }
                                " Three-ring architecture"
                            }
                            a.starthere-chip href="/wiki/architecture/compounding-substrate" {
                                span.kind-badge data-type="topic" { "Topic" }
                                " Compounding substrate"
                            }
                            a.starthere-chip href="/wiki/reference/nomenclature-taxonomy" {
                                span.kind-badge data-type="topic" { "Topic" }
                                " Naming conventions"
                            }
                        }
                    }

                    // ── Browse by area — documentation instance only ─────────
                    @if brand_instance == "documentation" {
                        div.section-head {
                            h2 { (s.section_browse) }
                        }
                        div.cat-grid {
                            @for (display_name, primary_slug, description, slugs) in HOMEPAGE_CATEGORIES {
                                @let count: usize = slugs.iter()
                                    .flat_map(|s| buckets.get(*s).map(|v| v.as_slice()).unwrap_or(&[]).iter())
                                    .filter(|t| t.status.as_deref() != Some("stub"))
                                    .count();
                                a.cat-card href={ "/category/" (primary_slug) } {
                                    div.cat-card__head {
                                        span.cat-card__name { (display_name) }
                                        @if count > 0 {
                                            span.cat-card__count { (count) }
                                        }
                                    }
                                    @let desc_text = cat_descriptions
                                        .get(*primary_slug)
                                        .map(|s| s.as_str())
                                        .filter(|s| !s.is_empty())
                                        .unwrap_or(description);
                                    @if !desc_text.is_empty() {
                                        p.cat-card__desc { (desc_text) }
                                    } @else if count == 0 {
                                        p.cat-card__desc.cat-card__desc--empty { "In preparation." }
                                    }
                                }
                            }
                        }
                    }

                    // ── Operational guides ───────────────────────────────────
                    @if !guides.is_empty() {
                        div.section-head {
                            h2 { (s.section_guides) }
                            a.section-head__hint href="/special/all-pages" { (s.guides_all_link) }
                        }
                        div.recent {
                            @for g in guides.iter().take(6) {
                                a.recent__item href={ "/wiki/" (g.slug) } {
                                    div {
                                        span.recent__title { (g.title) }
                                        @if !g.lede_first_line.is_empty() {
                                            span.recent__crumb { (g.lede_first_line) }
                                        }
                                    }
                                    @if let Some(cat) = g.slug.split_once('/').map(|(c, _)| c) {
                                        span.recent__cat { (humanize_category(cat)) }
                                    }
                                    @if let Some(ref d) = g.last_edited {
                                        span.recent__date { (d) }
                                    }
                                }
                            }
                        }
                    }

                }

                // ── Stats one-liner (footer, subtle) ─────────────────────────
                @if stats.article_count > 0 {
                    p.home-stats-oneliner {
                        (fmt_commas(stats.article_count))
                        " articles"
                        @if brand_instance == "documentation" {
                            " · "
                            (HOMEPAGE_CATEGORIES.len())
                            " categories"
                        }
                        @if let Some(ref d) = stats.last_updated {
                            " · Updated " (d)
                        }
                    }
                }

                // ── Also on this platform (cross-instance discovery band) ────
                @if !peers.is_empty() {
                    aside.peer-band {
                        span.peer-band__label { "Also on this platform" }
                        @for peer in peers {
                            a.peer-band__link href=(peer.url) rel="noopener" {
                                (peer.label)
                                (PreEscaped(r#"<svg class="peer-band__arrow" viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true"><path d="M2 5h6M6 3l2 2-2 2"/></svg>"#))
                            }
                        }
                    }
                }

                div #continue-reading-strip hidden="true" {}
                (sovereign_footer(tenant, None))
                script src="/static/wiki.js" defer="true" {}
            }
        }
    }
}

/// Locale-keyed UI strings for the home-page chrome.
///
/// Returns a struct with labels for every string rendered by `home_chrome` so
/// that the `/es/` route can render navigation labels, section headings, and
/// list titles in Spanish without duplicating the template.
struct HomeStrings {
    section_browse: &'static str,
    section_featured: &'static str,
    section_recent: &'static str,
    _lang_toggle_label: &'static str,
    section_start: &'static str,
    section_guides: &'static str,
    featured_cta: &'static str,
    recent_all_link: &'static str,
    guides_all_link: &'static str,
}

fn strings(locale: Locale) -> HomeStrings {
    match locale {
        Locale::En => HomeStrings {
            section_browse: "Browse by area",
            section_featured: "Featured article",
            section_recent: "Recently updated",
            _lang_toggle_label: "ES",
            section_start: "New here? Start with these",
            section_guides: "Operational guides",
            featured_cta: "Read the full article",
            recent_all_link: "All changes →",
            guides_all_link: "All guides →",
        },
        Locale::Es => HomeStrings {
            section_browse: "Explorar por área",
            section_featured: "Artículo destacado",
            section_recent: "Actualizado recientemente",
            _lang_toggle_label: "EN",
            section_start: "¿Primera vez aquí? Empieza con estos",
            section_guides: "Guías operacionales",
            featured_cta: "Leer el artículo completo",
            recent_all_link: "Todos los cambios →",
            guides_all_link: "Todas las guías →",
        },
    }
}

/// Convert a category slug to a display label: hyphens become spaces, each word title-cased.
/// E.g. "design-system" → "Design System", "substrate" → "Substrate".
fn humanize_category(s: &str) -> String {
    // Known acronyms / brand tokens that should render upper-case in nav + titles.
    const ACRONYMS: &[(&str, &str)] = &[
        ("bim", "BIM"),
        ("gis", "GIS"),
        ("os", "OS"),
        ("slm", "SLM"),
        ("worm", "WORM"),
        ("ai", "AI"),
        ("mba", "MBA"),
        ("ppn", "PPN"),
        ("ews", "EWS"),
        ("imap", "IMAP"),
        ("vpn", "VPN"),
        ("udp", "UDP"),
        ("api", "API"),
        ("ui", "UI"),
        ("ux", "UX"),
        ("sel4", "seL4"),
        ("ifc", "IFC"),
        ("pdf", "PDF"),
        ("svg", "SVG"),
        ("id", "ID"),
    ];
    s.split('-')
        .map(|word| {
            if let Some((_, up)) = ACRONYMS.iter().find(|(low, _)| *low == word) {
                return (*up).to_string();
            }
            let mut c = word.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().to_string() + c.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Returns a display label ("Guide" or "Topic") based on the slug stem.
fn item_type_label(slug: &str) -> &'static str {
    if slug.rsplit('/').next().is_some_and(|s| s.starts_with("guide-")) { "Guide" } else { "Topic" }
}

/// Returns a CSS data-type key ("guide" or "topic") based on the slug stem.
fn item_type_key(slug: &str) -> &'static str {
    if slug.rsplit('/').next().is_some_and(|s| s.starts_with("guide-")) { "guide" } else { "topic" }
}

/// Per-content-dir cache entry: (instant the bucket was computed, shared bucket).
type NavCacheEntry = (std::time::Instant, Arc<CategoryBuckets>);
/// Map from content directory to its cached bucket entry.
type NavCacheMap = std::collections::HashMap<PathBuf, NavCacheEntry>;

/// Process-global cache of the category buckets used to build the left
/// navigation, keyed by content directory (one entry per running instance;
/// tests with distinct temp dirs do not collide). A short TTL keeps the nav
/// fast on every article page while still reflecting edits within ~20 s.
static NAV_CACHE: std::sync::OnceLock<tokio::sync::RwLock<NavCacheMap>> =
    std::sync::OnceLock::new();
const NAV_TTL: std::time::Duration = std::time::Duration::from_secs(20);

/// Return the category buckets for the left navigation, parsing the content
/// tree at most once per [`NAV_TTL`] window. The expensive frontmatter parse
/// happens on a cache miss; warm requests clone a cheap `Arc`.
async fn nav_buckets_cached(state: &AppState) -> Arc<CategoryBuckets> {
    let cache =
        NAV_CACHE.get_or_init(|| tokio::sync::RwLock::new(std::collections::HashMap::new()));
    {
        let r = cache.read().await;
        if let Some((built, buckets)) = r.get(state.primary_path()) {
            if built.elapsed() < NAV_TTL {
                return buckets.clone();
            }
        }
    }
    let gds = state.guide_dirs_arr();
    let buckets = Arc::new(
        bucket_topics_by_category(
            state.primary_path(),
            gds[0],
            gds[1],
        )
        .await
        .unwrap_or_default(),
    );
    let mut w = cache.write().await;
    w.insert(
        state.primary_path().to_path_buf(),
        (std::time::Instant::now(), buckets.clone()),
    );
    buckets
}

// ─── Placeholder index (index.md absent) ───────────────────────────────────

/// Current flat-listing index behaviour, preserved for the absent-`index.md`
/// case. Extracted verbatim from the pre-iteration-1 `index()` handler.
async fn placeholder_index(
    state: &AppState,
    user: Option<&User>,
    pending_count: i64,
) -> Result<Markup, WikiError> {
    let mut entries = fs::read_dir(state.primary_path()).await?;
    let mut pages: Vec<String> = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if let Some(slug) = name.strip_suffix(".md") {
            // Skip bilingual siblings, system/repo files.
            if !slug.ends_with(".es") && !SYSTEM_FILE_STEMS.contains(&slug) {
                pages.push(slug.to_string());
            }
        }
    }
    pages.sort();

    Ok(chrome(
        "Index",
        html! {
            h1 { "PointSav Knowledge" }
            p.lede {
                "Flat-file Markdown source-of-truth, single-binary engine, AI-optional. "
                "Phase 1 — render."
            }
            h2 { "Pages" }
            @if pages.is_empty() {
                p.empty { "No pages in content directory yet." }
            } @else {
                ul.page-list {
                    @for slug in &pages {
                        li {
                            a href=(format!("/wiki/{slug}")) { (slug) }
                        }
                    }
                }
            }
        },
        &state.site_title,
        user,
        pending_count,
    ))
}

// ─── Category listing handler (Wave 5B) ─────────────────────────────────────

async fn category_page(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    CurrentUser(maybe_user): CurrentUser,
) -> Result<Markup, WikiError> {
    let pending_count = pending_count_for(&state, maybe_user.as_ref()).await;
    let gds = state.guide_dirs_arr();
    let buckets = bucket_topics_by_category(
        state.primary_path(),
        gds[0],
        gds[1],
    )
    .await?;
    let empty: Vec<TopicSummary> = Vec::new();
    let topics = buckets.get(&name).unwrap_or(&empty);
    let display = humanize_category(&name);
    let count = topics.len();

    // Render _index.md MOC prose above the auto-list when present.
    let moc_html: Option<String> = {
        let index_path = state.primary_path().join(&name).join("_index.md");
        if index_path.exists() {
            match fs::read_to_string(&index_path).await {
                Ok(text) => {
                    if let Ok(parsed) = crate::render::parse_page(&text) {
                        Some(crate::render::render_html_raw(
                            &parsed.body_md,
                            state.primary_path(),
                            &state.link_roots(),
                        ))
                    } else {
                        None
                    }
                }
                Err(_) => None,
            }
        } else {
            None
        }
    };

    Ok(chrome(
        &format!("{display} — {}", state.site_title),
        html! {
            h1.wiki-cat-page-title { (display) }
            @if let Some(ref moc) = moc_html {
                div.wiki-cat-moc {
                    (PreEscaped(moc))
                }
            }
            @if count == 0 {
                div.wiki-empty-state {
                    p.wiki-empty-title { "This area is being built." }
                    p.wiki-empty-body { "Articles in this category will appear here." }
                }
            } @else {
                div.facet-bar role="group" aria-label="Filter by type" {
                    button.facet-pill.is-active data-filter="all" { "All" }
                    button.facet-pill data-filter="topic" { "Topics" }
                    button.facet-pill data-filter="guide" { "Guides" }
                }
                details.cat-full-index {
                    summary {
                        "All " (count) " article" @if count != 1 { "s" } " in this area, A–Z"
                    }
                    ul.wiki-cat-page-list {
                        @for t in topics {
                            li.wiki-cat-page-item data-kind=(item_type_key(&t.slug)) {
                                a.wiki-cat-page-item-title href={ "/wiki/" (t.slug) } { (t.title) }
                                @if let Some(ref d) = t.last_edited {
                                    span.wiki-cat-page-item-date { (d) }
                                }
                                @if let Some(ref desc) = t.short_description {
                                    p.wiki-cat-page-item-desc { (desc) }
                                }
                            }
                        }
                    }
                }
            }
        },
        &state.site_title,
        maybe_user.as_ref(),
        pending_count,
    ))
}
