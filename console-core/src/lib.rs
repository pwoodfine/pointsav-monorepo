//! # console-core
//!
//! The interaction core of the os-console rebuild (Phase I-1). It defines a
//! single, namespaced **Intent** vocabulary that *both* keyboard chords and
//! mouse gestures resolve into, plus the registry, keymap, and the structural
//! **parity gate** that makes "every action is reachable by both keyboard and
//! mouse" a build-time invariant rather than a convention.
//!
//! Design rule: cartridges act *only* on `Intent`s. Neither keys nor pointer
//! gestures mutate cartridge state directly — both are pure front-ends that
//! resolve into an [`IntentId`] and call a single dispatch path. Parity is then
//! the property that the only way to reach an action is through an `Intent`, and
//! any enabled `Intent` is reachable from the command palette (the keyboard
//! floor) by definition. See [`parity::audit`].
//!
//! This crate is intentionally dependency-free (pure std). Raw terminal input
//! is translated into the canonical [`Chord`] form and pane-local mouse
//! affordances by `app-console-keys`; this crate is terminal-agnostic.

pub mod intent;
pub mod keymap;
pub mod parity;
pub mod registry;
pub mod seed;

pub use intent::{Chord, IntentArgs, IntentId, IntentScope, IntentSpec, MouseAffordance, Waiver};
pub use keymap::Keymap;
pub use parity::{audit, waiver_count, ParityFault, ParityViolation};
pub use registry::IntentRegistry;
