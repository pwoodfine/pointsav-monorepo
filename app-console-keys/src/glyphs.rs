//! Named glyph vocabulary (Phase C-1). Centralizing these stops literals from
//! scattering across cartridges and keeps the iconography consistent. Rounded
//! chrome is for human surfaces; square chrome for machine-of-record surfaces
//! (see `widgets::card`).

/// Workspace / anchor mark.
pub const ANCHOR: &str = "◈";
/// Waiting (ambient, paired with a slow pulse — never a fast spinner).
pub const WAIT: &str = "◌";
/// Accept / success.
pub const OK: &str = "✓";
/// Reject / deny.
pub const DENY: &str = "✗";
/// Edit.
pub const EDIT: &str = "✎";
/// Document.
pub const DOC: &str = "▤";
/// Search.
pub const SEARCH: &str = "⌕";
/// Cryptographic seal / audit (the WORM ledger, the F12 SEALED state).
pub const SEAL: &str = "⬡";
/// Inline arrow / breadcrumb.
pub const ARROW: &str = "›";
/// Filled status dot.
pub const DOT_FILLED: &str = "●";
/// Empty status dot.
pub const DOT_EMPTY: &str = "◌";
/// Selection marker (active row).
pub const MARKER: &str = "▌";
