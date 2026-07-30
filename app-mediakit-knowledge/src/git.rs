use crate::error::WikiError;
use git2::{Commit, Repository, Signature};
use std::path::{Path, PathBuf};

/// Opens the git repository at `content_dir` if one exists, or initialises a new one.
pub fn open_or_init(content_dir: &Path) -> Result<Repository, WikiError> {
    Repository::open(content_dir)
        .or_else(|_| Repository::init(content_dir))
        .map_err(|e| WikiError::WriteFailed(format!("git init/open failed: {e}")))
}

/// Commits a topic change.
///
/// Uses `git2`'s `Index::add_path` + `commit` machinery. The file being
/// committed is `<slug>.md` relative to the repo's workdir.
pub fn commit_topic(
    repo: &Repository,
    slug: &str,
    _body: &str,
    author_email: &str,
    author_name: &str,
    message: &str,
) -> Result<git2::Oid, WikiError> {
    let mut index = repo
        .index()
        .map_err(|e| WikiError::WriteFailed(format!("git index failed: {e}")))?;
    let path = Path::new(slug).with_extension("md");
    index
        .add_path(&path)
        .map_err(|e| WikiError::WriteFailed(format!("git add failed: {e}")))?;
    index
        .write()
        .map_err(|e| WikiError::WriteFailed(format!("git index write failed: {e}")))?;

    let tree_id = index
        .write_tree()
        .map_err(|e| WikiError::WriteFailed(format!("git write tree failed: {e}")))?;
    let tree = repo
        .find_tree(tree_id)
        .map_err(|e| WikiError::WriteFailed(format!("git find tree failed: {e}")))?;

    let sig = if author_name.is_empty() || author_email.is_empty() {
        repo.signature()
            .map_err(|e| WikiError::WriteFailed(format!("git default signature failed: {e}")))?
    } else {
        Signature::now(author_name, author_email)
            .map_err(|e| WikiError::WriteFailed(format!("git signature failed: {e}")))?
    };

    let parent_commit = match repo.head() {
        Ok(head) => Some(
            head.peel_to_commit()
                .map_err(|e| WikiError::WriteFailed(format!("git peel to commit failed: {e}")))?,
        ),
        Err(_) => None,
    };

    let parents: Vec<&Commit> = parent_commit.as_ref().map(|c| vec![c]).unwrap_or_default();

    let commit_id = repo
        .commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)
        .map_err(|e| WikiError::WriteFailed(format!("git commit failed: {e}")))?;

    Ok(commit_id)
}

/// Falls back to the bin/commit-as-next.sh J/P alternation identity.
pub fn ensure_commit_identity_from_env(repo: &Repository) -> Result<(), WikiError> {
    let toggle_path = std::env::var("HOME")
        .map(|h| PathBuf::from(h).join("Foundry/identity/.toggle"))
        .unwrap_or_else(|_| PathBuf::from("/srv/foundry/identity/.toggle"));

    let (name, email) = match std::fs::read_to_string(&toggle_path) {
        Ok(s) => {
            if s.trim() == "1" {
                ("Peter Woodfine", "pwoodfine@users.noreply.github.com")
            } else {
                ("Jennifer Woodfine", "jwoodfine@users.noreply.github.com")
            }
        }
        Err(_) => {
            tracing::warn!(path = %toggle_path.display(), "identity toggle file missing or unreadable, defaulting to Jennifer");
            ("Jennifer Woodfine", "jwoodfine@users.noreply.github.com")
        }
    };

    let mut config = repo
        .config()
        .map_err(|e| WikiError::WriteFailed(format!("git config failed: {e}")))?;
    config
        .set_str("user.name", name)
        .map_err(|e| WikiError::WriteFailed(format!("git set user.name failed: {e}")))?;
    config
        .set_str("user.email", email)
        .map_err(|e| WikiError::WriteFailed(format!("git set user.email failed: {e}")))?;

    Ok(())
}
