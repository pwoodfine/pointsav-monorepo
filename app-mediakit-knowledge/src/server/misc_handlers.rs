/// Shared shell for non-article pages (search, category, errors).
/// Replaced with sovereign chrome — single unified masthead + footer.
fn chrome(
    title: &str,
    body: Markup,
    site_title: &str,
    _user: Option<&User>,
    _pending_count: i64,
) -> Markup {
    let tenant = Tenant::from_str(
        if site_title.contains("Projects") { "projects" }
        else if site_title.contains("Woodfine") { "corporate" }
        else { "documentation" }
    );
    sovereign_page(title, tenant, "en", site_title, "/es/", body)
}

/// GET /page/:slug — static pages (Disclaimer, Contact, etc.) served on-domain
/// with the full wiki chrome so navigation stays on the same domain.
/// Tries `page-{slug}.md` in the primary content directory first; renders a
/// placeholder if the file is not present.
pub async fn page_handler(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> impl IntoResponse {
    let brand_instance = state.brand_instance.as_str();

    let primary = state.primary_path().to_path_buf();
    let md_path = primary.join(format!("page-{slug}.md"));
    let content_html: String = match fs::read_to_string(&md_path).await {
        Ok(text) => {
            match parse_page(&text) {
                Ok(parsed) => render_html_raw(&parsed.body_md, &primary, &[]),
                Err(_) => String::new(),
            }
        }
        Err(_) => {
            "<p class=\"article__lede\">This page is being prepared.</p>\
             <p>For immediate assistance, contact us at \
             <a href=\"mailto:open.source@pointsav.com\">open.source@pointsav.com</a>.</p>"
                .to_string()
        }
    };

    let display_title = page_title_for_slug(&slug);
    let site_title = &state.site_title;
    let tenant = Tenant::from_str(brand_instance);

    let content = html! {
        div class="shell" {
            article class="prose" {
                h1 { (display_title) }
                (PreEscaped(content_html))
            }
        }
    };
    let page = sovereign_page(
        &format!("{display_title} \u{2014} {site_title}"),
        tenant,
        "en",
        site_title,
        "/es/",
        content,
    );

    axum::response::Html(page.into_string())
}

fn page_title_for_slug(slug: &str) -> &'static str {
    match slug {
        "disclaimer" => "Disclaimer",
        "contact" => "Contact us",
        "privacy" => "Privacy Policy",
        "terms" => "Terms of Use",
        _ => "Page",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    async fn fixture_state() -> (AppState, tempfile::TempDir, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let state_dir = tempfile::tempdir().unwrap();
        tokio::fs::write(
            dir.path().join("topic-test.md"),
            "---\ntitle: Test Topic\n---\n# Heading\n\nbody with [[Other]] link.\n",
        )
        .await
        .unwrap();
        let index = crate::search::build_index(dir.path(), state_dir.path())
            .await
            .unwrap();
        let repo = crate::git::open_or_init(dir.path()).unwrap();
        (
            AppState {
                mounts: crate::mounts::resolve(dir.path(), None, None),
                // Use a path that does not exist; citation tests live in
                // tests/citations_test.rs where they control this path.
                // Server tests do not exercise /api/citations so the missing
                // file never triggers a load.
                citations_yaml: PathBuf::from("/nonexistent/citations.yaml"),
                search: Arc::new(index),
                git: Arc::new(Mutex::new(repo)),
                site_title: "PointSav Documentation Wiki".to_string(),
                git_tenant: "pointsav".to_string(),
                mcp_enabled: false,
                glossary: Arc::new(crate::glossary::Glossary::default()),
                links: crate::links::LinkGraph::for_testing(),
                brand_theme: None,
                brand_instance: "documentation".to_string(),
                blueprints: crate::blueprints::Registry::builtin(),
            peers: vec![],
            canonical_url: None,
            activitypub_outbox_url: None,
            start_here: vec![],
            site_categories: vec![],
            },
            dir,
            state_dir,
        )
    }

    #[tokio::test]
    async fn healthz_responds_ok() {
        let (state, _dir, _state_dir) = fixture_state().await;
        let app = router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn renders_known_page() {
        let (state, _dir, _state_dir) = fixture_state().await;
        let app = router(state);
        // Canonical clean slug (topic- prefix stripped); topic-test.md served via fallback.
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/wiki/test")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let html = std::str::from_utf8(&body).unwrap();
        assert!(html.contains("Test Topic"), "title should appear: {html}");
        assert!(
            html.contains("Heading"),
            "body heading should appear: {html}"
        );
    }

    /// /wiki/topic-test 301-redirects to the clean canonical slug /wiki/test.
    #[tokio::test]
    async fn wiki_page_topic_prefix_redirects_to_clean_slug() {
        let (state, _dir, _state_dir) = fixture_state().await;
        let app = router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/wiki/topic-test")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::MOVED_PERMANENTLY);
        assert_eq!(
            resp.headers().get("location").and_then(|v| v.to_str().ok()),
            Some("/wiki/test")
        );
    }

    #[tokio::test]
    async fn returns_404_for_unknown_page() {
        let (state, _dir, _state_dir) = fixture_state().await;
        let app = router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/wiki/does-not-exist")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn rejects_path_traversal() {
        let (state, _dir, _state_dir) = fixture_state().await;
        let app = router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/wiki/..%2Fetc%2Fpasswd")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    // Phase 1.1 chrome tests — additive; all existing tests remain unchanged.

    /// Verify the encyclopedia chrome: Article/Talk/Edit/History tabs are present.
    #[tokio::test]
    async fn wiki_page_has_encyclopedia_tabs() {
        let (state, _dir, _state_dir) = fixture_state().await;
        let app = router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/wiki/test")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let html = std::str::from_utf8(&body).unwrap();
        assert!(
            html.contains("wiki-page-tabs"),
            "encyclopedia tab strip should be emitted: {html}"
        );
        assert!(
            html.contains("View source"),
            "View source link should appear (edit row): {html}"
        );
    }

    /// Verify the encyclopedia header: doc-header present + wiki-tagline present.
    #[tokio::test]
    async fn wiki_page_has_clean_header() {
        let (state, _dir, _state_dir) = fixture_state().await;
        let app = router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/wiki/test")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let html = std::str::from_utf8(&body).unwrap();
        assert!(
            html.contains("doc-header"),
            "encyclopedia docs header should appear: {html}"
        );
        assert!(
            html.contains("wiki-tagline"),
            "Wikipedia-style tagline should render: {html}"
        );
    }

    /// Verify the Wikipedia IVC masthead band / density toggle are gone.
    #[tokio::test]
    async fn wiki_page_has_no_ivc_band() {
        let (state, _dir, _state_dir) = fixture_state().await;
        let app = router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/wiki/test")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let html = std::str::from_utf8(&body).unwrap();
        assert!(
            !html.contains("wiki-ivc-band"),
            "IVC masthead band must not render: {html}"
        );
    }

    /// Verify that the hatnote renders when the frontmatter field is present.
    #[tokio::test]
    async fn wiki_page_renders_hatnote() {
        let dir = tempfile::tempdir().unwrap();
        let state_dir = tempfile::tempdir().unwrap();
        tokio::fs::write(
            dir.path().join("with-hatnote.md"),
            "---\ntitle: Hatnote Test\nhatnote: \"See also the companion page.\"\n---\n# Body\n",
        )
        .await
        .unwrap();
        let index = crate::search::build_index(dir.path(), state_dir.path())
            .await
            .unwrap();
        let repo = crate::git::open_or_init(dir.path()).unwrap();
        let state = AppState {
            mounts: crate::mounts::resolve(dir.path(), None, None),
            citations_yaml: PathBuf::from("/nonexistent/citations.yaml"),
            search: Arc::new(index),
            git: Arc::new(Mutex::new(repo)),
            site_title: "PointSav Documentation Wiki".to_string(),
            git_tenant: "pointsav".to_string(),
            mcp_enabled: false,
            glossary: Arc::new(crate::glossary::Glossary::default()),
            links: crate::links::LinkGraph::for_testing(),
            brand_theme: None,
            brand_instance: "documentation".to_string(),
            blueprints: crate::blueprints::Registry::builtin(),
            peers: vec![],
            canonical_url: None,
            activitypub_outbox_url: None,
            start_here: vec![],
            site_categories: vec![],
        };
        let app = router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/wiki/with-hatnote")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let html = std::str::from_utf8(&body).unwrap();
        assert!(
            html.contains("wiki-hatnote"),
            "hatnote block should appear: {html}"
        );
        assert!(
            html.contains("See also the companion page."),
            "hatnote text should appear: {html}"
        );
    }

    /// Verify the Wikipedia reader density toggle is gone from the clean chrome.
    #[tokio::test]
    async fn wiki_page_has_no_density_toggle() {
        let (state, _dir, _state_dir) = fixture_state().await;
        let app = router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/wiki/test")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let html = std::str::from_utf8(&body).unwrap();
        assert!(
            !html.contains("wiki-density-toggle"),
            "density toggle must not render: {html}"
        );
    }

    /// Verify that per-section [edit] pencils appear on headings.
    #[tokio::test]
    async fn wiki_page_has_edit_pencils() {
        let dir = tempfile::tempdir().unwrap();
        let state_dir = tempfile::tempdir().unwrap();
        tokio::fs::write(
            dir.path().join("sections.md"),
            "---\ntitle: Sections\n---\n## First section\n\nText.\n\n## Second section\n\nMore.\n",
        )
        .await
        .unwrap();
        let index = crate::search::build_index(dir.path(), state_dir.path())
            .await
            .unwrap();
        let repo = crate::git::open_or_init(dir.path()).unwrap();
        let state = AppState {
            mounts: crate::mounts::resolve(dir.path(), None, None),
            citations_yaml: PathBuf::from("/nonexistent/citations.yaml"),
            search: Arc::new(index),
            git: Arc::new(Mutex::new(repo)),
            site_title: "PointSav Documentation Wiki".to_string(),
            git_tenant: "pointsav".to_string(),
            mcp_enabled: false,
            glossary: Arc::new(crate::glossary::Glossary::default()),
            links: crate::links::LinkGraph::for_testing(),
            brand_theme: None,
            brand_instance: "documentation".to_string(),
            blueprints: crate::blueprints::Registry::builtin(),
            peers: vec![],
            canonical_url: None,
            activitypub_outbox_url: None,
            start_here: vec![],
            site_categories: vec![],
        };
        let app = router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/wiki/sections")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let html = std::str::from_utf8(&body).unwrap();
        assert!(
            html.contains("edit-pencil"),
            "edit pencil class should appear on headings: {html}"
        );
        assert!(
            html.contains("Edit this section"),
            "edit pencil title should appear: {html}"
        );
    }

    /// Verify categories render in the article footer when present.
    #[tokio::test]
    async fn wiki_page_renders_categories() {
        let dir = tempfile::tempdir().unwrap();
        let state_dir = tempfile::tempdir().unwrap();
        tokio::fs::write(
            dir.path().join("cats.md"),
            "---\ntitle: Cats\ncategories:\n  - Alpha\n  - Beta\n---\n# Body\n",
        )
        .await
        .unwrap();
        let index = crate::search::build_index(dir.path(), state_dir.path())
            .await
            .unwrap();
        let repo = crate::git::open_or_init(dir.path()).unwrap();
        let state = AppState {
            mounts: crate::mounts::resolve(dir.path(), None, None),
            citations_yaml: PathBuf::from("/nonexistent/citations.yaml"),
            search: Arc::new(index),
            git: Arc::new(Mutex::new(repo)),
            site_title: "PointSav Documentation Wiki".to_string(),
            git_tenant: "pointsav".to_string(),
            mcp_enabled: false,
            glossary: Arc::new(crate::glossary::Glossary::default()),
            links: crate::links::LinkGraph::for_testing(),
            brand_theme: None,
            brand_instance: "documentation".to_string(),
            blueprints: crate::blueprints::Registry::builtin(),
            peers: vec![],
            canonical_url: None,
            activitypub_outbox_url: None,
            start_here: vec![],
            site_categories: vec![],
        };
        let app = router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/wiki/cats")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let html = std::str::from_utf8(&body).unwrap();
        assert!(
            html.contains("Alpha"),
            "category Alpha should appear: {html}"
        );
        assert!(html.contains("Beta"), "category Beta should appear: {html}");
        assert!(
            html.contains("wiki-categories"),
            "categories block should appear: {html}"
        );
    }

    // Iteration-2 tests — additive; all existing tests remain unchanged.

    /// Verify that `short_description` renders as italic subtitle below the H1.
    #[tokio::test]
    async fn wiki_page_renders_short_description() {
        let dir = tempfile::tempdir().unwrap();
        let state_dir = tempfile::tempdir().unwrap();
        tokio::fs::write(
            dir.path().join("described.md"),
            "---\ntitle: Described Topic\nshort_description: \"One-sentence summary here.\"\n---\nBody content.\n",
        )
        .await
        .unwrap();
        let index = crate::search::build_index(dir.path(), state_dir.path())
            .await
            .unwrap();
        let repo = crate::git::open_or_init(dir.path()).unwrap();
        let state = AppState {
            mounts: crate::mounts::resolve(dir.path(), None, None),
            citations_yaml: PathBuf::from("/nonexistent/citations.yaml"),
            search: Arc::new(index),
            git: Arc::new(Mutex::new(repo)),
            site_title: "PointSav Documentation Wiki".to_string(),
            git_tenant: "pointsav".to_string(),
            mcp_enabled: false,
            glossary: Arc::new(crate::glossary::Glossary::default()),
            links: crate::links::LinkGraph::for_testing(),
            brand_theme: None,
            brand_instance: "documentation".to_string(),
            blueprints: crate::blueprints::Registry::builtin(),
            peers: vec![],
            canonical_url: None,
            activitypub_outbox_url: None,
            start_here: vec![],
            site_categories: vec![],
        };
        let app = router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/wiki/described")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let html = std::str::from_utf8(&body).unwrap();
        assert!(
            html.contains("article__lede"),
            "short_description container class should appear: {html}"
        );
        assert!(
            html.contains("One-sentence summary here."),
            "short_description text should appear: {html}"
        );
    }

    /// Verify that the encyclopedia tab strip and category sidenav render on article pages.
    /// Sprint C restored the docs-sidenav (category navigation) alongside the
    /// wiki-page-tabs strip — Article/Talk/Edit/History.
    #[tokio::test]
    async fn wiki_page_renders_navigation_portlet() {
        let dir = tempfile::tempdir().unwrap();
        let state_dir = tempfile::tempdir().unwrap();
        tokio::fs::write(
            dir.path().join("nav-portlet-test.md"),
            "---\ntitle: Nav Portlet Test\ncategory: architecture\n---\nBody.\n",
        )
        .await
        .unwrap();
        let index = crate::search::build_index(dir.path(), state_dir.path())
            .await
            .unwrap();
        let repo = crate::git::open_or_init(dir.path()).unwrap();
        let state = AppState {
            mounts: crate::mounts::resolve(dir.path(), None, None),
            citations_yaml: PathBuf::from("/nonexistent/citations.yaml"),
            search: Arc::new(index),
            git: Arc::new(Mutex::new(repo)),
            site_title: "PointSav Documentation Wiki".to_string(),
            git_tenant: "pointsav".to_string(),
            mcp_enabled: false,
            glossary: Arc::new(crate::glossary::Glossary::default()),
            links: crate::links::LinkGraph::for_testing(),
            brand_theme: None,
            brand_instance: "documentation".to_string(),
            blueprints: crate::blueprints::Registry::builtin(),
            peers: vec![],
            canonical_url: None,
            activitypub_outbox_url: None,
            start_here: vec![],
            site_categories: vec![],
        };
        let app = router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/wiki/nav-portlet-test")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let html = std::str::from_utf8(&body).unwrap();
        assert!(
            html.contains("wiki-page-tabs"),
            "encyclopedia tab strip should appear on article pages: {html}"
        );
        assert!(
            html.contains("docs-sidenav"),
            "category sidenav must appear on article pages (restored in Sprint C): {html}"
        );
    }

    /// Verify that a TOPIC in a subdirectory is reachable via the `/wiki/<cat>/<slug>` path.
    #[tokio::test]
    async fn wiki_page_resolves_subdirectory_slug() {
        let dir = tempfile::tempdir().unwrap();
        let state_dir = tempfile::tempdir().unwrap();
        // Create architecture/ subdirectory with one TOPIC.
        tokio::fs::create_dir_all(dir.path().join("architecture"))
            .await
            .unwrap();
        tokio::fs::write(
            dir.path().join("architecture/compounding-substrate.md"),
            "---\ntitle: The Compounding Substrate\ncategory: architecture\n---\nSubstrate body.\n",
        )
        .await
        .unwrap();
        let index = crate::search::build_index(dir.path(), state_dir.path())
            .await
            .unwrap();
        let repo = crate::git::open_or_init(dir.path()).unwrap();
        let state = AppState {
            mounts: crate::mounts::resolve(dir.path(), None, None),
            citations_yaml: PathBuf::from("/nonexistent/citations.yaml"),
            search: Arc::new(index),
            git: Arc::new(Mutex::new(repo)),
            site_title: "PointSav Documentation Wiki".to_string(),
            git_tenant: "pointsav".to_string(),
            mcp_enabled: false,
            glossary: Arc::new(crate::glossary::Glossary::default()),
            links: crate::links::LinkGraph::for_testing(),
            brand_theme: None,
            brand_instance: "documentation".to_string(),
            blueprints: crate::blueprints::Registry::builtin(),
            peers: vec![],
            canonical_url: None,
            activitypub_outbox_url: None,
            start_here: vec![],
            site_categories: vec![],
        };
        let app = router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/wiki/architecture/compounding-substrate")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "subdirectory TOPIC should resolve"
        );
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let html = std::str::from_utf8(&body).unwrap();
        assert!(
            html.contains("The Compounding Substrate"),
            "title from frontmatter should appear: {html}"
        );
    }

    /// Verify that a bare slug (`/wiki/compounding-substrate`) 301-redirects to
    /// the path-qualified slug (`/wiki/architecture/compounding-substrate`) when
    /// the file lives in a category subdirectory.
    #[tokio::test]
    async fn wiki_page_bare_slug_redirects_to_qualified() {
        let dir = tempfile::tempdir().unwrap();
        let state_dir = tempfile::tempdir().unwrap();
        tokio::fs::create_dir_all(dir.path().join("architecture"))
            .await
            .unwrap();
        tokio::fs::write(
            dir.path().join("architecture/bare-slug-test.md"),
            "---\ntitle: Bare Slug Test\ncategory: architecture\n---\nBody.\n",
        )
        .await
        .unwrap();
        let index = crate::search::build_index(dir.path(), state_dir.path())
            .await
            .unwrap();
        let repo = crate::git::open_or_init(dir.path()).unwrap();
        let state = AppState {
            mounts: crate::mounts::resolve(dir.path(), None, None),
            citations_yaml: PathBuf::from("/nonexistent/citations.yaml"),
            search: Arc::new(index),
            git: Arc::new(Mutex::new(repo)),
            site_title: "Test Wiki".to_string(),
            git_tenant: "pointsav".to_string(),
            mcp_enabled: false,
            glossary: Arc::new(crate::glossary::Glossary::default()),
            links: crate::links::LinkGraph::for_testing(),
            brand_theme: None,
            brand_instance: "documentation".to_string(),
            blueprints: crate::blueprints::Registry::builtin(),
            peers: vec![],
            canonical_url: None,
            activitypub_outbox_url: None,
            start_here: vec![],
            site_categories: vec![],
        };
        let app = router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/wiki/bare-slug-test")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::MOVED_PERMANENTLY,
            "bare slug should 301 redirect to path-qualified form"
        );
        let location = resp.headers().get("location").unwrap().to_str().unwrap();
        assert_eq!(
            location, "/wiki/architecture/bare-slug-test",
            "redirect location should be the path-qualified slug"
        );
    }

    /// Verify that subdirectory TOPICs appear in the home-page category grid.
    #[tokio::test]
    async fn home_page_buckets_subdirectory_topics() {
        let dir = tempfile::tempdir().unwrap();
        let state_dir = tempfile::tempdir().unwrap();
        // index.md required for home_chrome path.
        tokio::fs::write(
            dir.path().join("index.md"),
            "---\ntitle: Home\ncategory: root\n---\nWelcome.\n",
        )
        .await
        .unwrap();
        // Architecture subdirectory with one TOPIC.
        tokio::fs::create_dir_all(dir.path().join("architecture"))
            .await
            .unwrap();
        tokio::fs::write(
            dir.path().join("architecture/my-article.md"),
            "---\ntitle: My Article\ncategory: architecture\n---\nContent here.\n",
        )
        .await
        .unwrap();
        let index = crate::search::build_index(dir.path(), state_dir.path())
            .await
            .unwrap();
        let repo = crate::git::open_or_init(dir.path()).unwrap();
        let state = AppState {
            mounts: crate::mounts::resolve(dir.path(), None, None),
            citations_yaml: PathBuf::from("/nonexistent/citations.yaml"),
            search: Arc::new(index),
            git: Arc::new(Mutex::new(repo)),
            site_title: "PointSav Documentation Wiki".to_string(),
            git_tenant: "pointsav".to_string(),
            mcp_enabled: false,
            glossary: Arc::new(crate::glossary::Glossary::default()),
            links: crate::links::LinkGraph::for_testing(),
            brand_theme: None,
            brand_instance: "documentation".to_string(),
            blueprints: crate::blueprints::Registry::builtin(),
            peers: vec![],
            canonical_url: None,
            activitypub_outbox_url: None,
            start_here: vec![],
            site_categories: vec![],
        };
        let app = router(state);
        let resp = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let html = std::str::from_utf8(&body).unwrap();
        // The article title should appear in the category grid.
        assert!(
            html.contains("My Article"),
            "subdirectory TOPIC title should appear in category grid: {html}"
        );
        // The Architecture category should show at least 1 article.
        assert!(
            html.contains("Architecture"),
            "Architecture category header should appear: {html}"
        );
    }

    // Iteration-2 Item 11 tests — language toggle auto-detection.

    /// EN article with a `.es.md` sibling gets an ES toggle auto-injected.
    #[tokio::test]
    async fn wiki_page_auto_detects_es_sibling() {
        let dir = tempfile::tempdir().unwrap();
        let state_dir = tempfile::tempdir().unwrap();
        // EN article
        tokio::fs::write(
            dir.path().join("my-topic.md"),
            "---\ntitle: My Topic\ncategory: architecture\n---\nEN content.\n",
        )
        .await
        .unwrap();
        // ES sibling
        tokio::fs::write(
            dir.path().join("my-topic.es.md"),
            "---\ntitle: Mi Tema\ncategory: architecture\n---\nContenido ES.\n",
        )
        .await
        .unwrap();
        let index = crate::search::build_index(dir.path(), state_dir.path())
            .await
            .unwrap();
        let repo = crate::git::open_or_init(dir.path()).unwrap();
        let state = AppState {
            mounts: crate::mounts::resolve(dir.path(), None, None),
            citations_yaml: PathBuf::from("/nonexistent/citations.yaml"),
            search: Arc::new(index),
            git: Arc::new(Mutex::new(repo)),
            site_title: "PointSav Documentation Wiki".to_string(),
            git_tenant: "pointsav".to_string(),
            mcp_enabled: false,
            glossary: Arc::new(crate::glossary::Glossary::default()),
            links: crate::links::LinkGraph::for_testing(),
            brand_theme: None,
            brand_instance: "documentation".to_string(),
            blueprints: crate::blueprints::Registry::builtin(),
            peers: vec![],
            canonical_url: None,
            activitypub_outbox_url: None,
            start_here: vec![],
            site_categories: vec![],
        };
        let app = router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/wiki/my-topic")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let html = std::str::from_utf8(&body).unwrap();
        // Should show ES toggle
        assert!(
            html.contains("wiki-lang-switcher"),
            "language switcher should appear when .es.md sibling exists: {html}"
        );
        assert!(
            html.contains("/wiki/my-topic.es"),
            "ES sibling link should appear in language switcher: {html}"
        );
    }

    /// ES article auto-gets an EN link back to the base slug.
    #[tokio::test]
    async fn wiki_page_es_article_gets_en_toggle() {
        let dir = tempfile::tempdir().unwrap();
        let state_dir = tempfile::tempdir().unwrap();
        // EN base article
        tokio::fs::write(
            dir.path().join("my-topic.md"),
            "---\ntitle: My Topic\ncategory: architecture\n---\nEN content.\n",
        )
        .await
        .unwrap();
        // ES sibling
        tokio::fs::write(
            dir.path().join("my-topic.es.md"),
            "---\ntitle: Mi Tema\ncategory: architecture\n---\nContenido ES.\n",
        )
        .await
        .unwrap();
        let index = crate::search::build_index(dir.path(), state_dir.path())
            .await
            .unwrap();
        let repo = crate::git::open_or_init(dir.path()).unwrap();
        let state = AppState {
            mounts: crate::mounts::resolve(dir.path(), None, None),
            citations_yaml: PathBuf::from("/nonexistent/citations.yaml"),
            search: Arc::new(index),
            git: Arc::new(Mutex::new(repo)),
            site_title: "PointSav Documentation Wiki".to_string(),
            git_tenant: "pointsav".to_string(),
            mcp_enabled: false,
            glossary: Arc::new(crate::glossary::Glossary::default()),
            links: crate::links::LinkGraph::for_testing(),
            brand_theme: None,
            brand_instance: "documentation".to_string(),
            blueprints: crate::blueprints::Registry::builtin(),
            peers: vec![],
            canonical_url: None,
            activitypub_outbox_url: None,
            start_here: vec![],
            site_categories: vec![],
        };
        let app = router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/wiki/my-topic.es")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let html = std::str::from_utf8(&body).unwrap();
        // ES article should show EN toggle back to base
        assert!(
            html.contains("wiki-lang-switcher"),
            "language switcher should appear on ES article: {html}"
        );
        assert!(
            html.contains("/wiki/my-topic\""),
            "EN base link should appear in language switcher on ES article: {html}"
        );
    }

    /// EN article WITHOUT an ES sibling should NOT show the language switcher.
    #[tokio::test]
    async fn wiki_page_no_toggle_when_sibling_absent() {
        let dir = tempfile::tempdir().unwrap();
        let state_dir = tempfile::tempdir().unwrap();
        tokio::fs::write(
            dir.path().join("solo-topic.md"),
            "---\ntitle: Solo Topic\ncategory: architecture\n---\nBody only.\n",
        )
        .await
        .unwrap();
        // No .es.md sibling written.
        let index = crate::search::build_index(dir.path(), state_dir.path())
            .await
            .unwrap();
        let repo = crate::git::open_or_init(dir.path()).unwrap();
        let state = AppState {
            mounts: crate::mounts::resolve(dir.path(), None, None),
            citations_yaml: PathBuf::from("/nonexistent/citations.yaml"),
            search: Arc::new(index),
            git: Arc::new(Mutex::new(repo)),
            site_title: "PointSav Documentation Wiki".to_string(),
            git_tenant: "pointsav".to_string(),
            mcp_enabled: false,
            glossary: Arc::new(crate::glossary::Glossary::default()),
            links: crate::links::LinkGraph::for_testing(),
            brand_theme: None,
            brand_instance: "documentation".to_string(),
            blueprints: crate::blueprints::Registry::builtin(),
            peers: vec![],
            canonical_url: None,
            activitypub_outbox_url: None,
            start_here: vec![],
            site_categories: vec![],
        };
        let app = router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/wiki/solo-topic")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let html = std::str::from_utf8(&body).unwrap();
        assert!(
            !html.contains("wiki-lang-switcher"),
            "language switcher should NOT appear when no sibling exists: {html}"
        );
    }

    /// Accept: application/json returns a JSON object with the expected keys.
    #[tokio::test]
    async fn wiki_page_json_content_negotiation_returns_json() {
        let (state, _dir, _state_dir) = fixture_state().await;
        let app = router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/wiki/test")
                    .header("accept", "application/json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let ct = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(
            ct.contains("application/json"),
            "content-type should be JSON: {ct}"
        );
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let val: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(val.get("frontmatter").is_some(), "missing frontmatter key");
        assert!(val.get("body_md").is_some(), "missing body_md key");
        assert!(val.get("blake3").is_some(), "missing blake3 key");
        assert!(
            val.get("revision_sha").is_some(),
            "missing revision_sha key"
        );
        assert!(val.get("backlinks").is_some(), "missing backlinks key");
        assert!(val.get("claims").is_some(), "missing claims key");
        assert_eq!(val["frontmatter"]["title"], "Test Topic");
    }

    /// ?asof= with an unknown revision returns 404. The test content dir is
    /// an empty git repo (no commits), so any SHA is unknown.
    #[tokio::test]
    async fn wiki_page_asof_unknown_revision_returns_404() {
        let (state, _dir, _state_dir) = fixture_state().await;
        let app = router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/wiki/test?asof=deadbeefdeadbeef")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    // ── Phase 5.1: bilingual /es/ routing tests ───────────────────────────────

    /// /es/ serves index.es.md with lang="es" when the ES index exists.
    #[tokio::test]
    async fn home_es_serves_es_index_when_present() {
        let (state, dir, _state_dir) = fixture_state().await;
        tokio::fs::write(
            dir.path().join("index.md"),
            "---\ntitle: Home EN\n---\nEnglish home content.\n",
        )
        .await
        .unwrap();
        tokio::fs::write(
            dir.path().join("index.es.md"),
            "---\ntitle: Inicio\n---\nContenido en español.\n",
        )
        .await
        .unwrap();
        let app = router(state);
        let resp = app
            .oneshot(Request::builder().uri("/es/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let html = std::str::from_utf8(&body).unwrap();
        assert!(html.contains(r#"lang="es""#), "should have lang=es: {html}");
        assert!(
            html.contains("Contenido en español"),
            "should serve ES content: {html}"
        );
    }

    /// /es/ falls back to index.md (returning 200) when index.es.md is absent.
    #[tokio::test]
    async fn home_es_falls_back_to_en_when_no_es_index() {
        let (state, dir, _state_dir) = fixture_state().await;
        tokio::fs::write(
            dir.path().join("index.md"),
            "---\ntitle: Home EN\n---\nEnglish home content.\n",
        )
        .await
        .unwrap();
        let app = router(state);
        let resp = app
            .oneshot(Request::builder().uri("/es/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let html = std::str::from_utf8(&body).unwrap();
        assert!(
            html.contains("English home content"),
            "fallback should serve EN content: {html}"
        );
    }

    /// /es/wiki/{slug} serves the .es.md file with lang="es" when it exists.
    #[tokio::test]
    async fn wiki_page_es_serves_es_article_when_present() {
        let (state, dir, _state_dir) = fixture_state().await;
        tokio::fs::write(
            dir.path().join("topic-test.es.md"),
            "---\ntitle: Tema de Prueba\n---\n# Encabezado\n\nContenido en español.\n",
        )
        .await
        .unwrap();
        let app = router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/es/wiki/test")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let html = std::str::from_utf8(&body).unwrap();
        assert!(html.contains(r#"lang="es""#), "should have lang=es: {html}");
        assert!(
            html.contains("Encabezado"),
            "should serve ES body content: {html}"
        );
    }

    /// /es/wiki/{slug} falls back to the EN article (200, lang="en") when
    /// no .es.md sibling exists.
    #[tokio::test]
    async fn wiki_page_es_falls_back_to_en_when_no_es_article() {
        let (state, _dir, _state_dir) = fixture_state().await;
        let app = router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/es/wiki/test")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let html = std::str::from_utf8(&body).unwrap();
        assert!(
            html.contains(r#"lang="en""#),
            "fallback should have lang=en: {html}"
        );
        assert!(
            html.contains("Test Topic"),
            "fallback should serve EN content: {html}"
        );
    }

    /// /es/wiki/{slug} returns 404 when the slug exists in neither locale.
    #[tokio::test]
    async fn wiki_page_es_returns_404_for_unknown_slug() {
        let (state, _dir, _state_dir) = fixture_state().await;
        let app = router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/es/wiki/does-not-exist")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    /// The EN home page nav contains a link to /es/.
    #[tokio::test]
    async fn home_has_lang_toggle_to_es() {
        let (state, dir, _state_dir) = fixture_state().await;
        tokio::fs::write(
            dir.path().join("index.md"),
            "---\ntitle: Home\n---\nHome content.\n",
        )
        .await
        .unwrap();
        let app = router(state);
        let resp = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let html = std::str::from_utf8(&body).unwrap();
        assert!(
            html.contains(r#"href="/es/""#),
            "EN home nav should link to /es/: {html}"
        );
    }

    /// The ES article page nav contains a link back to the EN article.
    #[tokio::test]
    async fn wiki_page_es_has_lang_toggle_to_en() {
        let (state, dir, _state_dir) = fixture_state().await;
        tokio::fs::write(
            dir.path().join("topic-test.es.md"),
            "---\ntitle: Tema de Prueba\n---\nContenido en español.\n",
        )
        .await
        .unwrap();
        let app = router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/es/wiki/test")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let html = std::str::from_utf8(&body).unwrap();
        assert!(
            html.contains(r#"href="/wiki/test""#),
            "ES article nav should link to EN article at clean slug: {html}"
        );
    }

    /// The ES article page head contains hreflang="en" and rel="canonical" tags.
    #[tokio::test]
    async fn wiki_page_es_has_hreflang_tags() {
        let (state, dir, _state_dir) = fixture_state().await;
        tokio::fs::write(
            dir.path().join("topic-test.es.md"),
            "---\ntitle: Tema de Prueba\n---\nContenido en español.\n",
        )
        .await
        .unwrap();
        let app = router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/es/wiki/test")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let html = std::str::from_utf8(&body).unwrap();
        assert!(
            html.contains(r#"hreflang="en""#),
            "ES article head should have hreflang=en: {html}"
        );
        assert!(
            html.contains(r#"rel="canonical""#),
            "ES article head should have canonical link: {html}"
        );
    }

    /// `GET /` with `Accept-Language: es` redirects to `/es/`.
    #[tokio::test]
    async fn index_redirects_to_es_on_accept_language() {
        let (state, dir, _state_dir) = fixture_state().await;
        tokio::fs::write(
            dir.path().join("index.md"),
            "---\ntitle: Home\n---\nHome content.\n",
        )
        .await
        .unwrap();
        let app = router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header("Accept-Language", "es,en;q=0.8")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::FOUND,
            "Accept-Language: es should redirect to /es/"
        );
        assert_eq!(
            resp.headers().get("location").and_then(|v| v.to_str().ok()),
            Some("/es/")
        );
    }

    /// `GET /?noredirect=1` with `Accept-Language: es` serves EN home (no redirect).
    #[tokio::test]
    async fn index_noredirect_suppresses_accept_language_redirect() {
        let (state, dir, _state_dir) = fixture_state().await;
        tokio::fs::write(
            dir.path().join("index.md"),
            "---\ntitle: Home\n---\nHome content.\n",
        )
        .await
        .unwrap();
        let app = router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/?noredirect=1")
                    .header("Accept-Language", "es,en;q=0.8")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "noredirect=1 should suppress Accept-Language redirect"
        );
    }

    /// `GET /` with no Accept-Language (or EN preference) serves EN home directly.
    #[tokio::test]
    async fn index_no_accept_language_serves_en() {
        let (state, dir, _state_dir) = fixture_state().await;
        tokio::fs::write(
            dir.path().join("index.md"),
            "---\ntitle: Home\n---\nHome content.\n",
        )
        .await
        .unwrap();
        let app = router(state);
        let resp = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "no Accept-Language should serve EN 200"
        );
    }

    /// ES home lang-toggle links to `/?noredirect=1` to prevent redirect loop.
    #[tokio::test]
    async fn home_es_lang_toggle_links_to_en_with_noredirect() {
        let (state, dir, _state_dir) = fixture_state().await;
        tokio::fs::write(
            dir.path().join("index.es.md"),
            "---\ntitle: Inicio\n---\nContenido.\n",
        )
        .await
        .unwrap();
        let app = router(state);
        let resp = app
            .oneshot(Request::builder().uri("/es/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let html = std::str::from_utf8(&body).unwrap();
        assert!(
            html.contains(r#"href="/?noredirect=1""#),
            "ES home nav should link to /?noredirect=1 to prevent redirect loop: {html}"
        );
    }

    // ── /images/ route tests ──────────────────────────────────────────────────

    /// /images/{path} serves a file from <content_dir>/images/.
    #[tokio::test]
    async fn images_route_serves_existing_file() {
        let (state, dir, _state_dir) = fixture_state().await;
        tokio::fs::create_dir(dir.path().join("images")).await.unwrap();
        tokio::fs::write(dir.path().join("images/test.png"), b"\x89PNG\r\n").await.unwrap();
        let app = router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/images/test.png")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let ct = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(ct.contains("image/png"), "content-type should be image/png: {ct}");
    }

    /// /images/{path} returns 404 for a file that does not exist.
    #[tokio::test]
    async fn images_route_returns_404_for_missing_file() {
        let (state, _dir, _state_dir) = fixture_state().await;
        let app = router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/images/missing.png")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    /// Path traversal via /images/../<anything> is rejected with 404.
    #[tokio::test]
    async fn images_route_rejects_path_traversal() {
        let (state, _dir, _state_dir) = fixture_state().await;
        let app = router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/images/..%2Fetc%2Fpasswd")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
