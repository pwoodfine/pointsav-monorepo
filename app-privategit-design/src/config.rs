// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

use std::{collections::HashMap, env, path::PathBuf};

pub struct Config {
    pub vault: PathBuf,
    pub bind: String,
    pub doorman_url: String,
    pub tenant: String,
    /// DESIGN-BUNDLE mounts: bundle name -> canonical source directory owned by
    /// another archive (mounted read-only for serving + /download; never copied).
    pub bundle_mounts: HashMap<String, PathBuf>,
    /// minijinja template directory. Defaults to CARGO_MANIFEST_DIR/templates for
    /// local dev convenience — that default is a compile-time path baked into the
    /// binary and does NOT exist once the binary is copied to another machine
    /// (found 2026-07-03: caused a production crash-loop on foundry-prod,
    /// "nav.html missing", since the binary is built on the workspace VM but
    /// runs on a separate deploy target). Production deploys MUST set
    /// DESIGN_TEMPLATES_DIR explicitly.
    pub templates_dir: PathBuf,
    /// Static assets (CSS/JS) served at /static/*. Same CARGO_MANIFEST_DIR-baked-path
    /// issue as templates_dir — found 2026-07-03 as a second, silent instance of the
    /// same bug (ServeDir 404s instead of panicking, so it didn't surface in the
    /// initial fix's smoke test). Production deploys MUST set DESIGN_STATIC_DIR.
    pub static_dir: PathBuf,
    /// SEO — canonical/OG/JSON-LD/sitemap base origin. Defaults to a plain
    /// http://<bind> URL (127.0.0.1:9094 by default) rather than hardcoding the
    /// production domain, so a local/staging instance never emits production
    /// canonical URLs for itself — the same class of bug templates_dir/static_dir
    /// already hit (a compile-time/hardcoded value silently wrong on another
    /// deployment). Only the promoted foundry-prod deploy should set
    /// DESIGN_SITE_ORIGIN=https://design.pointsav.com.
    pub site_origin: String,
}

impl Config {
    pub fn from_env() -> Self {
        let vault = PathBuf::from(
            env::var("DESIGN_VAULT_DIR")
                .or_else(|_| env::var("DESIGN_VAULT"))
                .unwrap_or_else(|_| {
                    "/srv/foundry/deployments/vault-privategit-design-1".to_string()
                }),
        );

        let mut bundle_mounts = HashMap::new();
        bundle_mounts.insert(
            "editorial-style-guide".to_string(),
            PathBuf::from(env::var("BUNDLE_MOUNT_EDITORIAL_STYLE_GUIDE").unwrap_or_else(|_| {
                "/srv/foundry/clones/project-editorial/media-knowledge-documentation/.internal/style-guides".to_string()
            })),
        );
        // P1.11 — "Get started / Download tokens" front door: the compiled DTCG bundle
        // exports (tokens.css, tokens.full.json), not markdown drafts.
        bundle_mounts.insert(
            "tokens".to_string(),
            PathBuf::from(
                env::var("BUNDLE_MOUNT_TOKENS")
                    .unwrap_or_else(|_| vault.join("exports").to_string_lossy().into_owned()),
            ),
        );

        let templates_dir = PathBuf::from(env::var("DESIGN_TEMPLATES_DIR").unwrap_or_else(|_| {
            concat!(env!("CARGO_MANIFEST_DIR"), "/templates").to_string()
        }));
        let static_dir = PathBuf::from(env::var("DESIGN_STATIC_DIR").unwrap_or_else(|_| {
            concat!(env!("CARGO_MANIFEST_DIR"), "/static").to_string()
        }));

        let bind = env::var("DESIGN_BIND").unwrap_or_else(|_| "127.0.0.1:9094".to_string());
        let site_origin = env::var("DESIGN_SITE_ORIGIN").unwrap_or_else(|_| format!("http://{bind}"));

        Config {
            vault,
            bind,
            doorman_url: env::var("DOORMAN_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:9092".to_string()),
            tenant: env::var("DESIGN_TENANT").unwrap_or_else(|_| "pointsav".to_string()),
            bundle_mounts,
            templates_dir,
            static_dir,
            site_origin,
        }
    }
}
