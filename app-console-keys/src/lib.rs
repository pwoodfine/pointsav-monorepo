pub mod cartridge;
pub mod chassis;
pub mod colors;
pub mod config;
pub mod fkey;
pub mod input_bytes;
pub mod pairing;
pub mod qr;
pub mod session;
pub mod widgets;

pub use cartridge::{Cartridge, CartridgeAction};
pub use chassis::{request_shutdown, AppConsoleKeys, ChassisAction};
// Re-export the console-core intent vocabulary so cartridges depend only on
// app-console-keys (the chassis crate) to implement the intent methods.
pub use console_core::{IntentArgs, IntentId, IntentScope, IntentSpec, MouseAffordance, Waiver};
pub use config::ConsoleConfig;
pub use fkey::FKey;
pub use pairing::{PairingEvent, PairingState};
pub use session::SessionState;
