// SPDX-License-Identifier: FSL-1.1-ALv2
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

//! Validates every real, shipped content file — not just test fixtures.
//!
//! Added 2026-07-02 after a real bug reached a shadow deploy undetected: an
//! editing mistake left an orphaned YAML fragment in `home/page.yaml` that
//! broke parsing, but nothing in the test suite loaded the actual shipped
//! content, so `cargo test` stayed green while the live page 500'd. Content
//! is loaded from disk at runtime (by design — the manifest is the single
//! source of truth, see `content.rs`), so it is never validated by the
//! compiler; this test is the only thing that will catch a bad edit here.

use app_mediakit_marketing_2::content;
use std::path::Path;

fn content_root() -> std::path::PathBuf {
    // This crate lives at .../pointsav-monorepo/app-mediakit-marketing-2/;
    // the real content ships in the sibling (retired) crate's directory,
    // per DESIGN-SYSTEM.md — `-2` reads the same content trees so both
    // engines can run side by side during the rewrite.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("app-mediakit-marketing")
}

fn assert_all_pages_load(content_dir: &Path, module_id: &str) {
    assert!(
        content_dir.is_dir(),
        "content dir missing: {}",
        content_dir.display()
    );
    let slugs = content::list_slugs(content_dir);
    assert!(
        !slugs.is_empty(),
        "no pages found under {} — list_slugs regression?",
        content_dir.display()
    );

    for slug in &slugs {
        let page = content::load_page(content_dir, slug, None).unwrap_or_else(|e| {
            panic!("{module_id}/{slug} (EN) failed to parse: {e}");
        });
        let markup = app_mediakit_marketing_2::ui::page_shell(
            &app_mediakit_marketing_2::ui::Tenant::by_module_id(module_id),
            &page,
            module_id,
            "/",
            Some("/es"),
            None,
        );
        assert!(markup.into_string().contains("<!DOCTYPE html>"));

        // Spanish variant is optional per-page, but if the file exists it
        // must parse and render too.
        if content_dir.join(slug).join("page.es.yaml").is_file() {
            let page_es = content::load_page(content_dir, slug, Some("es"))
                .unwrap_or_else(|e| panic!("{module_id}/{slug} (ES) failed to parse: {e}"));
            assert_eq!(page_es.lang, "es");
        }
    }
}

#[test]
fn all_real_woodfine_pages_parse_and_render() {
    assert_all_pages_load(&content_root().join("content"), "woodfine");
}

#[test]
fn all_real_pointsav_pages_parse_and_render() {
    assert_all_pages_load(&content_root().join("content-pointsav"), "pointsav");
}
