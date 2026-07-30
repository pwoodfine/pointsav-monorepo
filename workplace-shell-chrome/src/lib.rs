//! `workplace-shell-chrome` — shared backend substrate for the
//! `app-workplace-*` Tauri app family.
//!
//! ## Why this crate exists
//!
//! `app-workplace-workbench` and `app-workplace-gis` independently
//! reinvented an identical pattern: a JSON config file in the app's data
//! directory, holding one endpoint/port value, with a `get_X` / `set_X` /
//! `has_X_config` IPC command triad wrapping it — same shape, two
//! hand-written implementations, zero shared code. See
//! `DESIGN-RESEARCH-workplace-shell-chrome.draft.md` (finding 1) and
//! `BRIEF-workplace-institutional-quality-roadmap.md` (Track B) for the full
//! duplication evidence and roadmap context.
//!
//! This crate factors the duplicated *logic* (read/parse-with-fallback,
//! create-dir-and-write, existence-check) into one place. It deliberately
//! does **not** wrap `#[tauri::command]` itself — Tauri's command macro
//! expects to see the function at the call site of `generate_handler!`, so
//! each app keeps a thin (3-8 line) command wrapper in its own `main.rs`
//! that delegates to these plain functions. That thinness is the point: the
//! part worth sharing (file I/O + serde + fallback semantics) is shared: the
//! part that must stay per-app (Tauri's own macro plumbing, and each app's
//! own config shape) stays per-app.
//!
//! ## Verification status (2026-07-14, honest record — do not silently drop)
//!
//! This crate itself has **zero** dependency on `tauri`/`wry`/`webkit2gtk`/
//! `soup2-sys` — only `serde` + `serde_json` + `std`. `cargo check` and
//! `cargo test` against this crate **standalone** exercise real compiled
//! and executed Rust code.
//!
//! The two consumer crates (`app-workplace-workbench/src-tauri`,
//! `app-workplace-gis/src-tauri`) currently **cannot** reach a full
//! `cargo check` pass on this host, for a pre-existing reason unrelated to
//! this crate or to the retrofit that wires it in: their pinned
//! `tauri = "1.7"` stack resolves to `wry 0.24` / `webkit2gtk 0.18`, which
//! pulls in `soup2-sys`, which requires `libsoup-2.4` via `pkg-config` —
//! not installed on this Ubuntu 24.04 host (only `libsoup-3.0-dev` is
//! present; `libsoup2.4-dev` is available via apt but not yet installed,
//! ask-first, flagged repeatedly this run — see
//! `BRIEF-workplace-institutional-quality-roadmap.md` Phase 1 Work Log and
//! Track A). That build-script failure happens during dependency
//! resolution, **before** either consumer crate's own `src/` — including
//! the retrofit made here — is ever reached by the compiler. So a
//! `cargo check` run in either consumer directory fails with the identical
//! `soup2-sys` error whether or not this retrofit is correct, and cannot be
//! used to verify the retrofit's own correctness.
//!
//! Given that constraint, correctness for the consumer-side change is
//! instead established by: (1) this crate's own `cargo test` passing
//! (exercises the exact read/write/fallback/existence code path each
//! consumer now calls); (2) a byte-for-byte behavior-preservation review of
//! each consumer's retrofitted `main.rs` against its pre-retrofit version
//! (same IPC command names/signatures, same config filenames, same default
//! values, same error strings); (3) `rustc --edition 2021 --crate-type lib
//! --emit=metadata` run directly against each consumer's `main.rs` is not
//! possible (it needs the `tauri` crate/macros to resolve `#[tauri::command]`
//! and `tauri::AppHandle`), so no isolated syntax-only check of the consumer
//! files beyond careful manual review was available in this session.
//! Re-running `cargo check` in both consumer directories after
//! `libsoup2.4-dev` is installed (Track A) is the outstanding step that
//! would give full end-to-end verification; it is explicitly **not** done
//! in this session (ask-first `apt-get install`, no operator present).

use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Resolve the on-disk path for a named JSON config file inside an app's
/// data directory. Pure path arithmetic — does not touch the filesystem.
pub fn config_path(app_data_dir: &Path, filename: &str) -> PathBuf {
    app_data_dir.join(filename)
}

