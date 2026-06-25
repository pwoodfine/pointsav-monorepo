
// ─── index handler ──────────────────────────────────────────────────────────

async fn index(
    State(state): State<Arc<AppState>>,
    CurrentUser(maybe_user): CurrentUser,
    Query(params): Query<IndexQueryParams>,
    headers: HeaderMap,
) -> Result<Response, WikiError> {
    if params.noredirect.is_none() && prefers_spanish(&headers) {
        return Ok(Response::builder()
            .status(StatusCode::FOUND)
            .header(header::LOCATION, "/es/")
            .body(axum::body::Body::empty())
            .unwrap());
    }
    home_inner(state, Locale::En, maybe_user)
        .await
        .map(IntoResponse::into_response)
}

fn prefers_spanish(headers: &HeaderMap) -> bool {
    headers
        .get("accept-language")
        .and_then(|v| v.to_str().ok())
        .map(|s| {
            // Check only the first (highest-quality) language tag.
            let first = s.split(',').next().unwrap_or("").trim();
            let tag = first.split(';').next().unwrap_or("").trim();
            tag.eq_ignore_ascii_case("es") || tag.to_ascii_lowercase().starts_with("es-")
        })
        .unwrap_or(false)
}

async fn home_es(
    State(state): State<Arc<AppState>>,
    CurrentUser(maybe_user): CurrentUser,
) -> Result<Markup, WikiError> {
    home_inner(state, Locale::Es, maybe_user).await
}

async fn home_inner(
    state: Arc<AppState>,
    locale: Locale,
    maybe_user: Option<User>,
) -> Result<Markup, WikiError> {
    let pending_count = pending_count_for(&state, maybe_user.as_ref()).await;
    // Prefer locale-specific index (index.es.md) when available.
    let home_path = state
        .primary_path()
        .join(format!("index{}.md", locale.suffix()));
    let home_path = if home_path.exists() {
        home_path
    } else {
        state.primary_path().join("index.md")
    };
    if !home_path.exists() {
        return placeholder_index(&state, maybe_user.as_ref(), pending_count).await;
    }

    let home_text = fs::read_to_string(&home_path).await?;
    let home_parsed = crate::render::parse_page(&home_text)?;
    let gds = state.guide_dirs_arr();
    let buckets = bucket_topics_by_category(
        state.primary_path(),
        gds[0],
        gds[1],
    )
    .await?;
    let recent = recent_topics_by_last_edited(&buckets, 8);
    let stats = compute_home_stats(&buckets);
    let home_html = crate::render::render_html_raw(
        &home_parsed.body_md,
        state.primary_path(),
        &state.link_roots(),
    );
    let home_html = crate::glossary::inject_glossary_tooltips(&home_html, &state.glossary);
    let featured = load_featured(state.primary_path(), &buckets).await;
    let dyk = load_dyk_localized(state.primary_path(), locale).await;
    let ref_inv = load_reference_invariants(state.primary_path()).await;
    let cat_descriptions =
        load_category_descriptions(state.primary_path(), RATIFIED_CATEGORIES).await;

    let mut guide_summaries: Vec<TopicSummary> = buckets
        .values()
        .flatten()
        .filter(|t| {
            t.slug
                .split('/')
                .next_back()
                .map(|s| s.starts_with("guide-"))
                .unwrap_or(false)
        })
        .cloned()
        .collect();
    guide_summaries.sort_by(|a, b| a.title.cmp(&b.title));

    Ok(home_chrome(
        locale,
        &home_parsed.frontmatter,
        &home_html,
        &buckets,
        &recent,
        &stats,
        &guide_summaries,
        featured,
        dyk,
        ref_inv,
        &cat_descriptions,
        &state.site_title,
        state.brand_theme.as_deref(),
        &state.brand_instance,
        &state.peers,
        &state.start_here,
        maybe_user.as_ref(),
        pending_count,
    ))
}

/// Search category subdirectories of the primary content dir (and any guide
/// dirs) for a file whose stem matches `bare_slug`. Returns the path-qualified
/// slug (`"category/bare_slug"`) when exactly one match is found, or
/// `None` if zero or more than one match exist (ambiguous).
async fn resolve_bare_slug(state: &AppState, bare_slug: &str) -> Option<String> {
    let guide_dirs = state.link_roots();
    let dirs: Vec<&std::path::Path> = std::iter::once(state.primary_path())
        .chain(guide_dirs)
        .collect();

    let mut found: Option<String> = None;
    for dir in &dirs {
        let mut entries = match fs::read_dir(dir).await {
            Ok(e) => e,
            Err(_) => continue,
        };
        while let Some(entry) = entries.next_entry().await.ok().flatten() {
            let ft = match entry.file_type().await {
                Ok(ft) => ft,
                Err(_) => continue,
            };
            if !ft.is_dir() {
                continue;
            }
            let dir_name = entry.file_name().to_string_lossy().to_string();
            if dir_name.starts_with('.') {
                continue;
            }
            let candidate = entry.path().join(format!("{bare_slug}.md"));
            if candidate.exists() {
                if found.is_some() {
                    return None; // Ambiguous: two subdirectories share the same stem.
                }
                found = Some(format!("{dir_name}/{bare_slug}"));
            }
        }
    }
    found
}

/// Scan all articles for one whose `aliases:` frontmatter lists `target_slug`.
/// Returns the canonical slug of the owning article when found, or `None`.
/// Runs only on miss (after all other resolution paths have failed), so the
/// hot path — article found by slug — is unaffected.
async fn resolve_alias_slug(state: &AppState, target_slug: &str) -> Option<String> {
    let topic_files =
        collect_all_topic_files(state.primary_path(), &state.guide_dirs_arr())
            .await
            .unwrap_or_default();
    for tf in &topic_files {
        if let Ok(text) = fs::read_to_string(&tf.path).await {
            if let Ok(parsed) = crate::render::parse_page(&text) {
                if parsed.frontmatter.aliases.iter().any(|a| a == target_slug) {
                    return Some(tf.slug.clone());
                }
            }
        }
    }
    None
}

async fn wiki_page(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
    Query(q): Query<WikiPageQuery>,
    CurrentUser(maybe_user): CurrentUser,
    headers: HeaderMap,
) -> Result<Response, WikiError> {
    wiki_page_inner(state, slug, Locale::En, q, maybe_user, headers).await
}

