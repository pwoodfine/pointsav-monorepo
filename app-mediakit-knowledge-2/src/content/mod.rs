//! Content pipeline — mounts, frontmatter, walk/index, and rendering.
//!
//! Markdown files in a Git tree are the source of truth; the `ContentIndex`
//! is a derived, regenerable slug → file lookup built at startup.

pub mod frontmatter;
pub mod mount;
pub mod render;
pub mod walk;

pub use frontmatter::{parse, Frontmatter, ParsedDoc};
pub use mount::{Mount, MountSet};
pub use render::{render, Heading, Rendered};
pub use walk::{load, ContentIndex, DocRef, Lang};
