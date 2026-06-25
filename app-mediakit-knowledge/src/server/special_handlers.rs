
// ─── Wikipedia-parity special page handlers ────────────────────────────────

/// `GET /special/whatlinkshere/{slug}` — lists all articles that link to the
/// given slug, equivalent to Wikipedia's Special:WhatLinksHere.
async fn what_links_here(
    Path(slug): Path<String>,
    State(state): State<Arc<AppState>>,
    CurrentUser(maybe_user): CurrentUser,
) -> Result<Markup, WikiError> {
    let pending_count = pending_count_for(&state, maybe_user.as_ref()).await;

    // Use the redb link graph for exact wikilink backlinks (Step 4.4).
    let backlink_slugs = state.links.backlinks(&slug).unwrap_or_default();

    // Look up the actual article title from frontmatter for each backlink.
    let mut backlinks: Vec<(String, String)> = Vec::new();
    for s in backlink_slugs {
        let path = state.primary_path().join(format!("{s}.md"));
        let title = if let Ok(text) = fs::read_to_string(&path).await {
            if let Ok((fm, _)) = crate::walker::parse_frontmatter(&text) {
                fm.title.unwrap_or_else(|| {
                    s.rsplit('/').next().unwrap_or(&s).replace('-', " ")
                })
            } else {
                s.rsplit('/').next().unwrap_or(&s).replace('-', " ")
            }
        } else {
            s.rsplit('/').next().unwrap_or(&s).replace('-', " ")
        };
        backlinks.push((s, title));
    }

    let page_title = format!("What links here: {slug}");
    Ok(chrome(
        &format!("{} — {}", page_title, state.site_title),
        html! {
            h1 { "What links here" }
            p.wiki-special-intro { "Articles that link to " em { (slug) } "." }
            @if backlinks.is_empty() {
                p { "No other articles currently link to this page." }
            } @else {
                p { (backlinks.len()) " article" @if backlinks.len() != 1 { "s" } " link here:" }
                ul.wiki-backlinks-list {
                    @for (s, title) in &backlinks {
                        li {
                            a href={ "/wiki/" (s) } { (title) }
                            span.search-hit-slug { " — " (s) }
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

/// `GET /special/pageinfo/{slug}` — shows metadata for an article (title,
/// category, status, last edited, word count), equivalent to Wikipedia's
/// Special:PageInfo.
async fn page_info(
    Path(slug): Path<String>,
    State(state): State<Arc<AppState>>,
    CurrentUser(maybe_user): CurrentUser,
) -> Result<Markup, WikiError> {
    let pending_count = pending_count_for(&state, maybe_user.as_ref()).await;

    // Load the article file to extract frontmatter.
    let md_path = state.primary_path().join(format!("{slug}.md"));
    let (title, category, status, last_edited, word_count) = if md_path.exists() {
        let raw = tokio::fs::read_to_string(&md_path)
            .await
            .unwrap_or_default();
        let parsed =
            crate::render::parse_page(&raw).unwrap_or_else(|_| crate::render::ParsedPage {
                frontmatter: crate::render::Frontmatter::default(),
                body_md: raw.clone(),
            });
        let fm = parsed.frontmatter;
        let title = fm.title.unwrap_or_else(|| slug.clone());
        let category = fm.category.unwrap_or_else(|| "—".to_string());
        let status = fm.status.unwrap_or_else(|| "stable".to_string());
        let last_edited = fm.last_edited.unwrap_or_else(|| "—".to_string());
        let word_count = parsed.body_md.split_whitespace().count();
        (title, category, status, last_edited, word_count)
    } else {
        (
            slug.clone(),
            "—".to_string(),
            "—".to_string(),
            "—".to_string(),
            0,
        )
    };

    Ok(chrome(
        &format!("Page information: {title} — {}", state.site_title),
        html! {
            h1 { "Page information: " em { (title) } }
            table.wiki-info-table {
                tr { th { "Field" } th { "Value" } }
                tr { td { "Slug" }        td { code { (slug) } } }
                tr { td { "Category" }    td { (category) } }
                tr { td { "Status" }      td { (status) } }
                tr { td { "Last edited" } td { (last_edited) } }
                tr { td { "Word count" }  td { (word_count) } }
            }
            p { a href={ "/wiki/" (slug) } { "← Back to article" } }
        },
        &state.site_title,
        maybe_user.as_ref(),
        pending_count,
    ))
}

/// `GET /special/cite/{slug}` — renders citation formats for the article
/// (Wikipedia/APA/MLA), equivalent to Wikipedia's "Cite this page" tool.
async fn cite_page(
    Path(slug): Path<String>,
    State(state): State<Arc<AppState>>,
    CurrentUser(maybe_user): CurrentUser,
) -> Result<Markup, WikiError> {
    let pending_count = pending_count_for(&state, maybe_user.as_ref()).await;

    let md_path = state.primary_path().join(format!("{slug}.md"));
    let (title, last_edited) = if md_path.exists() {
        let raw = tokio::fs::read_to_string(&md_path)
            .await
            .unwrap_or_default();
        let parsed =
            crate::render::parse_page(&raw).unwrap_or_else(|_| crate::render::ParsedPage {
                frontmatter: crate::render::Frontmatter::default(),
                body_md: raw.clone(),
            });
        let fm = parsed.frontmatter;
        (
            fm.title.unwrap_or_else(|| slug.clone()),
            fm.last_edited.unwrap_or_else(|| "n.d.".to_string()),
        )
    } else {
        (slug.clone(), "n.d.".to_string())
    };

    let url = format!("https://documentation.pointsav.com/wiki/{slug}");
    let site = &state.site_title;
    let apa = format!("PointSav Digital Systems. ({last_edited}). {title}. {site}. {url}");
    let mla = format!("PointSav Digital Systems. \"{title}.\" {site}, {last_edited}, {url}.");
    let wiki =
        format!("{{{{cite web|url={url}|title={title}|website={site}|date={last_edited}}}}}");

    Ok(chrome(
        &format!("Cite: {title} — {site}"),
        html! {
            h1 { "Cite this page: " em { (title) } }
            p { "Use one of the formats below to cite this article." }
            h2 { "APA" }
            pre.wiki-cite-block { (apa) }
            h2 { "MLA" }
            pre.wiki-cite-block { (mla) }
            h2 { "Wikitext" }
            pre.wiki-cite-block { (wiki) }
            p { a href={ "/wiki/" (slug) } { "← Back to article" } }
        },
        &state.site_title,
        maybe_user.as_ref(),
        pending_count,
    ))
}

// ─── Phase 3 Step 3.4 handlers ─────────────────────────────────────────────

/// `GET /sitemap.xml` — sitemaps.org standard XML sitemap.
///
/// Walks `content_dir` recursively, emits one `<url>` per TOPIC (excluding
/// `*.es.md` bilingual siblings). Content-Type: `application/xml; charset=utf-8`.
async fn sitemap_xml(State(state): State<Arc<AppState>>) -> Result<Response, WikiError> {
    let topic_files = collect_all_topic_files(
        state.primary_path(),
        &state.guide_dirs_arr(),
    )
    .await?;
    let mut slugs: Vec<String> = topic_files.into_iter().map(|tf| tf.slug).collect();
    slugs.sort();

    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
    );
    let loc_base = state.canonical_url.as_deref().unwrap_or_default();
    for slug in &slugs {
        xml.push_str(&format!("  <url><loc>{loc_base}/wiki/{slug}</loc></url>\n"));
    }
    xml.push_str("</urlset>\n");

    let mut resp = xml.into_response();
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("application/xml; charset=utf-8"),
    );
    Ok(resp)
}

/// `GET /robots.txt` — static crawl-permission declaration.
///
/// Allows all crawlers and declares the sitemap location.
/// Content-Type: `text/plain; charset=utf-8`.
async fn robots_txt() -> Response {
    let body = "User-agent: *\nAllow: /\nSitemap: /sitemap.xml\n";
    let mut resp = body.into_response();
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    resp
}

/// `GET /llms.txt` — emerging LLM-readable site manifest convention.
///
/// Per the llmstxt.org convention (informal, 2025–2026). Lists all TOPICs
/// with a one-line snippet, and points crawlers at the structured data
/// surfaces (JSON-LD, Atom, JSON Feed, sitemap). Content-Type:
/// `text/markdown; charset=utf-8`.
async fn llms_txt(State(state): State<Arc<AppState>>) -> Result<Response, WikiError> {
    let topic_files = collect_all_topic_files(
        state.primary_path(),
        &state.guide_dirs_arr(),
    )
    .await?;
    let mut tf_list: Vec<(String, PathBuf)> = topic_files
        .into_iter()
        .map(|tf| (tf.slug, tf.path))
        .collect();
    tf_list.sort_by(|a, b| a.0.cmp(&b.0));

    // Read each TOPIC to extract a one-line title + snippet directly from the
    // parsed body — avoids a second directory traversal compared to calling
    // `collect_recent_items`.
    let mut topic_lines: Vec<String> = Vec::new();
    for (slug, path) in &tf_list {
        let text = match fs::read_to_string(path).await {
            Ok(t) => t,
            Err(_) => continue,
        };
        let slug_str = slug.as_str();
        let parsed = match crate::render::parse_page(&text) {
            Ok(p) => p,
            Err(_) => continue,
        };
        let title = parsed.frontmatter.title.unwrap_or_else(|| slug.clone());
        let slug = slug_str;

        // Build a ~120-character snippet from the first non-heading body line.
        let body_snippet = llms_txt_snippet(&parsed.body_md, 120);

        topic_lines.push(format!("- [{title}](/wiki/{slug}): {body_snippet}"));
    }

    let topics_section = topic_lines.join("\n");

    let body = format!(
        "# {site_title}\n\
         \n\
         > Single-binary Markdown wiki engine; flat-file source-of-truth, \
         AI-optional, Wikipedia-shaped UX. Substrate substitution per \
         DOCTRINE claim #29.\n\
         \n\
         ## TOPICs\n\
         \n\
         {topics_section}\n\
         \n\
         ## Structured data\n\
         \n\
         - JSON-LD: every TOPIC `<head>` carries schema.org `TechArticle` / `DefinedTerm`\n\
         - Atom feed: `/feed.atom`\n\
         - JSON Feed: `/feed.json`\n\
         - Sitemap: `/sitemap.xml`\n",
        site_title = state.site_title,
        topics_section = topics_section,
    );

    let mut resp = body.into_response();
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("text/markdown; charset=utf-8"),
    );
    Ok(resp)
}

/// Extract a plain-text snippet for llms.txt, capped at `max_chars`.
/// Skips heading, blank, and HR lines; strips crude Markdown punctuation.
fn llms_txt_snippet(body_md: &str, max_chars: usize) -> String {
    let first = body_md
        .lines()
        .find(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with('#') && !t.starts_with("---")
        })
        .unwrap_or("");
    let clean: String = first
        .trim_start_matches(['-', '*', '+', '>', ' '])
        .chars()
        .filter(|&c| c != '`' && c != '*' && c != '_')
        .collect();
    let clean = clean.trim();
    if clean.len() <= max_chars {
        clean.to_string()
    } else {
        let safe_end = (0..=max_chars).rev().find(|&i| clean.is_char_boundary(i)).unwrap_or(0);
        let boundary = clean[..safe_end].rfind(' ').unwrap_or(safe_end);
        format!("{}…", &clean[..boundary])
    }
}

/// `GET /git/{slug}.md` — raw Markdown source for `git clone`-style ingestion.
///
/// Validates the slug via `validate_slug`, reads
/// `<content_dir>/<slug>.md` from disk, and returns the raw bytes with
/// Content-Type `text/markdown; charset=utf-8`. Phase 4 upgrades this to a
/// full read-only Git remote.
///
/// Axum 0.8 captures the `{slug}` parameter **without** the `.md` suffix
/// when the route pattern is `/git/{slug}.md` — the literal `.md` in the
/// pattern is consumed by the router and not included in the extract.
async fn git_markdown(
    State(state): State<Arc<AppState>>,
    Path(raw): Path<String>,
) -> Result<Response, WikiError> {
    // Accept both `/git/topic-foo` and `/git/topic-foo.md` — strip an
    // optional `.md` suffix before slug validation. The `.md` extension
    // surfaces in the URL for consumer convenience (looks like a static
    // file under `git clone` mirror semantics) but is not part of the slug.
    let slug = raw.strip_suffix(".md").unwrap_or(&raw).to_string();

    // Slug validation rejects path traversal, uppercase, and other illegal forms.
    validate_slug(&slug)?;

    let path = state.primary_path().join(format!("{slug}.md"));
    let bytes = match fs::read(&path).await {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(WikiError::NotFound(slug));
        }
        Err(e) => return Err(e.into()),
    };

    let mut resp = bytes.into_response();
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("text/markdown; charset=utf-8"),
    );
    Ok(resp)
}

// ── Sprint C: Special pages ────────────────────────────────────────────────

/// C1: `GET /special/recent-changes` — cross-repo git log, newest 50 changes.
async fn recent_changes_page(
    State(state): State<Arc<AppState>>,
    CurrentUser(maybe_user): CurrentUser,
) -> Result<Markup, WikiError> {
    let pending_count = pending_count_for(&state, maybe_user.as_ref()).await;

    let entries = {
        let repo = gix::open(state.primary_path())
            .map_err(|e| WikiError::WriteFailed(format!("gix open: {e}")))?;
        let head = match repo.head() {
            Ok(h) => h,
            Err(_) => {
                return Ok(chrome(
                    &format!("Recent changes — {}", state.site_title),
                    html! { h1 { "Recent changes" } p { "No git history yet." } },
                    &state.site_title,
                    maybe_user.as_ref(),
                    pending_count,
                ));
            }
        };
        let id = match head.id() {
            Some(id) => id,
            None => {
                return Ok(chrome(
                    &format!("Recent changes — {}", state.site_title),
                    html! { h1 { "Recent changes" } p { "Empty repository." } },
                    &state.site_title,
                    maybe_user.as_ref(),
                    pending_count,
                ));
            }
        };
        let mut out = Vec::new();
        let ancestors = id
            .ancestors()
            .all()
            .map_err(|e| WikiError::WriteFailed(format!("gix ancestors: {e}")))?;
        for item in ancestors.take(50) {
            let item = match item {
                Ok(i) => i,
                Err(_) => continue,
            };
            let commit = match item.object() {
                Ok(c) => c,
                Err(_) => continue,
            };
            let author = match commit.author() {
                Ok(a) => a,
                Err(_) => continue,
            };
            let message = match commit.message() {
                Ok(m) => m,
                Err(_) => continue,
            };
            let time = match commit.time() {
                Ok(t) => t,
                Err(_) => continue,
            };
            let ts = {
                use chrono::{TimeZone, Utc};
                let secs = time.seconds;
                Utc.timestamp_opt(secs, 0)
                    .single()
                    .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                    .unwrap_or_else(|| secs.to_string())
            };
            // Extract article slug from commit message if it references one content file.
            let msg = message.summary().to_string();
            out.push((item.id().to_string(), author.name.to_string(), ts, msg));
        }
        out
    };

    Ok(chrome(
        &format!("Recent changes — {}", state.site_title),
        html! {
            h1 { "Recent changes" }
            p.wiki-special-intro { "Last 50 edits across all articles." }
            @if entries.is_empty() {
                p { em { "No changes recorded yet." } }
            } @else {
                table.wiki-special-table {
                    thead {
                        tr {
                            th { "Date" }
                            th { "Author" }
                            th { "Summary" }
                        }
                    }
                    tbody {
                        @for (sha, author, date, msg) in &entries {
                            tr {
                                td.rc-date { (date) }
                                td.rc-author { (author) }
                                td.rc-summary {
                                    code.rc-sha { (&sha[..7.min(sha.len())]) }
                                    " "
                                    (msg)
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

/// C2: `GET /special/all-pages` — alphabetical directory grouped by first letter.
async fn all_pages_page(
    State(state): State<Arc<AppState>>,
    CurrentUser(maybe_user): CurrentUser,
) -> Result<Markup, WikiError> {
    let pending_count = pending_count_for(&state, maybe_user.as_ref()).await;

    let topic_files = collect_all_topic_files(
        state.primary_path(),
        &state.guide_dirs_arr(),
    )
    .await?;

    // Collect (title, slug) pairs, sorted by title.
    let mut pages: Vec<(String, String)> = Vec::new();
    for tf in &topic_files {
        let title = if let Ok(text) = fs::read_to_string(&tf.path).await {
            if let Ok(parsed) = crate::render::parse_page(&text) {
                parsed.frontmatter.title.unwrap_or_else(|| tf.slug.clone())
            } else {
                tf.slug.clone()
            }
        } else {
            tf.slug.clone()
        };
        pages.push((title, tf.slug.clone()));
    }
    pages.sort_by_key(|a| a.0.to_lowercase());

    // Group by first letter.
    let mut groups: BTreeMap<char, Vec<(String, String)>> = BTreeMap::new();
    for (title, slug) in pages {
        let ch = title
            .chars()
            .next()
            .unwrap_or('#')
            .to_uppercase()
            .next()
            .unwrap_or('#');
        let key = if ch.is_ascii_alphabetic() { ch } else { '#' };
        groups.entry(key).or_default().push((title, slug));
    }

    Ok(chrome(
        &format!("All pages — {}", state.site_title),
        html! {
            h1 { "All pages" }
            p.wiki-special-intro { (topic_files.len()) " articles total." }
            // Jump links
            nav.wiki-allpages-jump {
                @for ch in groups.keys() {
                    a href={ "#ap-" (ch) } { (ch) }
                    " "
                }
            }
            @for (ch, entries) in &groups {
                h2 id={ "ap-" (ch) } { (ch) }
                ul.wiki-allpages-list {
                    @for (title, slug) in entries {
                        li { a href={ "/wiki/" (slug) } { (title) } }
                    }
                }
            }
        },
        &state.site_title,
        maybe_user.as_ref(),
        pending_count,
    ))
}

/// `GET /special/categories` — index of all categories with article counts,
/// mirroring Wikipedia's Special:Categories.
async fn categories_index_page(
    State(state): State<Arc<AppState>>,
    CurrentUser(maybe_user): CurrentUser,
) -> Result<Markup, WikiError> {
    let pending_count = pending_count_for(&state, maybe_user.as_ref()).await;

    let topic_files = collect_all_topic_files(
        state.primary_path(),
        &state.guide_dirs_arr(),
    )
    .await?;

    // Collect category → count pairs.
    let mut cat_counts: BTreeMap<String, usize> = BTreeMap::new();
    for tf in &topic_files {
        if let Ok(text) = fs::read_to_string(&tf.path).await {
            if let Ok(parsed) = crate::render::parse_page(&text) {
                // categories[] list takes precedence over singular category:
                if let Some(cats) = parsed.frontmatter.categories {
                    for cat in cats {
                        *cat_counts.entry(cat).or_insert(0) += 1;
                    }
                } else if let Some(cat) = parsed.frontmatter.category {
                    if cat != "root" {
                        *cat_counts.entry(cat).or_insert(0) += 1;
                    }
                }
            }
        }
    }

    // Group by first letter of the humanized name.
    let mut groups: BTreeMap<char, Vec<(String, String, usize)>> = BTreeMap::new();
    for (cat_slug, count) in &cat_counts {
        let display = humanize_category(cat_slug);
        let ch = display
            .chars()
            .next()
            .unwrap_or('#')
            .to_uppercase()
            .next()
            .unwrap_or('#');
        let key = if ch.is_ascii_alphabetic() { ch } else { '#' };
        groups
            .entry(key)
            .or_default()
            .push((display, cat_slug.clone(), *count));
    }
    for entries in groups.values_mut() {
        entries.sort_by_key(|a| a.0.to_lowercase());
    }

    Ok(chrome(
        &format!("Categories — {}", state.site_title),
        html! {
            h1 { "Categories" }
            p.wiki-special-intro { (cat_counts.len()) " categories across " (topic_files.len()) " articles." }
            nav.wiki-allpages-jump {
                @for ch in groups.keys() {
                    a href={ "#cat-" (ch) } { (ch) }
                    " "
                }
            }
            @for (ch, entries) in &groups {
                h2 id={ "cat-" (ch) } { (ch) }
                ul.wiki-allpages-list {
                    @for (display, slug, count) in entries {
                        li {
                            a href={ "/category/" (slug) } { (display) }
                            " "
                            span.wiki-cat-count { "(" (count) (if *count == 1 { " article" } else { " articles" }) ")" }
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

/// C3: `GET /special/statistics` — article count, categories, redlink count, most recent edit.
async fn statistics_page(
    State(state): State<Arc<AppState>>,
    CurrentUser(maybe_user): CurrentUser,
) -> Result<Markup, WikiError> {
    let pending_count = pending_count_for(&state, maybe_user.as_ref()).await;
    let re_redlink = Regex::new(r#"class="wiki-redlink""#).expect("static regex");

    let topic_files = collect_all_topic_files(
        state.primary_path(),
        &state.guide_dirs_arr(),
    )
    .await?;

    let article_count = topic_files.len();
    let mut category_set: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut redlink_count: usize = 0;
    let mut most_recent: Option<String> = None;

    for tf in &topic_files {
        if let Ok(text) = fs::read_to_string(&tf.path).await {
            if let Ok(parsed) = crate::render::parse_page(&text) {
                if let Some(cat) = parsed.frontmatter.category {
                    category_set.insert(cat);
                }
                if let Some(ref le) = parsed.frontmatter.last_edited {
                    let is_newer = most_recent.as_ref().map_or(true, |mr| le > mr);
                    if is_newer {
                        most_recent = Some(le.clone());
                    }
                }
                let html =
                    crate::render::render_html_raw(&text, state.primary_path(), &state.link_roots());
                redlink_count += re_redlink.find_iter(&html).count();
            }
        }
    }

    Ok(chrome(
        &format!("Statistics — {}", state.site_title),
        html! {
            h1 { "Statistics" }
            table.wiki-special-table {
                tbody {
                    tr { th { "Articles" } td { (article_count) } }
                    tr { th { "Categories" } td { (category_set.len()) } }
                    tr { th { "Redlinks (missing articles)" } td { (redlink_count) } }
                    tr { th { "Most recent edit" } td {
                        @if let Some(ref d) = most_recent { (d) }
                        @else { em { "unknown" } }
                    } }
                }
            }
        },
        &state.site_title,
        maybe_user.as_ref(),
        pending_count,
    ))
}

// ── Sprint C4: Talk namespace ──────────────────────────────────────────────

fn talk_file_path(content_dir: &FsPath, slug: &str) -> PathBuf {
    content_dir.join("talk").join(format!("{slug}.md"))
}

/// C4: `GET /talk/{*slug}` — serve talk page or empty stub.
async fn talk_page(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
    CurrentUser(maybe_user): CurrentUser,
) -> Result<Markup, WikiError> {
    if slug.contains("..") || slug.is_empty() {
        return Err(WikiError::NotFound(slug));
    }
    let pending_count = pending_count_for(&state, maybe_user.as_ref()).await;
    let talk_path = talk_file_path(state.primary_path(), &slug);
    let talk_md = if talk_path.is_file() {
        fs::read_to_string(&talk_path).await.unwrap_or_default()
    } else {
        String::new()
    };
    let body_html = if talk_md.is_empty() {
        String::new()
    } else {
        crate::render::render_html(&talk_md, state.primary_path(), &state.link_roots())
    };

    let article_url = format!("/wiki/{slug}");
    Ok(chrome(
        &format!("Talk: {slug} — {}", state.site_title),
        html! {
            div.wiki-title-row {
                nav.wiki-page-tabs aria-label="Page tabs" {
                    a.wiki-tab href=(article_url) { "Article" }
                    a.wiki-tab.wiki-tab-active aria-current="page" href={ "/talk/" (slug) } { "Talk" }
                }
                div.wiki-title-block {
                    h1.page-title { "Talk: " (slug) }
                    p.wiki-tagline { "From " (state.site_title.trim_end_matches(" Wiki")) }
                }
                nav.wiki-action-tabs aria-label="Page actions" {
                    a.wiki-tab.wiki-tab-active aria-current="page" href={ "/talk/" (slug) } { "Discussion" }
                }
            }
            @if body_html.is_empty() {
                p.wiki-talk-empty {
                    em { "No discussion yet. Add a section below to start the conversation." }
                }
            } @else {
                div.wiki-article { div.page-body { (PreEscaped(body_html)) } }
            }
            @if maybe_user.is_some() {
                section.wiki-talk-post {
                    h2 { "Add a new section" }
                    form method="post" action={ "/talk/" (slug) } {
                        div.talk-form-row {
                            label for="talk-section-title" { "Section title" }
                            input #talk-section-title name="section_title" type="text"
                                placeholder="Section heading" required;
                        }
                        div.talk-form-row {
                            label for="talk-body" { "Comment" }
                            textarea #talk-body name="body" rows="6"
                                placeholder="Write your comment here…" required {}
                        }
                        button.wiki-btn type="submit" { "Add section" }
                    }
                }
            }
        },
        &state.site_title,
        maybe_user.as_ref(),
        pending_count,
    ))
}

#[derive(Deserialize)]
struct TalkPostForm {
    section_title: String,
    body: String,
}

/// C4: `POST /talk/{*slug}` — append a new section to the talk page.
async fn talk_post(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
    CurrentUser(maybe_user): CurrentUser,
    axum::Form(form): axum::Form<TalkPostForm>,
) -> Result<Response, WikiError> {
    if slug.contains("..") || slug.is_empty() {
        return Err(WikiError::NotFound(slug));
    }
    let user = maybe_user.ok_or_else(|| WikiError::NotFound("unauthenticated".into()))?;

    let section_title = form.section_title.trim().to_string();
    let body_text = form.body.trim().to_string();
    if section_title.is_empty() || body_text.is_empty() {
        return Err(WikiError::SlugInvalid(
            "section_title and body are required".into(),
        ));
    }

    let talk_dir = state.primary_path().join("talk");
    tokio::fs::create_dir_all(&talk_dir).await?;
    let talk_path = talk_file_path(state.primary_path(), &slug);

    let existing = if talk_path.is_file() {
        tokio::fs::read_to_string(&talk_path)
            .await
            .unwrap_or_default()
    } else {
        String::new()
    };

    use chrono::Utc;
    let now = Utc::now().format("%Y-%m-%d %H:%M UTC").to_string();
    let new_section = format!(
        "\n\n## {}\n\n*{} — {}*\n\n{}\n",
        section_title, user.username, now, body_text
    );
    let updated = format!("{}{}", existing.trim_end(), new_section);
    tokio::fs::write(&talk_path, updated.as_bytes()).await?;

    Ok(Response::builder()
        .status(StatusCode::FOUND)
        .header(header::LOCATION, format!("/talk/{slug}"))
        .body(axum::body::Body::empty())
        .unwrap())
}

async fn history_page(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
    Query(hp): Query<HistoryPageParams>,
    CurrentUser(maybe_user): CurrentUser,
) -> Result<Markup, WikiError> {
    validate_slug(&slug)?;
    let path = state.primary_path().join(format!("{slug}.md"));
    if !path.is_file() {
        return Err(WikiError::NotFound(slug));
    }

    const PER_PAGE: usize = 25;
    let page = hp.page.unwrap_or(1).max(1) as usize;
    let all_history = crate::history::topic_history(state.primary_path(), &slug, 500)?;
    let total = all_history.len();
    let start = (page - 1) * PER_PAGE;
    let history: Vec<_> = all_history.into_iter().skip(start).take(PER_PAGE).collect();
    let has_older = start + history.len() < total;
    let has_newer = page > 1;

    let body = html! {
        h1 { "History: " (slug) }
        @if total == 0 {
            p { "No revision history yet." }
        } @else {
            table.history-table {
                thead {
                    tr.history-thead-row {
                        th.history-th { "SHA" }
                        th.history-th { "Author" }
                        th.history-th { "Date" }
                        th.history-th { "Commit" }
                        th.history-th { "Edit summary" }
                    }
                }
                tbody {
                    @for entry in &history {
                        tr.history-body-row {
                            td.history-td-sha {
                                a href=(format!("/diff/{}?b={}&a={}~", slug, entry.sha, entry.sha)) {
                                    @if entry.sha.len() >= 7 {
                                        (entry.sha[..7].to_string())
                                    } @else {
                                        (entry.sha)
                                    }
                                }
                            }
                            td.history-td { (entry.author) }
                            td.history-td-date { (entry.timestamp_iso) }
                            td.history-td { (entry.message) }
                            td.history-td-summary {
                                @if !entry.edit_summary.is_empty() {
                                    (entry.edit_summary)
                                }
                            }
                        }
                    }
                }
            }
            nav.history-pagination {
                @if has_newer {
                    a.history-page-link href={ "/history/" (slug) "?page=" (page - 1) } { "← newer" }
                }
                @if has_older {
                    a.history-page-link href={ "/history/" (slug) "?page=" (page + 1) } { "older →" }
                }
            }
        }
    };

    let pending_count = pending_count_for(&state, maybe_user.as_ref()).await;
    Ok(chrome(
        &format!("History: {}", slug),
        body,
        &state.site_title,
        maybe_user.as_ref(),
        pending_count,
    ))
}

async fn blame_page(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
    CurrentUser(maybe_user): CurrentUser,
) -> Result<Markup, WikiError> {
    validate_slug(&slug)?;
    let path = state.primary_path().join(format!("{slug}.md"));
    if !path.is_file() {
        return Err(WikiError::NotFound(slug));
    }
    let blame = crate::history::topic_blame(state.primary_path(), &slug)?;

    let body = html! {
        h1 { "Blame: " (slug) }
        div.blame-container {
            pre.blame-pre {
                @for line in blame {
                    div.blame-line {
                        span.blame-meta {
                            @if line.sha.len() >= 7 {
                                (line.sha[..7].to_string())
                            } @else {
                                (line.sha)
                            }
                            " " (line.author)
                        }
                        span.blame-text { (line.line_text) }
                    }
                }
            }
        }
    };

    let pending_count = pending_count_for(&state, maybe_user.as_ref()).await;
    Ok(chrome(
        &format!("Blame: {}", slug),
        body,
        &state.site_title,
        maybe_user.as_ref(),
        pending_count,
    ))
}

#[derive(Deserialize)]
struct HistoryPageParams {
    page: Option<u32>,
}

#[derive(Deserialize)]
struct DiffQueryParams {
    a: Option<String>,
    b: Option<String>,
}

/// D1: Two-column word-level diff — Wikipedia-style red/green inline table.
async fn diff_page(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
    Query(query): Query<DiffQueryParams>,
    CurrentUser(maybe_user): CurrentUser,
) -> Result<Markup, WikiError> {
    validate_slug(&slug)?;
    let a_sha = query.a.unwrap_or_default();
    let b_sha = query.b.unwrap_or_else(|| "HEAD".to_string());

    // Retrieve file content at both revisions (blocking — run on threadpool).
    let content_dir = state.primary_path().to_path_buf();
    let slug2 = slug.clone();
    let a2 = a_sha.clone();
    let b2 = b_sha.clone();
    let (old_text, new_text) = tokio::task::spawn_blocking(move || {
        let old = crate::history::get_file_at_rev(&content_dir, &slug2, &a2).unwrap_or_default();
        let new = crate::history::get_file_at_rev(&content_dir, &slug2, &b2).unwrap_or_default();
        (old, new)
    })
    .await
    .map_err(|e| WikiError::WriteFailed(format!("diff spawn: {e}")))?;

    // Build two-column rows: (left_html, right_html, row_class)
    let line_diff = similar::TextDiff::from_lines(&old_text, &new_text);
    let mut rows: Vec<(String, String, &'static str)> = Vec::new();

    // Collect paired del/ins groups for inline word-level diff.
    let mut pending_del: Vec<String> = Vec::new();
    let mut pending_ins: Vec<String> = Vec::new();

    fn flush_pending(
        del: &mut Vec<String>,
        ins: &mut Vec<String>,
        rows: &mut Vec<(String, String, &'static str)>,
    ) {
        let max = del.len().max(ins.len());
        for i in 0..max {
            let old_line = del.get(i).map(|s| s.as_str()).unwrap_or("");
            let new_line = ins.get(i).map(|s| s.as_str()).unwrap_or("");
            let (left_html, right_html) = if !old_line.is_empty() && !new_line.is_empty() {
                word_diff_pair(old_line, new_line)
            } else {
                (html_escape(old_line), html_escape(new_line))
            };
            let cls = if old_line.is_empty() {
                "diff-row-ins"
            } else if new_line.is_empty() {
                "diff-row-del"
            } else {
                "diff-row-chg"
            };
            rows.push((left_html, right_html, cls));
        }
        del.clear();
        ins.clear();
    }

    for change in line_diff.iter_all_changes() {
        match change.tag() {
            similar::ChangeTag::Equal => {
                flush_pending(&mut pending_del, &mut pending_ins, &mut rows);
                let s = html_escape(change.value());
                rows.push((s.clone(), s, "diff-row-eq"));
            }
            similar::ChangeTag::Delete => {
                pending_del.push(change.value().to_string());
            }
            similar::ChangeTag::Insert => {
                pending_ins.push(change.value().to_string());
            }
        }
    }
    flush_pending(&mut pending_del, &mut pending_ins, &mut rows);

    let added_count: usize = rows.iter().filter(|(_, _, c)| *c == "diff-row-ins").count();
    let deleted_count: usize = rows.iter().filter(|(_, _, c)| *c == "diff-row-del").count();
    let changed_count: usize = rows.iter().filter(|(_, _, c)| *c == "diff-row-chg").count();
    let add_lines = added_count + changed_count;
    let del_lines = deleted_count + changed_count;

    let body = html! {
        h1 { "Diff: " (slug) }
        p.diff-header { "From " code { (&a_sha[..7.min(a_sha.len())]) } " to " code { (&b_sha[..7.min(b_sha.len())]) } }
        div.diff-stats { "+" (add_lines) " / −" (del_lines) " lines" }
        div.diff-two-col-wrap {
            table.diff-two-col {
                thead {
                    tr {
                        th.diff-col-old { "Before" }
                        th.diff-col-new { "After" }
                    }
                }
                tbody {
                    @for (left, right, cls) in &rows {
                        tr class=(cls) {
                            td.diff-cell-old { (PreEscaped(left)) }
                            td.diff-cell-new { (PreEscaped(right)) }
                        }
                    }
                }
            }
        }
    };

    let pending_count = pending_count_for(&state, maybe_user.as_ref()).await;
    Ok(chrome(
        &format!("Diff: {}", slug),
        body,
        &state.site_title,
        maybe_user.as_ref(),
        pending_count,
    ))
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Build (left_html, right_html) from a word-level diff of two lines.
/// Changed words are wrapped in `<del>` / `<ins>`; equal words are plain.
fn word_diff_pair(old_line: &str, new_line: &str) -> (String, String) {
    let wd = similar::TextDiff::from_words(old_line, new_line);
    let old_sl = wd.old_slices();
    let new_sl = wd.new_slices();
    let mut left = String::new();
    let mut right = String::new();
    for op in wd.ops() {
        match *op {
            similar::DiffOp::Equal { old_index, len, .. } => {
                for s in &old_sl[old_index..old_index + len] {
                    let e = html_escape(s);
                    left.push_str(&e);
                    right.push_str(&e);
                }
            }
            similar::DiffOp::Delete {
                old_index, old_len, ..
            } => {
                left.push_str("<del>");
                for s in &old_sl[old_index..old_index + old_len] {
                    left.push_str(&html_escape(s));
                }
                left.push_str("</del>");
            }
            similar::DiffOp::Insert {
                new_index, new_len, ..
            } => {
                right.push_str("<ins>");
                for s in &new_sl[new_index..new_index + new_len] {
                    right.push_str(&html_escape(s));
                }
                right.push_str("</ins>");
            }
            similar::DiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
            } => {
                left.push_str("<del>");
                for s in &old_sl[old_index..old_index + old_len] {
                    left.push_str(&html_escape(s));
                }
                left.push_str("</del>");
                right.push_str("<ins>");
                for s in &new_sl[new_index..new_index + new_len] {
                    right.push_str(&html_escape(s));
                }
                right.push_str("</ins>");
            }
        }
    }
    (left, right)
}

/// Phase 8 — look up an article by its blake3 content fingerprint.
async fn hash_lookup_page(
    State(state): State<Arc<AppState>>,
    Path(hash_hex): Path<String>,
    CurrentUser(maybe_user): CurrentUser,
) -> Result<Response, WikiError> {
    if hash_hex.len() != 64 || !hash_hex.chars().all(|c| c.is_ascii_hexdigit()) {
        let pending_count = pending_count_for(&state, maybe_user.as_ref()).await;
        let body = html! {
            h1 { "Hash lookup" }
            p { "Invalid fingerprint — expected 64 hex characters." }
        };
        return Ok(chrome(
            "Hash lookup",
            body,
            &state.site_title,
            maybe_user.as_ref(),
            pending_count,
        )
        .into_response());
    }

    let mut bytes = [0u8; 32];
    for (i, chunk) in hash_hex.as_bytes().chunks(2).enumerate() {
        let hi = (chunk[0] as char).to_digit(16).unwrap_or(0) as u8;
        let lo = (chunk[1] as char).to_digit(16).unwrap_or(0) as u8;
        bytes[i] = (hi << 4) | lo;
    }

    let pending_count = pending_count_for(&state, maybe_user.as_ref()).await;
    match state.links.lookup_by_hash(&bytes)? {
        Some((slug, revision_sha)) => {
            let short_sha = if revision_sha.len() >= 7 {
                &revision_sha[..7]
            } else {
                &revision_sha
            };
            let body = html! {
                h1 { "Hash lookup" }
                p {
                    "Article: "
                    a href={ "/wiki/" (&slug) } { (&slug) }
                    " at revision "
                    code { (short_sha) }
                }
                p {
                    a href={ "/diff/" (&slug) "?a=" (&revision_sha) "~&b=" (&revision_sha) } { "View diff for this revision" }
                }
            };
            Ok(chrome(
                "Hash lookup",
                body,
                &state.site_title,
                maybe_user.as_ref(),
                pending_count,
            )
            .into_response())
        }
        None => Err(WikiError::NotFound(format!(
            "fingerprint {}",
            &hash_hex[..16]
        ))),
    }
}

/// `GET /special/specialpages` — index of all available special pages,
/// equivalent to Wikipedia's Special:SpecialPages.
async fn special_pages_index(
    State(state): State<Arc<AppState>>,
    CurrentUser(maybe_user): CurrentUser,
) -> Result<Markup, WikiError> {
    let pending_count = pending_count_for(&state, maybe_user.as_ref()).await;

    // (group, [(path, title, description)])
    type Sp = (&'static str, &'static str, &'static str);
    let groups: &[(&str, &[Sp])] = &[
        ("Content", &[
            ("/special/all-pages",    "All pages",      "Browse every article in this wiki."),
            ("/special/categories",   "Categories",     "Browse articles by category."),
            ("/special/recent-changes", "Recent changes", "List of the most recent edits."),
            ("/special/statistics",   "Statistics",     "Article count, word count, and other metrics."),
        ]),
        ("Pages and files", &[
            ("/random",          "Random page",   "Jump to a randomly chosen article."),
            ("/special/wanted",  "Wanted pages",  "Articles linked to but not yet created."),
        ]),
        ("Per-page tools", &[
            ("/special/whatlinkshere/", "What links here", "Articles that link to a specific page."),
            ("/special/pageinfo/",      "Page information", "Metadata for a specific article."),
            ("/special/cite/",          "Cite this page",   "Citation templates for a specific article."),
            ("/special/hash-lookup/",   "Hash lookup",      "Find an article by its content fingerprint."),
        ]),
        ("Technical", &[
            ("/openapi.yaml",  "OpenAPI specification", "Machine-readable API description (OpenAPI 3.1)."),
            ("/robots.txt",    "robots.txt",           "Crawler exclusion rules."),
            ("/sitemap.xml",   "Sitemap",              "XML sitemap for search engines."),
            ("/feed.atom",     "Atom feed",            "Recent-changes feed in Atom format."),
            ("/feed.json",     "JSON feed",            "Recent-changes feed in JSON Feed 1.1 format."),
            ("/llms.txt",      "llms.txt",             "Plain-text index for large language models."),
        ]),
    ];

    Ok(chrome(
        &format!("Special pages — {}", state.site_title),
        html! {
            h1 { "Special pages" }
            p.wiki-special-intro { "Below is a list of all available special pages on this wiki." }
            @for (group_name, entries) in groups {
                h2 { (group_name) }
                ul.wiki-special-list {
                    @for (path, title, desc) in *entries {
                        li {
                            a href=(path) { (title) }
                            " — "
                            span.wiki-special-desc { (desc) }
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

#[cfg(test)]
mod llms_txt_tests {
    use super::llms_txt_snippet;

    #[test]
    fn snippet_short_body_returns_as_is() {
        let body = "Short body.";
        assert_eq!(llms_txt_snippet(body, 120), "Short body.");
    }

    #[test]
    fn snippet_truncates_at_word_boundary() {
        let body = "The quick brown fox jumps over the lazy dog and then some more words to exceed the limit.";
        let result = llms_txt_snippet(body, 50);
        assert!(result.ends_with('…'), "should end with ellipsis");
        assert!(result.len() <= 50 + '…'.len_utf8());
    }

    #[test]
    fn snippet_em_dash_near_boundary_does_not_panic() {
        // Regression: em dash (—, 3 bytes: 0xe2 0x80 0x94) crossing the 120-byte boundary
        // previously caused a panic in &s[..120] when byte 120 was inside the em dash.
        let prefix = "a".repeat(119);
        let body = format!("{}— extra words that push beyond the boundary", prefix);
        let result = llms_txt_snippet(&body, 120);
        assert!(result.ends_with('…') || result.len() <= body.len());
    }
}

