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

// ─── Shared footer ─────────────────────────────────────────────────────────

fn shell_footer(brand_instance: &str, view_source_slug: Option<&str>) -> maud::Markup {
    let woodfine = matches!(brand_instance, "projects" | "corporate");
    let copyright_entity = if woodfine { "Woodfine Management Corp." } else { "PointSav Digital Systems" };
    html! {
        footer.footer role="contentinfo" {
            div.cities {
                "Vancouver"
                span.sep { "|" }
                "New York"
            }
            nav.footnav aria-label="Footer navigation" {
                @if woodfine {
                    a href="/page/contact" { "Contact us" }
                    a href="/page/disclaimer" { "Disclaimer" }
                } @else {
                    a href="/page/contact" { "Contact" }
                    a href="/page/disclaimer" { "Disclaimer" }
                }
                a href="/sitemap.xml" { "Sitemap" }
                @if let Some(slug) = view_source_slug {
                    a href={ "/git/" (slug) } { "Source" }
                }
            }
        }
        div.copyright {
            "© 2026 " (copyright_entity) ". All rights reserved. "
            "Content licensed "
            a href="https://creativecommons.org/licenses/by/4.0/" rel="license" { "CC BY 4.0" }
            ". "
            "app-mediakit-knowledge v" (env!("CARGO_PKG_VERSION")) "."
        }
    }
}

// ─── Home-page chrome ───────────────────────────────────────────────────────

const SEARCH_ICON_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="8.5" cy="8.5" r="5"/><line x1="13" y1="13" x2="18" y2="18"/></svg>"#;

