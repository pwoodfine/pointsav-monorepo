//! Git-backed article history (git2 0.20).
//!
//! The content mount is itself a git repository (canonical per Doctrine §IV.e), so
//! an article's revision history is the git log of its file. We walk HEAD newest-
//! first and keep the commits that touched the file (compared against the first
//! parent, i.e. `git log` default semantics). Read-only; opened per request.

use std::path::Path;

use git2::{DiffOptions, Repository, Sort};

/// One revision of an article.
pub struct Revision {
    pub short_sha: String,
    pub author: String,
    pub date_iso: String, // YYYY-MM-DD
    pub message: String,  // subject line only
}

/// History of `rel` (path relative to `repo_root`), newest first, up to `limit`.
/// Returns an empty vec on any git error (no repo, detached, etc.).
pub fn file_history(repo_root: &Path, rel: &Path, limit: usize) -> Vec<Revision> {
    let Ok(repo) = Repository::open(repo_root) else {
        return Vec::new();
    };
    let Ok(mut walk) = repo.revwalk() else {
        return Vec::new();
    };
    let _ = walk.set_sorting(Sort::TIME);
    if walk.push_head().is_err() {
        return Vec::new();
    }

    let mut out = Vec::new();
    for oid in walk.flatten() {
        if out.len() >= limit {
            break;
        }
        let Ok(commit) = repo.find_commit(oid) else {
            continue;
        };
        if !touches(&repo, &commit, rel) {
            continue;
        }
        let author = commit.author();
        let subject = commit
            .summary()
            .unwrap_or("")
            .to_string();
        out.push(Revision {
            short_sha: oid.to_string().chars().take(8).collect(),
            author: author.name().unwrap_or("unknown").to_string(),
            date_iso: iso_date(commit.time().seconds()),
            message: subject,
        });
    }
    out
}

/// Did `commit` change `rel` relative to its first parent (or, for a root commit,
/// is the file present)?
fn touches(repo: &Repository, commit: &git2::Commit, rel: &Path) -> bool {
    let Ok(tree) = commit.tree() else {
        return false;
    };
    let mut opts = DiffOptions::new();
    opts.pathspec(rel);
    if commit.parent_count() == 0 {
        return repo
            .diff_tree_to_tree(None, Some(&tree), Some(&mut opts))
            .map(|d| d.deltas().len() > 0)
            .unwrap_or(false);
    }
    let Ok(parent) = commit.parent(0) else {
        return false;
    };
    let Ok(ptree) = parent.tree() else {
        return false;
    };
    repo.diff_tree_to_tree(Some(&ptree), Some(&tree), Some(&mut opts))
        .map(|d| d.deltas().len() > 0)
        .unwrap_or(false)
}

/// Format a Unix timestamp (UTC) as `YYYY-MM-DD` without pulling a date crate.
/// Uses Howard Hinnant's civil-from-days algorithm.
fn iso_date(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02}", y, m, d)
}