async fn wiki_page_es(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
    Query(q): Query<WikiPageQuery>,
    CurrentUser(maybe_user): CurrentUser,
    headers: HeaderMap,
) -> Result<Response, WikiError> {
    wiki_page_inner(state, slug, Locale::Es, q, maybe_user, headers).await
}

async fn wiki_page_inner(
    state: Arc<AppState>,
    slug: String,
    locale: Locale,
    q: WikiPageQuery,
    maybe_user: Option<User>,
    headers: HeaderMap,
) -> Result<Response, WikiError> {
    // Slug safety: reject path traversal. Allow at most one `/` separator
    // for category-scoped slugs (`architecture/compounding-substrate`).
    if slug.contains("..") || slug.is_empty() {
        return Err(WikiError::NotFound(slug));
    }
    // Validate component parts for safety.
    let parts: Vec<&str> = slug.splitn(3, '/').collect();
    if parts.len() > 2 {
        // More than one directory level — reject.
        return Err(WikiError::NotFound(slug));
    }
    for part in &parts {
        if part.is_empty() || part.starts_with('.') {
            return Err(WikiError::NotFound(slug.clone()));
        }
    }

    // Phase 0 slug normalization: /wiki/topic-foo → /wiki/foo (301).
    // Content files use topic-*.md names; the canonical URL drops the prefix.
    // Category-scoped slugs (cat/topic-foo) are handled by stripping from the
    // last path component only. Locale prefix is preserved in the Location header.
    {
        let bare = slug.rsplit('/').next().unwrap_or(&slug);
        if let Some(stripped) = bare.strip_prefix("topic-") {
            let clean = if let Some(pos) = slug.rfind('/') {
                format!("{}/{}", &slug[..pos], stripped)
            } else {
                stripped.to_string()
            };
            let location = match locale {
                Locale::Es => format!("/es/wiki/{clean}"),
                Locale::En => format!("/wiki/{clean}"),
            };
            return Ok(Response::builder()
                .status(StatusCode::MOVED_PERMANENTLY)
                .header(header::LOCATION, location)
                .body(axum::body::Body::empty())
                .unwrap());
        }
    }

    // For ES locale, try the .es.md sibling first (before the EN file).
    // `effective_locale` reflects what was actually served; used for lang= and hreflang.
    let mut effective_locale = Locale::En;
    if locale == Locale::Es && q.asof.is_none() {
        let es_path = state
            .primary_path()
            .join(format!("{slug}{}.md", Locale::Es.suffix()));
        if es_path.exists() {
            effective_locale = Locale::Es;
        }
    }

    // Try content_dir first; if not found, try guide_dir then guide_dir_2.
    let primary_path = state
        .primary_path()
        .join(format!("{slug}{}.md", effective_locale.suffix()));

    // §3.5: past-revision view — read from git history when ?asof= is set.
    // Only content_dir is git-tracked; guide_dir articles always use the
    // disk path. Any git error (unknown rev, empty repo) becomes a 404.
    let text = if let Some(ref rev) = q.asof {
        match crate::history::get_file_at_rev(state.primary_path(), &slug, rev) {
            Ok(t) if !t.is_empty() => t,
            _ => return Err(WikiError::NotFound(slug)),
        }
    } else {
        match fs::read_to_string(&primary_path).await {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let guide_dirs_arr = state.guide_dirs_arr();
                let mut found: Option<String> = None;
                for gd in guide_dirs_arr.iter().flatten() {
                    let gp = gd.join(format!("{slug}.md"));
                    if let Ok(t) = fs::read_to_string(&gp).await {
                        found = Some(t);
                        break;
                    }
                }
                match found {
                    Some(t) => t,
                    None => {
                        // Slug normalization fallback: if the slug has uppercase or
                        // spaces, try the lowercase+hyphenated form and redirect.
                        let norm = slug.to_lowercase().replace(' ', "-");
                        if norm != slug {
                            let norm_path = state.primary_path().join(format!("{norm}.md"));
                            if norm_path.exists() {
                                let location = format!("/wiki/{norm}");
                                return Ok(Response::builder()
                                    .status(StatusCode::MOVED_PERMANENTLY)
                                    .header(header::LOCATION, location)
                                    .body(axum::body::Body::empty())
                                    .unwrap());
                            }
                        }
                        // Bare-slug resolver: if the slug has no directory component,
                        // search category subdirectories for a unique stem match and
                        // 301-redirect to the qualified slug. Fixes wikilinks that were
                        // written before the Wave-1 category-subdirectory migration.
                        if !slug.contains('/') {
                            if let Some(full) = resolve_bare_slug(&state, &slug).await {
                                let location = format!("/wiki/{full}");
                                return Ok(Response::builder()
                                    .status(StatusCode::MOVED_PERMANENTLY)
                                    .header(header::LOCATION, location)
                                    .body(axum::body::Body::empty())
                                    .unwrap());
                            }
                        }
                        // Alias resolver: scan all articles for one whose `aliases:`
                        // frontmatter lists this slug, then 301-redirect to it.
                        // Only runs after all other resolution paths have failed.
                        if let Some(canonical) = resolve_alias_slug(&state, &slug).await {
                            let location = format!("/wiki/{canonical}?redirectedfrom={slug}");
                            return Ok(Response::builder()
                                .status(StatusCode::MOVED_PERMANENTLY)
                                .header(header::LOCATION, location)
                                .body(axum::body::Body::empty())
                                .unwrap());
                        }
                        // topic- filename fallback: content repos name files as
                        // topic-foo.md; serve them at /wiki/foo (canonical slug).
                        // For ES locale, try topic-foo.es.md first before the EN file.
                        {
                            if locale == Locale::Es {
                                let es_topic = state.primary_path().join(
                                    format!("topic-{slug}{}.md", Locale::Es.suffix()),
                                );
                                if let Ok(t) = fs::read_to_string(&es_topic).await {
                                    effective_locale = Locale::Es;
                                    t
                                } else {
                                    let topic_path =
                                        state.primary_path().join(format!("topic-{slug}.md"));
                                    if let Ok(t) = fs::read_to_string(&topic_path).await {
                                        t
                                    } else {
                                        return Err(WikiError::NotFound(slug));
                                    }
                                }
                            } else {
                                let topic_path =
                                    state.primary_path().join(format!("topic-{slug}.md"));
                                if let Ok(t) = fs::read_to_string(&topic_path).await {
                                    t
                                } else {
                                    return Err(WikiError::NotFound(slug));
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => return Err(e.into()),
        }
    }; // end of asof / disk read block
    let mut parsed = parse_page(&text)?;

    // A5: redirect_to frontmatter — 301 before any rendering.
    // Pass ?redirectedfrom=<slug> so the target page can render a hatnote.
    if let Some(ref target) = parsed.frontmatter.redirect_to.clone() {
        let location = format!("/wiki/{target}?redirectedfrom={slug}");
        return Ok(Response::builder()
            .status(StatusCode::MOVED_PERMANENTLY)
            .header(header::LOCATION, location)
            .body(axum::body::Body::empty())
            .unwrap());
    }

    // ── Item 11: Language toggle auto-detection ───────────────────────────
    //
    // If `translations:` frontmatter is absent or empty, check whether a
    // bilingual sibling exists on disk and inject the toggle automatically.
    //
    // Two cases:
    //   (A) Viewing the EN article (slug does NOT end in `.es`):
    //       Look for `<slug>.es.md`; if present, inject { "es" → "<slug>.es" }.
    //   (B) Viewing the ES article (slug ends in `.es`):
    //       Derive the base slug by stripping `.es`; if `<base>.md` exists,
    //       inject { "en" → "<base>" }.
    //
    // This means every article that has a sibling gets the language toggle
    // without requiring the content author to maintain `translations:` by hand.
    if parsed
        .frontmatter
        .translations
        .as_ref()
        .map(|t| t.is_empty())
        .unwrap_or(true)
    {
        let is_es = slug.ends_with(".es");
        if is_es {
            // Case B: we're on an ES article; offer EN link.
            let base_slug = slug.trim_end_matches(".es");
            let base_path = state.primary_path().join(format!("{base_slug}.md"));
            if base_path.exists() {
                let mut map = TranslationMap::new();
                map.insert("en".to_string(), base_slug.to_string());
                parsed.frontmatter.translations = Some(map);
            }
        } else {
            // Case A: we're on an EN article; offer ES link if sibling exists.
            let es_slug = format!("{slug}.es");
            let es_path = state.primary_path().join(format!("{es_slug}.md"));
            if es_path.exists() {
                let mut map = TranslationMap::new();
                map.insert("es".to_string(), es_slug);
                parsed.frontmatter.translations = Some(map);
            }
        }
    }

    // §3.5: extract claims and enrich with per-span blame timestamps.
    // fm_line_count converts body-relative line numbers to absolute file
    // lines that topic_blame addresses. Blame is skipped for past-revision
    // views (?asof=) — the claim graph reflects HEAD, not the past revision.
    let fm_line_count = text[..text.len().saturating_sub(parsed.body_md.len())]
        .matches('\n')
        .count() as u32;
    let mut claims = crate::claim::extract_claims(&parsed.body_md, &slug).claims;
    if q.asof.is_none() {
        crate::history::blame_published_at(state.primary_path(), &slug, fm_line_count, &mut claims);
    }

    // §3.7: JSON content-negotiation — return structured JSON when the client
    // prefers application/json. Skips the HTML render path entirely.
    let wants_json = headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .map(|s| {
            s.split(',')
                .any(|part| part.trim().starts_with("application/json"))
        })
        .unwrap_or(false);
    if wants_json {
        let blake3_hex = blake3::hash(text.as_bytes()).to_hex().to_string();
        let revision_sha = crate::history::topic_history(state.primary_path(), &slug, 1)
            .ok()
            .and_then(|mut h| h.drain(..).next())
            .map(|e| e.sha)
            .unwrap_or_default();
        let backlinks = state.links.backlinks(&slug).unwrap_or_default();
        return Ok(Json(json!({
            "frontmatter": serde_json::to_value(&parsed.frontmatter).unwrap_or_default(),
            "body_md": parsed.body_md,
            "blake3": blake3_hex,
            "revision_sha": revision_sha,
            "backlinks": backlinks,
            "claims": claims,
        }))
        .into_response());
    }

    // Two-step render: extract headings from clean comrak output (no edit pencils),
    // then inject pencils for the final body HTML. This keeps TOC text clean.
    let raw_html = render_html_raw(&parsed.body_md, state.primary_path(), &state.link_roots());
    let raw_html = crate::glossary::inject_glossary_tooltips(&raw_html, &state.glossary);
    let raw_html = crate::render::inject_citation_markers(&raw_html);
    let cite_registry = crate::citations::CitationRegistry::load(&state.citations_yaml).ok();
    let raw_html = crate::render::inject_citation_refs(&raw_html, cite_registry.as_ref());
    let is_journal = parsed.frontmatter.layout.as_deref() == Some("journal");
    let raw_html = crate::render::inject_sidenotes(&raw_html, is_journal);
    let headings = extract_headings(&raw_html);
    let body_html = inject_edit_pencils(&raw_html);

    // CHANGE 14: Strip <!--claim…-->  / <!--/claim--> HTML comment markers
    // from the final body HTML before serving. The claim extraction above has
    // already consumed the structured data; shipping the markers to readers
    // bloats page source and leaks internal annotation syntax.
    let body_html = crate::claim::strip_claim_markers(&body_html);

    // §3.5: past-revision notice — minimal engine-side render.
    // Freshness-ribbon visual is project-design component `component-freshness-ribbon`.
    let body_html = if let Some(ref rev) = q.asof {
        let notice = format!(
            concat!(
                r#"<div class="wiki-asof-notice" style="background:#fef3cd;"#,
                r#"border:1px solid #ffc107;padding:.5em 1em;margin-bottom:1em;"#,
                r#"border-radius:3px"><strong>Historical revision:</strong> "#,
                r#"showing <code>{slug}</code> as of <code>{rev}</code>. "#,
                r#"<a href="/wiki/{slug}">Return to current revision.</a></div>"#,
            ),
            slug = &slug,
            rev = rev,
        );
        format!("{notice}{body_html}")
    } else {
        body_html
    };

    let title = parsed
        .frontmatter
        .title
        .clone()
        .unwrap_or_else(|| slug.clone());

    let pending_count = pending_count_for(&state, maybe_user.as_ref()).await;
    let redirected_from = q.redirectedfrom.as_deref();
    let body_fingerprint = {
        let h = blake3::hash(parsed.body_md.as_bytes());
        let hex = h.to_hex();
        hex[..16].to_string()
    };
    // Phase 9 — build claim-rail HTML from CITATIONS table entries found in body
    let claim_rail_html = {
        let re = regex::Regex::new(r##"href="#fn-(\d+)""##).unwrap();
        let mut ticks = String::new();
        for cap in re.captures_iter(&body_html) {
            let n = &cap[1];
            let cite_id = format!("{}:fn-{}", slug, n);
            let status = state.links.citation_status(&cite_id);
            ticks.push_str(&format!(
                "<a class=\"claim-tick\" data-status=\"{status}\" data-cite-id=\"{cite_id}\" \
                 data-para=\"fn-{n}\" href=\"#fn-{n}\" title=\"Citation {n} ({status})\" \
                 aria-label=\"Citation {n}\"></a>"
            ));
        }
        if ticks.is_empty() {
            String::new()
        } else {
            format!("<aside class=\"claim-rail\" aria-label=\"Citation freshness\">{ticks}</aside>")
        }
    };
    // Load category buckets (cached, ~20s TTL) for prev/next navigation and sidenav.
    let nav_buckets = nav_buckets_cached(&state).await;
    let article_category: String = parsed.frontmatter.category
        .clone()
        .unwrap_or_else(|| slug.find('/').map(|p| slug[..p].to_owned()).unwrap_or_default());
    let (prev_article, next_article) = if article_category.is_empty() {
        (None, None)
    } else if let Some(articles) = nav_buckets.get(&article_category) {
        if let Some(pos) = articles.iter().position(|t| t.slug == slug) {
            let prev = if pos > 0 { Some(articles[pos - 1].clone()) } else { None };
            let next = if pos + 1 < articles.len() { Some(articles[pos + 1].clone()) } else { None };
            (prev, next)
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };
    // Phase 0 — blueprint relates_to rails.
    // Each rail is (human label, vec of related slugs). Empty when the page
    // type has no blueprint or no backlinks/forward-links of the related type.
    let relates_to_rails: Vec<(String, Vec<String>)> = {
        let ct = parsed.frontmatter.content_type.as_deref().unwrap_or("topic");
        let mut rails: Vec<(String, Vec<String>)> = Vec::new();
        if let Some(bp) = state.blueprints.find(ct) {
            for rel in &bp.relates_to {
                let candidates: Vec<String> = match rel.via.as_str() {
                    "link" => state.links.forward_links(&slug).unwrap_or_default(),
                    _ => state.links.backlinks(&slug).unwrap_or_default(),
                };
                let type_prefix = format!("{}-", rel.r#type);
                let filtered: Vec<String> = candidates
                    .into_iter()
                    .filter(|s| {
                        s.rsplit('/').next().unwrap_or(s).starts_with(&type_prefix)
                    })
                    .take(8)
                    .collect();
                if !filtered.is_empty() {
                    rails.push((rel.as_label.clone(), filtered));
                }
            }
        }
        rails
    };
    Ok(wiki_chrome(
        effective_locale,
        &title,
        &slug,
        parsed.frontmatter,
        &body_html,
        headings,
        &state.site_title,
        state.brand_theme.as_deref(),
        &state.brand_instance,
        &state.peers,
        maybe_user.as_ref(),
        pending_count,
        redirected_from,
        q.printable,
        &body_fingerprint,
        &claim_rail_html,
        prev_article.as_ref(),
        next_article.as_ref(),
        &relates_to_rails,
        &nav_buckets,
        &article_category,
    )
    .into_response())
}

async fn static_asset(Path(path): Path<String>) -> Response {
    match StaticAsset::get(&path) {
        Some(asset) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            let hash = asset.metadata.sha256_hash();
            let etag = format!(
                "\"{}\"",
                hash.iter().map(|b| format!("{b:02x}")).collect::<String>()
            );
            let mut resp = asset.data.into_owned().into_response();
            let hdrs = resp.headers_mut();
            if let Ok(v) = HeaderValue::from_str(mime.as_ref()) {
                hdrs.insert(header::CONTENT_TYPE, v);
            }
            // Cache-Control by asset type. Filenames for fonts/images are stable
            // and their content never changes in place, so they get a 1-year
            // immutable cache. CSS/JS share stable filenames but DO change on each
            // deploy, and the page links them unversioned — so they must revalidate
            // every load (cheap 304 via ETag when unchanged; the new file is fetched
            // instantly after a deploy). This is what ends the "stale CSS after
            // deploy" problem. (A future enhancement is content-hashed asset URLs.)
            let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
            let cache = match ext.as_str() {
                "woff2" | "woff" | "ttf" | "otf" | "png" | "svg" | "ico" | "jpg" | "jpeg"
                | "webp" | "gif" | "avif" => "public, max-age=31536000, immutable",
                "css" | "js" | "mjs" | "map" | "json" => "public, max-age=0, must-revalidate",
                _ => "public, max-age=3600",
            };
            hdrs.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
            if let Ok(v) = HeaderValue::from_str(&etag) {
                hdrs.insert(header::ETAG, v);
            }
            resp
        }
        None => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

/// `GET /images/{*path}` — serves images from `<content_dir>/images/`.
///
/// Path is validated with `validate_slug` (rejects `..`, leading `.`, and
/// non-lowercase characters) before being joined onto the content directory.
async fn serve_content_image(
    State(state): State<Arc<AppState>>,
    Path(path): Path<String>,
) -> Response {
    if validate_slug(&path).is_err() {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    }
    let target = state.primary_path().join("images").join(&path);
    match fs::read(&target).await {
        Ok(bytes) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            let mut resp = bytes.into_response();
            let hdrs = resp.headers_mut();
            if let Ok(v) = HeaderValue::from_str(mime.as_ref()) {
                hdrs.insert(header::CONTENT_TYPE, v);
            }
            hdrs.insert(
                header::CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=31536000, immutable"),
            );
            resp
        }
        Err(_) => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

/// Full article-page shell with Phase 1.1 Wikipedia muscle-memory chrome.
///
/// Additive over Phase 1's `chrome()`: the existing chrome function is
/// untouched and continues to serve the index page. This function is used
/// only by `wiki_page`.
///
/// Elements added (all additive; no existing behaviour changed):
/// - Article / Talk tab pair (top-left of title row)
/// - Read / Edit / View history tabs (top-right; Edit and View-history are
///   `href="#"` placeholders — Phase 2 wires the routes)
/// - IVC masthead band placeholder (horizontal strip below title row)
/// - Collapsible left-rail TOC with sticky scroll (Vector 2022 pattern)
/// - Language-switcher button (populated from frontmatter `translations:`)
/// - Hatnote (italic, indented; only when `hatnote:` frontmatter is present)
/// - "From PointSav Knowledge" tagline below the title
/// - `short_description` subtitle (italic, below H1; iteration-2 addition)
/// - Breadcrumb navigation (Documentation > Category > Title; iteration-2)
/// - Reader density toggle (Off / Exceptions only / All; localStorage)
/// - Per-section [edit] pencils (injected into rendered HTML by render module)
/// - Footer block: categories → license → about/contact links
#[allow(clippy::too_many_arguments)]
fn wiki_chrome(
    locale: Locale,
    title: &str,
    slug: &str,
    fm: Frontmatter,
    body_html: &str,
    headings: Vec<(String, String, u8)>,
    site_title: &str,
    brand_theme: Option<&str>,
    brand_instance: &str,
    peers: &[crate::config::PeerConfig],
    user: Option<&User>,
    _pending_count: i64,
    redirected_from: Option<&str>,
    printable: bool,
    _body_blake3: &str,
    claim_rail_html: &str,
    prev_article: Option<&TopicSummary>,
    next_article: Option<&TopicSummary>,
    relates_to_rails: &[(String, Vec<String>)],
    nav_buckets: &CategoryBuckets,
    article_category: &str,
) -> Markup {
    let woodfine_theme = matches!(brand_theme, Some("woodfine") | Some("woodfine-projects"));
    let _woodfine_projects = brand_theme == Some("woodfine-projects");
    let is_authenticated = user.is_some();
    let auth_attr = if is_authenticated { "user" } else { "anon" };
    let _talk_slug = format!("{slug}.talk");
    let page_title = format!("{title} — {site_title}");
    let tenant = Tenant::from_str(brand_instance);
    let lang_href = match locale { Locale::En => format!("/es/wiki/{slug}"), Locale::Es => format!("/wiki/{slug}") };

    // B5: Precompute ToC entries with hierarchical section numbers (1, 2, 2.1, etc.)
    let numbered_headings: Vec<(String, String, u8, String)> = {
        let mut counters = [0usize; 7];
        headings
            .iter()
            .map(|(id, text, level)| {
                let lvl = *level as usize;
                counters[lvl] += 1;
                for c in &mut counters[lvl + 1..] {
                    *c = 0;
                }
                let num = counters[1..=lvl]
                    .iter()
                    .skip_while(|n| **n == 0)
                    .map(|n| n.to_string())
                    .collect::<Vec<_>>()
                    .join(".");
                (id.clone(), text.clone(), *level, num)
            })
            .collect()
    };

    html! {
        (DOCTYPE)
        html lang=(locale.lang_attr())
             data-auth=(auth_attr)
             data-instance=(brand_instance) {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover";
                title { (page_title) }
                // Font preload — eliminates FOUT on first load
                link rel="preload" as="font" type="font/woff2" crossorigin href="/static/fonts/IBMPlexSans-Variable-latin.woff2";
                link rel="preload" as="font" type="font/woff2" crossorigin href="/static/fonts/PlayfairDisplay-Variable-latin.woff2";
                link rel="stylesheet" href="/static/tokens.css";
                link rel="stylesheet" href="/static/style.css";
                @if woodfine_theme {
                    // Brand override loads AFTER style.css so its :root tokens (e.g. --accent)
                    // win over style.css's defaults — otherwise the per-brand theme is dead.
                    link rel="stylesheet" href="/static/tokens-woodfine.css";
                }
                // Anti-FOUT: apply stored theme/width before first paint to
                // avoid a flash of the default light theme for dark-mode users.
                script { (PreEscaped(r#"(function(){var t=localStorage.getItem('wiki-theme')||'light';document.documentElement.setAttribute('data-theme',t);var w=localStorage.getItem('wiki-width')||'standard';document.documentElement.setAttribute('data-width',w);}());"#)) }
                // hreflang + canonical for bilingual articles
                @match locale {
                    Locale::En => {
                        link rel="alternate" hreflang="es" href={ "/es/wiki/" (slug) };
                        link rel="canonical" href={ "/wiki/" (slug) };
                    }
                    Locale::Es => {
                        link rel="alternate" hreflang="en" href={ "/wiki/" (slug) };
                        link rel="canonical" href={ "/es/wiki/" (slug) };
                    }
                }
                // JSON-LD baseline (Phase 2 Step 1) — schema.org TechArticle /
                // DefinedTerm. Cumulative across phases; AEO crawlers + downstream
                // consumers ingest the structured data.
                (PreEscaped(jsonld_for_topic(&fm, slug)))
            }
            body class=(if printable { "printable" } else { "" }) data-slug=(slug) {
                div.reading-progress-bar aria-hidden="true" {}
                a class="skip-to-content" href="#mw-content-text" { "Skip to content" }
                (sovereign_nav(tenant, locale.lang_attr(), site_title, &lang_href))
                // Mobile-only toggle buttons placed outside topnav so the header
                // height is consistent across all page types (P1 fix).
                div.mobile-topnav-toggles {
                    @if !numbered_headings.is_empty() {
                        button.toc-toggle-btn.mobile-only #toc-toggle-btn
                            aria-label="Contents"
                            aria-expanded="false"
                            aria-controls="mobile-toc-drawer"
                        { "Contents" }
                    }
                }

                // Mobile nav drawer — hidden on desktop, toggled by hamburger button
                nav.mobile-nav-drawer #mobile-nav-drawer aria-hidden="true" {
                    div.mobile-nav-header {
                        a.site-title href="/" { (site_title) }
                        button.mobile-nav-close #mobile-nav-close aria-label="Close navigation" { "Close" }
                    }
                    // Sprint K: article ToC inside the nav drawer (visible above nav links)
                    @if !numbered_headings.is_empty() {
                        p.mobile-drawer-section-heading { "Contents" }
                        ol.mobile-toc-list.mobile-nav-toc {
                            @for (id, text, level, num) in &numbered_headings {
                                li class={ "toc-level-" (level) } {
                                    a href={ "#" (id) } {
                                        span.toc-numb { (num) }
                                        " "
                                        (text)
                                    }
                                }
                            }
                        }
                        hr.mobile-drawer-divider;
                        p.mobile-drawer-section-heading { "Navigation" }
                    }
                    ul.mobile-nav-list {
                        li { a href="/" { "Home" } }
                        li { a href="/search" { "Search" } }
                        li { a href="/random" { "Random article" } }
                        li { a href="/wanted" { "Wanted articles" } }
                        li { a href="/special/all-pages" { "All pages" } }
                        li { a href="/special/categories" { "Categories" } }
                        li { a href="/special/recent-changes" { "Recent changes" } }
                        li { a href="/special/statistics" { "Statistics" } }
                    }
                }
                // Mobile ToC drawer — hidden on desktop, toggled by § button
                @if !numbered_headings.is_empty() {
                    div.mobile-toc-drawer #mobile-toc-drawer aria-hidden="true" {
                        div.mobile-nav-header {
                            span.mobile-drawer-title { "Contents" }
                            button.mobile-toc-close #mobile-toc-close aria-label="Close contents" { "Close" }
                        }
                        ol.mobile-toc-list {
                            @for (id, text, level, num) in &numbered_headings {
                                li class={ "toc-level-" (level) } {
                                    a href={ "#" (id) } {
                                        span.toc-numb { (num) }
                                        " "
                                        (text)
                                    }
                                }
                            }
                        }
                    }
                }
                div.mobile-nav-overlay #mobile-nav-overlay aria-hidden="true" {}

                // Cross-instance discovery strip — only rendered when peers are configured
                @if !peers.is_empty() {
                    nav.peer-strip aria-label="Also on this platform" {
                        span.peer-strip__label { "Also on this platform" }
                        @for peer in peers {
                            a.peer-strip__link href=(peer.url) rel="noopener" {
                                (peer.label)
                            }
                        }
                    }
                }

                // Mobile bottom action bar (wiki_chrome only; CSS hides on desktop)
                nav.mobile-bottom-bar aria-label="Mobile article actions" {
                    button.mobile-bar-btn #mobile-bar-toc aria-label="Contents" { "Contents" }
                    button.mobile-bar-btn #mobile-bar-share aria-label="Share" { "Share" }
                    a.mobile-bar-btn.tab-edit
                        href={ "/git/" (slug) }
                        aria-label="Edit this article"
                    { (match locale { Locale::En => "Edit", Locale::Es => "Editar" }) }
                    a.mobile-bar-btn
                        href={ "/history/" (slug) }
                        aria-label="View history"
                    { (match locale { Locale::En => "History", Locale::Es => "Historial" }) }
                }

                // Article layout: sidenav + article body (+ optional claim-rail at ≥1280px).
                div.shell {

                    // --- Left sidenav: internal category navigation (hidden below 1024px) ---
                    nav.docs-sidenav aria-label="Documentation navigation" {
                        div.docs-sidenav__inner {
                            @for cat_slug in RATIFIED_CATEGORIES {
                                @let cat_articles = nav_buckets.get(*cat_slug)
                                    .map(|v| v.as_slice())
                                    .unwrap_or(&[]);
                                @let is_open = *cat_slug == article_category;
                                details.docs-sidenav__cat open[is_open] {
                                    summary { (humanize_category(cat_slug)) }
                                    @if !cat_articles.is_empty() {
                                        ul.docs-sidenav__list {
                                            @for art in cat_articles {
                                                @let is_current = art.slug == slug;
                                                li {
                                                    @if is_current {
                                                        a.docs-sidenav__link.is-active
                                                            href={ "/wiki/" (art.slug) }
                                                            aria-current="page"
                                                        { (art.title) }
                                                    } @else {
                                                        a.docs-sidenav__link
                                                            href={ "/wiki/" (art.slug) }
                                                        { (art.title) }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // --- Article body column (two-column: prose + TOC) ---
                    main.article-wrap {
                    article.article__body data-content-type=(fm.content_type.as_deref().unwrap_or("article")) {

                        // Clean product-docs article header: breadcrumb, title,
                        // content-type badge, lede, language switcher, last-edited.
                        // Encyclopedia article header: tabs + tagline + breadcrumb + title.
                        header.doc-header {
                            // Wikipedia-pattern Article/Talk/Edit/History tab strip.
                            nav.wiki-page-tabs aria-label="Page tabs" {
                                a.wiki-tab aria-current="page" href={ "/wiki/" (slug) } {
                                    (match locale { Locale::En => "Article", Locale::Es => "Artículo" })
                                }
                                a.wiki-tab href={ "/talk/" (slug) } {
                                    (match locale { Locale::En => "Talk", Locale::Es => "Discusión" })
                                }
                                a.wiki-tab href={ "/git/" (slug) } {
                                    (match locale { Locale::En => "Edit", Locale::Es => "Editar" })
                                }
                                a.wiki-tab href={ "/history/" (slug) } {
                                    (match locale { Locale::En => "History", Locale::Es => "Historial" })
                                }
                            }
                            @if let Some(ref cat) = fm.category {
                                @if cat != "root" {
                                    nav.crumb aria-label="breadcrumb" {
                                        a href="/" { (match locale { Locale::En => "Home", Locale::Es => "Inicio" }) }
                                        " › "
                                        a href={ "/category/" (cat) } { (humanize_category(cat)) }
                                        " › "
                                        (title)
                                    }
                                }
                            }
                            div.doc-header__titlewrap {
                                h1.article__title { (title) }
                                // P4: content-type badge always visible; defaults to "topic".
                                @let ct = fm.content_type.as_deref().unwrap_or("topic");
                                @let badge_label = match ct {
                                    "guide"     => "Guide",
                                    "topic"     => "Topic",
                                    "research"  => "Research",
                                    "reference" => "Reference",
                                    "category"  => "Category",
                                    "article"   => "Article",
                                    _           => "Topic",
                                };
                                span.content-type-badge data-type=(ct) { (badge_label) }
                            }
                            // Wikipedia "From <wiki>" tagline.
                            p.wiki-tagline { "From the " (site_title) }
                            @if let Some(ref desc) = fm.short_description {
                                p.article__lede { (desc) }
                            }
                            div.doc-header__meta {
                                @if let Some(ref date) = fm.last_edited {
                                    span.doc-header__edited {
                                        "Updated " (date)
                                        " · "
                                        a href={ "/history/" (slug) } { "History" }
                                    }
                                }
                                @if let Some(translations) = &fm.translations {
                                    @if !translations.is_empty() {
                                        span.wiki-lang-switcher {
                                            @for (lang, lang_slug) in translations {
                                                @let lang_label = match lang.as_str() {
                                                    "es" => "Español",
                                                    "en" => "English",
                                                    "fr" => "Français",
                                                    "de" => "Deutsch",
                                                    "pt" => "Português",
                                                    "zh" => "中文",
                                                    "ja" => "日本語",
                                                    "ar" => "العربية",
                                                    _ => lang.as_str(),
                                                };
                                                a.wiki-lang-btn
                                                    href={ "/wiki/" (lang_slug) }
                                                    lang=(lang)
                                                    hreflang=(lang)
                                                    title={ "Read in " (lang_label) }
                                                { (lang_label) }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Audience chips: compact role indicators from frontmatter `audience:`
                        @if !fm.audience.is_empty() {
                            div.audience-chips {
                                @for aud in &fm.audience {
                                    span.audience-chip {
                                        (match aud.as_str() {
                                            "customer-woodfine" => "Woodfine Group",
                                            "operator"          => "Operator",
                                            "developer"         => "Developer",
                                            "public"            => "Public",
                                            other               => other,
                                        })
                                    }
                                }
                            }
                        }

                        // Redirected-from hatnote: shown when arriving via a redirect page
                        @if let Some(from_slug) = redirected_from {
                            div.wiki-redirected-from {
                                "(Redirected from "
                                a href={ "/wiki/" (from_slug) } { (from_slug) }
                                ")"
                            }
                        }

                        // Forward-looking-information notice (unchanged from Phase 1)
                        @if fm.forward_looking {
                            aside.fli-notice {
                                strong { "Forward-looking information." }
                                " Statements herein are subject to material assumptions and risks. "
                                "Per NI 51-102 / OSC SN 51-721 disclosure posture."
                            }
                        }

                        // Stub notice: hatnote-style banner when status == "stub"
                        @if fm.status.as_deref() == Some("stub") {
                            div.stub-notice {
                                em { "This article is a stub. You can expand it." }
                            }
                        }

                        // Hatnote (item 6): typed or freehand, top of article body.
                        // hatnote_type selects the rendering variant; hatnote: supplies
                        // the target slug for "main" and "see-also" types.
                        @match fm.hatnote_type.as_deref() {
                            Some("main") => {
                                @if let Some(slug) = &fm.hatnote {
                                    div.wiki-hatnote.hatnote--main {
                                        "Main article: "
                                        a href={ "/wiki/" (slug) } { (slug) }
                                    }
                                }
                            }
                            Some("see-also") => {
                                @if let Some(slug) = &fm.hatnote {
                                    div.wiki-hatnote.hatnote--see-also {
                                        "See also: "
                                        a href={ "/wiki/" (slug) } { (slug) }
                                    }
                                }
                            }
                            Some("disambig") => {
                                div.wiki-hatnote.hatnote--disambig {
                                    "This page is a disambiguation page."
                                }
                            }
                            _ => {
                                // "note", unknown type, or absent — freehand text fallback
                                @if let Some(hatnote) = &fm.hatnote {
                                    div.wiki-hatnote {
                                        (hatnote)
                                    }
                                }
                            }
                        }

                        // Phase 5: Guide steps — structured ol from frontmatter `steps:` array
                        @if fm.content_type.as_deref() == Some("guide") {
                            @if let Some(steps_val) = fm.extra.get("steps") {
                                @if let Some(steps) = steps_val.as_sequence() {
                                    @if !steps.is_empty() {
                                        ol.guide-steps {
                                            @for step in steps {
                                                @if let Some(text) = step.as_str() {
                                                    li { (text) }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Phase 5: Research methodology box from frontmatter `methodology:`
                        @if fm.content_type.as_deref() == Some("research") {
                            @if let Some(method_val) = fm.extra.get("methodology") {
                                @if let Some(method) = method_val.as_str() {
                                    aside.methodology-box {
                                        h4 { "Methodology" }
                                        p { (method) }
                                    }
                                }
                            }
                        }

                        // A6: Disambiguation page notice
                        @if fm.disambig == Some(true) {
                            div.wiki-disambig-notice {
                                em {
                                    "This disambiguation page lists articles associated with the same title. "
                                    "If an internal link led you here, you may wish to change the link to point directly to the intended article."
                                }
                            }
                        }

                        // Frontmatter-driven infobox (float-right summary table).
                        // Alternative to the code-fence infobox block in prose.
                        @if let Some(ref ib) = fm.infobox {
                            aside.infobox {
                                @if let Some(ref t) = ib.title {
                                    div.infobox-title { (t) }
                                }
                                @if let Some(ref img_src) = ib.image {
                                    div.infobox-image {
                                        img src=(img_src) alt=(ib.title.as_deref().unwrap_or("")) loading="lazy";
                                    }
                                }
                                @if !ib.rows.is_empty() {
                                    table.infobox-table {
                                        tbody {
                                            @for row in &ib.rows {
                                                tr {
                                                    th.infobox-label { (row.label) }
                                                    td.infobox-data { (row.value) }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Article body
                        div.prose #mw-content-text
                            data-layout=(fm.layout.as_deref().unwrap_or(""))
                            data-numbered=(if fm.auto_number { "true" } else { "false" })
                        {
                            (PreEscaped(body_html))
                        }

                        // E2: Research Trail Footer — collapsible <details> from frontmatter.
                        @if let Some(ref trail) = fm.research_trail {
                            @if !trail.is_empty() {
                                details.wiki-research-trail {
                                    summary { "Research trail" }
                                    dl.wiki-research-trail-dl {
                                        @for (key, val) in trail {
                                            dt { (key) }
                                            dd {
                                                @match val {
                                                    serde_yaml::Value::String(s) => (s),
                                                    serde_yaml::Value::Sequence(seq) => {
                                                        ul {
                                                            @for item in seq {
                                                                @if let serde_yaml::Value::String(ref s) = item {
                                                                    li { (s) }
                                                                }
                                                            }
                                                        }
                                                    }
                                                    other => (format!("{other:?}"))
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // End-of-article footer block (item 5 + item 15)
                        footer.wiki-article-footer {
                            // Categories list (from `categories:` array — item 15)
                            @if let Some(cats) = &fm.categories {
                                @if !cats.is_empty() {
                                    div.wiki-categories {
                                        span.cats-label { "Categories:" }
                                        ul.cats-list {
                                            @for cat in cats {
                                                li { a href={ "/category/" (cat) } { (cat) } }
                                            }
                                        }
                                    }
                                }
                            }
                            // Singular category tag from `category:` field when `categories:` absent
                            @else if let Some(ref cat) = fm.category {
                                @if cat != "root" {
                                    div.wiki-categories {
                                        span.cats-label { "Category:" }
                                        span.wiki-category-single-tag {
                                            a href={ "/category/" (cat) } { (humanize_category(cat)) }
                                        }
                                    }
                                }
                            }

                            // Last-edited date — Wikipedia footer convention
                            @if let Some(ref date) = fm.last_edited {
                                div.wiki-article-last-edited {
                                    "Last edited: "
                                    time datetime=(date) { (date) }
                                }
                            }

                        }

                        // Phase 0 — blueprint relates_to rails (e.g. "How-to guides" on topics).
                        @for (label, slugs) in relates_to_rails {
                            @if !slugs.is_empty() {
                                section.relates-to-rail aria-label=(label) {
                                    h3.relates-to-rail__heading { (label) }
                                    ul.relates-to-rail__list {
                                        @for related_slug in slugs {
                                            @let display = related_slug.rsplit('/').next().unwrap_or(related_slug)
                                                .trim_start_matches("guide-")
                                                .trim_start_matches("topic-")
                                                .replace('-', " ");
                                            li {
                                                a href={ "/wiki/" (related_slug) } { (display) }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // P3: Previous / Next article navigation
                        @if prev_article.is_some() || next_article.is_some() {
                            nav.article-nav aria-label="Article navigation" {
                                @if let Some(prev) = prev_article {
                                    a.article-nav__prev href={ "/wiki/" (prev.slug) } {
                                        span.article-nav__label { "← Previous" }
                                        span.article-nav__title { (prev.title) }
                                    }
                                }
                                @if let Some(next) = next_article {
                                    a.article-nav__next href={ "/wiki/" (next.slug) } {
                                        span.article-nav__label { "Next →" }
                                        span.article-nav__title { (next.title) }
                                    }
                                }
                            }
                        }
                    }

                    // --- Right-side TOC (sticky, beside article prose) ---
                    @if !numbered_headings.is_empty() {
                        @if !numbered_headings.is_empty() {
                            aside.toc {
                                div.toc__header {
                                    span.toc__title { "On this page" }
                                }
                                ol #toc-list {
                                    @for (id, text, level, _num) in &numbered_headings {
                                        li class={ "toc-level-" (level) } {
                                            a href={ "#" (id) } { (text) }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    // Phase 9 — claim-rail freshness sidebar (visible at ≥1280px)
                    @if !claim_rail_html.is_empty() {
                        (PreEscaped(claim_rail_html))
                    }
                    }
                }

                // "Edit this page" affordance — docs convention; auth-gated by CSS
                // (hidden for `html[data-auth="anon"]`). No floating Wikipedia FAB.
                div.doc-edit-row {
                    a.doc-edit-link href={ "/git/" (slug) } { "Edit this page" }
                    " · "
                    a.doc-edit-link href={ "/git/" (slug) } { "View source" }
                }

                (sovereign_footer(tenant, Some(slug)))

                // Minimal JS: TOC collapse toggle + density preference persistence.
                // Loaded last so HTML renders without it. No in-browser editor —
                // contributions flow through git (the Edit tab links to the raw
                // Markdown source at /git/{slug}).
                script src="/static/wiki.js" defer="true" {}
                script src="/static/toc-persistence.js" defer="true" {}
            }
        }
    }
}

// ─── /edit/{*slug} stub (L25: route-gated editor bundle) ────────────────────
//
// Phase 0 stub: shows the raw Markdown in a CodeMirror editor view. The
// form submission (POST /edit/{slug}) is Phase 2 Step 3 work — this handler
// renders the editor surface only, proving the bundle loads on the correct
// route. See BRIEF L25.

async fn edit_page(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> Result<Response, WikiError> {
    if slug.contains("..") || slug.is_empty() {
        return Err(WikiError::NotFound(slug));
    }

    // Read the current Markdown content; empty string if the file doesn't exist yet.
    let md_path = state.primary_path().join(format!("{slug}.md"));
    let content = fs::read_to_string(&md_path).await.unwrap_or_default();

    let page_title = format!("Edit: {slug} — {}", state.site_title);
    let edit_tenant = Tenant::from_str(&state.brand_instance);

    let markup = html! {
        (DOCTYPE)
        html lang="en" data-instance=(&state.brand_instance) {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover";
                title { (page_title) }
                link rel="preload" as="font" type="font/woff2" crossorigin href="/static/fonts/IBMPlexSans-Variable-latin.woff2";
                link rel="preload" as="font" type="font/woff2" crossorigin href="/static/fonts/PlayfairDisplay-Variable-latin.woff2";
                link rel="stylesheet" href="/static/tokens.css";
                link rel="stylesheet" href="/static/style.css";
                @if matches!(state.brand_theme.as_deref(), Some("woodfine") | Some("woodfine-projects")) {
                    link rel="stylesheet" href="/static/tokens-woodfine.css";
                }
            }
            body {
                (sovereign_nav(edit_tenant, "en", &state.site_title, "/"))
                div.edit-page-wrap {
                    nav.edit-page-crumb {
                        a href={ "/wiki/" (&slug) } { "← Back to article" }
                    }
                    h1.edit-page-title { "Edit: " (&slug) }
                    p.edit-page-note {
                        "Contributions flow through git. This editor view is a read-only "
                        "preview surface — submit changes via "
                        a href={ "/git/" (&slug) } { "git" }
                        "."
                    }
                    // L25: textarea for CodeMirror to attach to.
                    // editor.js mounts on #wikitext on DOMContentLoaded.
                    // POST removed: git-only contribution workflow (Phase 5 superseded).
                    form.edit-page-form {
                        textarea #wikitext name="content" rows="30" {
                            (content)
                        }
                    }
                }
                (sovereign_footer(edit_tenant, Some(&slug)))
                // L25: editor assets loaded only on /edit/* routes.
                script src="/static/vendor/cm-saa.bundle.js" defer="true" {}
                script src="/static/editor.js" defer="true" {}
            }
        }
    };
    Ok(markup.into_response())
}