const WORDMARK_SVG_WOODFINE: &str = r##"<svg class="logo-svg" aria-label="Woodfine Capital Projects" role="img" viewBox="0 0 144 36" width="320" height="80" version="1.1" xmlns="http://www.w3.org/2000/svg"><g class="institutional-fill" fill="#111827" fill-rule="evenodd"><path d="M 16.069 29.8287 L 16.069 31.0989 L 26.233 31.0989 L 26.233 29.8287 Z"></path><path d="M 118.051 29.8287 L 118.051 31.0989 L 128.216 31.0989 L 128.216 29.8287 Z"></path><path d="M 19.7019 23.1944 L 23.3651 23.1944 L 26.3901 9.48497 L 26.4456 9.48497 L 29.2485 23.1944 L 32.9395 23.1944 L 37.3244 3.15753 L 33.7444 3.15753 L 31.0802 16.2565 L 31.0247 16.2565 L 28.2495 3.15753 L 24.5585 3.15753 L 21.811 16.2565 L 21.7555 16.2565 L 19.0636 3.15753 L 15.5113 3.15753 L 19.7019 23.1944 Z M 38.5452 16.756 C 38.5452 21.6958 41.1539 23.472 44.8727 23.472 C 48.5914 23.472 51.2001 21.6958 51.2001 16.756 L 51.2001 9.59598 C 51.2001 4.65613 48.5914 2.88 44.8727 2.88 C 41.1539 2.88 38.5452 4.65613 38.5452 9.59598 L 38.5452 16.756 Z M 42.375 9.09645 C 42.375 6.87629 43.3463 6.26575 44.8727 6.26575 C 46.399 6.26575 47.3704 6.87629 47.3704 9.09645 L 47.3704 17.2555 C 47.3704 19.4757 46.399 20.0862 44.8727 20.0862 C 43.3463 20.0862 42.375 19.4757 42.375 17.2555 L 42.375 9.09645 Z M 53.4203 16.756 C 53.4203 21.6958 56.029 23.472 59.7477 23.472 C 63.4665 23.472 66.0752 21.6958 66.0752 16.756 L 66.0752 9.59598 C 66.0752 4.65613 63.4665 2.88 59.7477 2.88 C 56.029 2.88 53.4203 4.65613 53.4203 9.59598 L 53.4203 16.756 Z M 57.25 9.09645 C 57.25 6.87629 58.2214 6.26575 59.7477 6.26575 C 61.2741 6.26575 62.2454 6.87629 62.2454 9.09645 L 62.2454 17.2555 C 62.2454 19.4757 61.2741 20.0862 59.7477 20.0862 C 58.2214 20.0862 57.25 19.4757 57.25 17.2555 L 57.25 9.09645 Z M 68.4896 23.1944 L 73.818 23.1944 C 78.2028 23.1944 80.6449 21.3073 80.8114 16.2565 L 80.8114 10.0955 C 80.6449 5.04466 78.2028 3.15753 73.818 3.15753 L 68.4896 3.15753 L 68.4896 23.1944 Z M 72.3194 6.54326 L 73.6514 6.54326 C 76.0381 6.54326 76.9817 7.70885 76.9817 10.5395 L 76.9817 15.8124 C 76.9817 18.8096 75.7606 19.8087 73.6514 19.8087 L 72.3194 19.8087 L 72.3194 6.54326 Z M 86.9169 23.1944 L 86.9169 14.5358 L 91.7457 14.5358 L 91.7457 11.1501 L 86.9169 11.1501 L 86.9169 6.54326 L 93.0778 6.54326 L 93.0778 3.15753 L 83.0871 3.15753 L 83.0871 23.1944 L 86.9169 23.1944 Z M 99.4603 23.1944 L 99.4603 3.15753 L 95.6306 3.15753 L 95.6306 23.1944 L 99.4603 23.1944 Z M 105.594 23.1944 L 105.594 10.262 L 105.649 10.262 L 111.505 23.1944 L 115.168 23.1944 L 115.168 3.15753 L 111.671 3.15753 L 111.671 15.0354 L 111.616 15.0354 L 106.287 3.15753 L 102.097 3.15753 L 102.097 23.1944 L 105.594 23.1944 Z M 128.489 23.1944 L 128.489 19.8087 L 121.551 19.8087 L 121.551 14.5358 L 126.629 14.5358 L 126.629 11.1501 L 121.551 11.1501 L 121.551 6.54326 L 128.211 6.54326 L 128.211 3.15753 L 117.721 3.15753 L 117.721 23.1944 L 128.489 23.1944 Z"></path></g></svg>"##;
const WORDMARK_SVG_POINTSAV: &str = r##"<svg class="logo-svg" role="img" aria-label="PointSav, Inc." viewBox="0 0 144 36" xmlns="http://www.w3.org/2000/svg" fill="#111827"><g transform="translate(-28.8654, -10.2516) scale(1.3911)"><defs id="defs2"></defs><path d="M 36.629983,15.88 V 12.4 h 0.7 q 1.44,0 1.44,1.74 0,0.9 -0.38,1.32 -0.36,0.42 -1.06,0.42 z m -3.72,-6.16 V 24 h 3.72 v -5.44 h 1.3 q 2.3,0 3.48,-1.1 1.2,-1.12 1.2,-3.34 0,-0.94 -0.24,-1.74 -0.24,-0.8 -0.76,-1.38 -0.5,-0.6 -1.28,-0.94 -0.78,-0.34 -1.84,-0.34 z m 14.300013,7.14 q 0,-1.44 0.04,-2.38 0.04,-0.96 0.16,-1.52 0.14,-0.56 0.38,-0.78 0.26,-0.22 0.68,-0.22 0.42,0 0.66,0.22 0.26,0.22 0.38,0.78 0.14,0.56 0.18,1.52 0.04,0.94 0.04,2.38 0,1.44 -0.04,2.4 -0.04,0.94 -0.18,1.5 -0.12,0.56 -0.38,0.78 -0.24,0.22 -0.66,0.22 -0.42,0 -0.68,-0.22 -0.24,-0.22 -0.38,-0.78 -0.12,-0.56 -0.16,-1.5 -0.04,-0.96 -0.04,-2.4 z m -3.84,0 q 0,2 0.26,3.42 0.26,1.4 0.86,2.3 0.6,0.88 1.58,1.26 0.98,0.38 2.4,0.38 1.42,0 2.4,-0.38 0.98,-0.38 1.58,-1.26 0.6,-0.9 0.86,-2.3 0.26,-1.42 0.26,-3.42 0,-2 -0.26,-3.4 -0.26,-1.42 -0.86,-2.3 -0.6,-0.9 -1.58,-1.3 -0.98,-0.42 -2.4,-0.42 -1.42,0 -2.4,0.42 -0.98,0.4 -1.58,1.3 -0.6,0.88 -0.86,2.3 -0.26,1.4 -0.26,3.4 z M 54.769991,9.72 V 24 h 3.72 V 9.72 Z m 5.340005,0 V 24 h 3.48 v -8.82 h 0.04 l 2.48,8.82 h 4.08 V 9.72 h -3.48 v 8.8 h -0.04 l -2.4,-8.8 z m 13.720019,3.16 V 24 h 3.72 V 12.88 h 2.8 V 9.72 h -9.28 v 3.16 z m 12.919998,0.96 h 3.48 q 0,-2.3 -1.1,-3.34 -1.08,-1.06 -3.52,-1.06 -2.36,0 -3.52,1.14 -1.16,1.14 -1.16,3.32 0,1.26 0.42,2.04 0.44,0.78 1.08,1.26 0.66,0.48 1.42,0.78 0.76,0.3 1.4,0.6 0.66,0.28 1.08,0.7 0.44,0.4 0.44,1.1 0,0.58 -0.32,0.98 -0.3,0.4 -0.88,0.4 -0.54,0 -0.88,-0.36 -0.34,-0.38 -0.34,-1.3 v -0.34 h -3.6 v 0.5 q 0,1.12 0.32,1.88 0.32,0.76 0.92,1.24 0.62,0.46 1.5,0.64 0.9,0.2 2.06,0.2 2.46,0 3.76,-1.02 1.3,-1.04 1.3,-3.32 0,-1.3 -0.46,-2.1 -0.44,-0.82 -1.12,-1.32 -0.68,-0.5 -1.46,-0.8 -0.78,-0.32 -1.46,-0.62 -0.68,-0.3 -1.14,-0.7 -0.44,-0.42 -0.44,-1.12 0,-0.48 0.28,-0.86 0.28,-0.4 0.88,-0.4 0.54,0 0.8,0.46 0.26,0.44 0.26,1.08 z m 9.840013,-1.2 -1.02,6.06 h 2.08 l -1.02,-6.06 z m 2.36,-2.92 3.480004,14.28 h -3.960004 l -0.38,-2.5 h -2.96 l -0.38,2.5 h -3.9 l 3.42,-14.28 z m 2.259984,0 3.02,14.28 h 4.8 l 3.08,-14.28 h -3.96 l -1.5,10.76 h -0.04 l -1.5,-10.76 z" id="text1" style="font-weight:900;font-size:20px;font-family:'Helvetica Neue', Helvetica, Arial, sans-serif;text-anchor:middle;fill:#111827;fill-rule:evenodd" aria-label="POINTSAV"></path><path d="m 36.767496,30.4389 v -2.25 h 0.56 q 0.29,0 0.485,0.085 0.2,0.08 0.32,0.235 0.12,0.155 0.17,0.375 0.055,0.215 0.055,0.485 0,0.295 -0.075,0.5 -0.075,0.205 -0.2,0.335 -0.125,0.125 -0.285,0.18 -0.16,0.055 -0.33,0.055 z m -0.785,-2.91 v 3.57 h 1.54 q 0.41,0 0.71,-0.135 0.305,-0.14 0.505,-0.38 0.205,-0.24 0.305,-0.57 0.1,-0.33 0.1,-0.72 0,-0.445 -0.125,-0.775 -0.12,-0.33 -0.34,-0.55 -0.215,-0.22 -0.515,-0.33 -0.295,-0.11 -0.64,-0.11 z m 5.704996,0 v 3.57 h 0.785 v -3.57 z m 6.055,3.165 0.08,0.405 h 0.5 v -1.93 h -1.5 v 0.585 h 0.79 q -0.035,0.375 -0.25,0.575 -0.21,0.195 -0.6,0.195 -0.265,0 -0.45,-0.1 -0.185,-0.105 -0.3,-0.275 -0.115,-0.17 -0.17,-0.38 -0.05,-0.215 -0.05,-0.44 0,-0.235 0.05,-0.455 0.055,-0.22 0.17,-0.39 0.115,-0.175 0.3,-0.275 0.185,-0.105 0.45,-0.105 0.285,0 0.485,0.15 0.2,0.15 0.27,0.45 h 0.75 q -0.03,-0.305 -0.165,-0.54 -0.135,-0.235 -0.345,-0.395 -0.205,-0.16 -0.465,-0.24 -0.255,-0.085 -0.53,-0.085 -0.41,0 -0.74,0.145 -0.325,0.145 -0.55,0.4 -0.225,0.255 -0.345,0.6 -0.12,0.34 -0.12,0.74 0,0.39 0.12,0.73 0.12,0.335 0.345,0.585 0.225,0.25 0.55,0.395 0.33,0.14 0.74,0.14 0.26,0 0.515,-0.105 0.255,-0.11 0.465,-0.38 z m 3.215004,-3.165 v 3.57 h 0.785 v -3.57 z m 4.265001,0.66 v 2.91 h 0.785 v -2.91 h 1.07 v -0.66 h -2.925 v 0.66 z m 4.705005,1.53 0.465,-1.31 h 0.01 l 0.45,1.31 z m 0.075,-2.19 -1.35,3.57 h 0.79 l 0.28,-0.795 h 1.335 l 0.27,0.795 h 0.815 l -1.335,-3.57 z m 4.449997,0 v 3.57 h 2.525 v -0.66 h -1.74 v -2.91 z m 8.890002,2.385 h -0.76 q -0.005,0.33 0.12,0.57 0.125,0.24 0.335,0.395 0.215,0.155 0.49,0.225 0.28,0.075 0.575,0.075 0.365,0 0.64,-0.085 0.28,-0.085 0.465,-0.235 0.19,-0.155 0.285,-0.365 0.095,-0.21 0.095,-0.455 0,-0.3 -0.13,-0.49 -0.125,-0.195 -0.3,-0.31 -0.175,-0.115 -0.355,-0.165 -0.175,-0.055 -0.275,-0.075 -0.335,-0.085 -0.545,-0.14 -0.205,-0.055 -0.325,-0.11 -0.115,-0.055 -0.155,-0.12 -0.04,-0.065 -0.04,-0.17 0,-0.115 0.05,-0.19 0.05,-0.075 0.125,-0.125 0.08,-0.05 0.175,-0.07 0.095,-0.02 0.19,-0.02 0.145,0 0.265,0.025 0.125,0.025 0.22,0.085 0.095,0.06 0.15,0.165 0.06,0.105 0.07,0.265 h 0.76 q 0,-0.31 -0.12,-0.525 -0.115,-0.22 -0.315,-0.36 -0.2,-0.14 -0.46,-0.2 -0.255,-0.065 -0.535,-0.065 -0.24,0 -0.48,0.065 -0.24,0.065 -0.43,0.2 -0.19,0.135 -0.31,0.34 -0.115,0.2 -0.115,0.475 0,0.245 0.09,0.42 0.095,0.17 0.245,0.285 0.15,0.115 0.34,0.19 0.19,0.07 0.39,0.12 0.195,0.055 0.385,0.1 0.19,0.045 0.34,0.105 0.15,0.06 0.24,0.15 0.095,0.09 0.095,0.235 0,0.135 -0.07,0.225 -0.07,0.085 -0.175,0.135 -0.105,0.05 -0.225,0.07 -0.12,0.015 -0.225,0.015 -0.155,0 -0.3,-0.035 -0.145,-0.04 -0.255,-0.115 -0.105,-0.08 -0.17,-0.205 -0.065,-0.125 -0.065,-0.305 z m 5.635002,-0.205 v 1.39 h 0.785 v -1.37 l 1.325,-2.2 h -0.875 l -0.83,1.41 -0.835,-1.41 h -0.88 z m 4.944999,0.205 h -0.76 q -0.005,0.33 0.12,0.57 0.125,0.24 0.335,0.395 0.215,0.155 0.49,0.225 0.28,0.075 0.575,0.075 0.365,0 0.64,-0.085 0.28,-0.085 0.465,-0.235 0.19,-0.155 0.285,-0.365 0.095,-0.21 0.095,-0.455 0,-0.3 -0.13,-0.49 -0.125,-0.195 -0.3,-0.31 -0.175,-0.115 -0.355,-0.165 -0.175,-0.055 -0.275,-0.075 -0.335,-0.085 -0.545,-0.14 -0.205,-0.055 -0.325,-0.11 -0.115,-0.055 -0.155,-0.12 -0.04,-0.065 -0.04,-0.17 0,-0.115 0.05,-0.19 0.05,-0.075 0.125,-0.125 0.08,-0.05 0.175,-0.07 0.095,-0.02 0.19,-0.02 0.145,0 0.265,0.025 0.125,0.025 0.22,0.085 0.095,0.06 0.15,0.165 0.06,0.105 0.07,0.265 h 0.76 q 0,-0.31 -0.12,-0.525 -0.115,-0.22 -0.315,-0.36 -0.2,-0.14 -0.46,-0.2 -0.255,-0.065 -0.535,-0.065 -0.24,0 -0.48,0.065 -0.24,0.065 -0.43,0.2 -0.19,0.135 -0.31,0.34 -0.115,0.2 -0.115,0.475 0,0.245 0.09,0.42 0.095,0.17 0.245,0.285 0.15,0.115 0.34,0.19 0.19,0.07 0.39,0.12 0.195,0.055 0.385,0.1 0.19,0.045 0.34,0.105 0.15,0.06 0.24,0.15 0.095,0.09 0.095,0.235 0,0.135 -0.07,0.225 -0.07,0.085 -0.175,0.135 -0.105,0.05 -0.225,0.07 -0.12,0.015 -0.225,0.015 -0.155,0 -0.3,-0.035 -0.145,-0.04 -0.255,-0.115 -0.105,-0.08 -0.17,-0.205 -0.065,-0.125 -0.065,-0.305 z m 5.499999,-1.725 v 2.91 h 0.785 v -2.91 h 1.07 v -0.66 h -2.925 v 0.66 z m 4.265001,-0.66 v 3.57 h 2.71 v -0.66 h -1.925 v -0.875 h 1.73 v -0.61 h -1.73 v -0.765 h 1.885 v -0.66 z m 5.240001,0 v 3.57 h 0.735 v -2.505 h 0.01 l 0.874997,2.505 h 0.605 l 0.875,-2.53 h 0.01 v 2.53 h 0.735 v -3.57 h -1.105 l -0.79,2.455 h -0.01 l -0.835,-2.455 z m 7.070007,2.385 h -0.76 q -0.005,0.33 0.12,0.57 0.125,0.24 0.335,0.395 0.215,0.155 0.49,0.225 0.28,0.075 0.575,0.075 0.365,0 0.64,-0.085 0.28,-0.085 0.465,-0.235 0.19,-0.155 0.285,-0.365 0.095,-0.21 0.095,-0.455 0,-0.3 -0.13,-0.49 -0.125,-0.195 -0.3,-0.31 -0.175,-0.115 -0.355,-0.165 -0.175,-0.055 -0.275,-0.075 -0.335,-0.085 -0.545,-0.14 -0.205,-0.055 -0.325,-0.11 -0.115,-0.055 -0.155,-0.12 -0.04,-0.065 -0.04,-0.17 0,-0.115 0.05,-0.19 0.05,-0.075 0.125,-0.125 0.08,-0.05 0.175,-0.07 0.095,-0.02 0.19,-0.02 0.145,0 0.265,0.025 0.125,0.025 0.22,0.085 0.095,0.06 0.15,0.165 0.06,0.105 0.07,0.265 h 0.76 q 0,-0.31 -0.12,-0.525 -0.115,-0.22 -0.315,-0.36 -0.2,-0.14 -0.46,-0.2 -0.255,-0.065 -0.535,-0.065 -0.24,0 -0.48,0.065 -0.24,0.065 -0.43,0.2 -0.19,0.135 -0.31,0.34 -0.115,0.2 -0.115,0.475 0,0.245 0.09,0.42 0.095,0.17 0.245,0.285 0.15,0.115 0.34,0.19 0.19,0.07 0.39,0.12 0.195,0.055 0.385,0.1 0.19,0.045 0.34,0.105 0.15,0.06 0.24,0.15 0.095,0.09 0.095,0.235 0,0.135 -0.07,0.225 -0.07,0.085 -0.175,0.135 -0.105,0.05 -0.225,0.07 -0.12,0.015 -0.225,0.015 -0.155,0 -0.3,-0.035 -0.145,-0.04 -0.255,-0.115 -0.105,-0.08 -0.17,-0.205 -0.065,-0.125 -0.065,-0.305 z" id="text2" style="font-weight:700;font-size:5px;font-family:'Helvetica Neue', Helvetica, Arial, sans-serif;letter-spacing:2;text-anchor:middle;fill:#111827;fill-rule:evenodd" aria-label="DIGITAL SYSTEMS"></path></g></svg>"##;

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
    pending_count: i64,
) -> Markup {
    let woodfine_theme = matches!(brand_theme, Some("woodfine") | Some("woodfine-projects"));
    let _title = home_fm.title.as_deref().unwrap_or(site_title);
    let auth_attr = if user.is_some() { "user" } else { "anon" };
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
                a.skip-to-content href="#mp-main" { "Skip to content" }
                header.topnav {
                    nav.left {
                        @if woodfine_theme {
                            a href="/page/disclaimer" { "Disclaimer" }
                            a href="/page/contact" { "Contact us" }
                        } @else {
                            a href="/page/disclaimer" { "Disclaimer" }
                        }
                    }
                    a.wordmark href="/" aria-label=(site_title) {
                        @if woodfine_theme {
                            (PreEscaped(WORDMARK_SVG_WOODFINE))
                        } @else {
                            (PreEscaped(WORDMARK_SVG_POINTSAV))
                        }
                    }
                    div.right-cluster {
                        nav.right {
                            @if woodfine_theme {
                                a.external href="https://corporate.woodfinegroup.com" target="_blank" rel="noopener" { "Corporate" }
                                a.external href="https://projects.woodfinegroup.com" target="_blank" rel="noopener" { "Projects" }
                                a.external href="https://newsroom.woodfinegroup.com" target="_blank" rel="noopener" { "Newsroom" }
                            } @else {
                                a.external href="https://software.pointsav.com" target="_blank" rel="noopener" { "Monorepo" }
                                a.external href="https://design.pointsav.com" target="_blank" rel="noopener" { "Design System" }
                            }
                        }
                        (auth_nav_widget(user, pending_count))
                        a.lang-toggle href=(match locale { Locale::En => "/es/", Locale::Es => "/?noredirect=1" }) {
                            (s.lang_toggle_label)
                        }
                        button.search-toggle type="button" aria-label="Search" aria-expanded="false"
                            aria-controls="topnav-search-panel" {
                            (PreEscaped(SEARCH_ICON_SVG))
                        }
                        @if woodfine_theme {
                            a.header-cta href="/page/contact" { "Enquire" }
                        }
                    }
                }
                div.topnav-search-panel #topnav-search-panel aria-hidden="true" {
                    form.topnav-search action="/search" method="get" role="search" {
                        input #header-search-q
                            type="search"
                            name="q"
                            placeholder="Search…"
                            autocomplete="off"
                            aria-label="Search this wiki"
                            spellcheck="false";
                        button.topnav-search-btn type="submit" aria-label="Search" { "→" }
                    }
                    div.ac-dropdown #search-autocomplete-dropdown {}
                }
                main.site-main #mp-main {

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
                (shell_footer(brand_instance, None))
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
    lang_toggle_label: &'static str,
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
            lang_toggle_label: "ES",
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
            lang_toggle_label: "EN",
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