/// Load and deserialize a JSON config file from an app's data directory.
///
/// Returns `None` on any read error (file missing, permissions) or parse
/// error (malformed JSON, shape mismatch) — callers fall back to an
/// app-specific default value. This matches the fallback behavior every
/// `app-workplace-*` crate independently re-implemented before this crate
/// existed (`load_port` in workbench, `load_config` in gis).
pub fn load_config<T: DeserializeOwned>(app_data_dir: &Path, filename: &str) -> Option<T> {
    let content = fs::read_to_string(config_path(app_data_dir, filename)).ok()?;
    serde_json::from_str(&content).ok()
}

/// Serialize and write a JSON config file into an app's data directory,
/// creating the directory first if it does not already exist.
pub fn save_config<T: Serialize>(
    app_data_dir: &Path,
    filename: &str,
    config: &T,
) -> Result<(), String> {
    fs::create_dir_all(app_data_dir).map_err(|e| e.to_string())?;
    let serialized = serde_json::to_string(config).map_err(|e| e.to_string())?;
    fs::write(config_path(app_data_dir, filename), serialized).map_err(|e| e.to_string())
}

/// True if a named JSON config file exists in an app's data directory — the
/// shared `has_X_config` half of the get/set/has triad every
/// `app-workplace-*` crate reimplemented independently.
pub fn has_config(app_data_dir: &Path, filename: &str) -> bool {
    config_path(app_data_dir, filename).exists()
}

/// Ensure an app's data directory exists. Shared half of every
/// `app-workplace-*` crate's `setup()` closure
/// (`app.path_resolver().app_data_dir()` → create it if missing).
pub fn ensure_app_data_dir(app_data_dir: &Path) -> io::Result<()> {
    fs::create_dir_all(app_data_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestConfig {
        value: String,
    }

    /// Each test gets its own directory under the OS temp dir, keyed by test
    /// name, and cleans up after itself — no external crate (e.g. `tempfile`)
    /// needed, keeping this crate's dependency footprint at exactly
    /// `serde` + `serde_json`.
    fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("workplace-shell-chrome-test-{name}"));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn has_config_false_when_directory_and_file_absent() {
        let dir = scratch_dir("has-config-absent");
        assert!(!has_config(&dir, "nope.json"));
    }

    #[test]
    fn load_config_none_when_file_absent() {
        let dir = scratch_dir("load-config-absent");
        let loaded: Option<TestConfig> = load_config(&dir, "nope.json");
        assert!(loaded.is_none());
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = scratch_dir("round-trip");
        let cfg = TestConfig {
            value: "hello".into(),
        };
        save_config(&dir, "test-config.json", &cfg).expect("save should succeed");
        assert!(has_config(&dir, "test-config.json"));
        let loaded: TestConfig =
            load_config(&dir, "test-config.json").expect("load should succeed");
        assert_eq!(loaded, cfg);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_config_none_on_malformed_json() {
        let dir = scratch_dir("malformed");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("bad.json"), "{not valid json").unwrap();
        let loaded: Option<TestConfig> = load_config(&dir, "bad.json");
        assert!(loaded.is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_config_creates_missing_directory() {
        let dir = scratch_dir("creates-dir");
        assert!(!dir.exists());
        let cfg = TestConfig {
            value: "created".into(),
        };
        save_config(&dir, "test-config.json", &cfg).expect("save should create dir + succeed");
        assert!(dir.exists());
        assert!(has_config(&dir, "test-config.json"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn ensure_app_data_dir_creates_and_is_idempotent() {
        let dir = scratch_dir("ensure-dir");
        assert!(!dir.exists());
        ensure_app_data_dir(&dir).expect("first ensure should create dir");
        assert!(dir.exists());
        ensure_app_data_dir(&dir).expect("second ensure should be a no-op success");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn config_path_joins_dir_and_filename() {
        let dir = PathBuf::from("/tmp/example-app-data");
        assert_eq!(
            config_path(&dir, "example-config.json"),
            PathBuf::from("/tmp/example-app-data/example-config.json")
        );
    }
}
