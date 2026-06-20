// Protection Domain event-loop trait and entry-point macro.
//
// Microkit PD protocol (from the seL4 Microkit manual):
//   init()           — called once by rootserver at PD start.
//   notified(ch)     — called on incoming notification on channel `ch`.
//   protected(ch, m) — called on a protected-procedure-call arrival.
//
// The `sel4_main!` macro generates C-ABI entry points from a type that
// implements `PdEntry`. Each os-* PD crate defines one struct per domain
// and invokes `sel4_main!(MyPd)` at crate root.
//
// Note on CompileRustPd: moonshot-toolkit Phase 1C compiles C PDs only.
// Rust PD compilation (CompileRustPd plan step) is the Phase H2 gate.
// Until then, os-totebox.toml references C stub binaries; this module
// provides the typed interface that the Rust PDs will implement when the
// Rust compile path lands.

use crate::{ChannelId, MsgInfo};

/// Trait implemented by each seL4 protection domain.
///
/// Microkit invokes `init`, `notified`, and `pp_called` via C ABI.
/// The `sel4_main!` macro bridges the C symbols to this trait.
pub trait PdEntry {
    /// Called once by Microkit after the PD is fully initialised.
    fn init();
    /// Called by Microkit on an incoming notification on channel `ch`.
    fn notified(ch: ChannelId);
    /// Called by Microkit for a protected-procedure-call on channel `ch`.
    /// Returns a reply MsgInfo (use `MsgInfo::new(0, 0, 0, 0)` for void).
    fn pp_called(ch: ChannelId, msginfo: MsgInfo) -> MsgInfo;
}

/// Generate Microkit C-ABI entry points from a [`PdEntry`] implementation.
///
/// ```rust,ignore
/// struct WatchdogPd;
/// impl moonshot_sel4_vmm::pd::PdEntry for WatchdogPd {
///     fn init() { moonshot_sel4_vmm::puts_line("watchdog: init"); }
///     fn notified(ch: ChannelId) { /* handle heartbeat */ }
///     fn pp_called(ch: ChannelId, m: MsgInfo) -> MsgInfo { MsgInfo::new(0,0,0,0) }
/// }
/// moonshot_sel4_vmm::sel4_main!(WatchdogPd);
/// ```
#[macro_export]
macro_rules! sel4_main {
    ($pd:ty) => {
        #[no_mangle]
        pub extern "C" fn init() {
            <$pd as $crate::pd::PdEntry>::init();
        }

        #[no_mangle]
        pub extern "C" fn notified(ch: u64) {
            <$pd as $crate::pd::PdEntry>::notified($crate::ChannelId::new(ch));
        }

        #[no_mangle]
        pub extern "C" fn protected(ch: u64, msginfo: u64) -> u64 {
            <$pd as $crate::pd::PdEntry>::pp_called(
                $crate::ChannelId::new(ch),
                $crate::MsgInfo::from(msginfo),
            )
            .raw()
        }
    };
}

/// Signal (notify) another PD on the given output channel.
///
/// Wraps `seL4_Signal` — the Microkit notification ABI. `ch` is the
/// local channel ID as declared in the system-description TOML for this PD.
///
/// # Safety
/// `ch.value()` must be a valid output-notification cap slot in this PD's CSpace.
#[inline]
pub fn notify(ch: ChannelId) {
    let empty = MsgInfo::new(0, 0, 0, 0);
    // Safety: caller upholds cap-slot validity.
    unsafe { crate::syscall::send(ch.value(), empty.raw()) }
}

/// Channel ID constants for the os-totebox 7-PD Capability Geometry spec.
///
/// Local channel IDs are assigned per PD in order of channel declaration in
/// `moonshot-toolkit/examples/os-totebox.toml`. Each PD sees its own
/// 0-indexed local view of the channels it participates in.
///
/// Topology (all Notification channels):
///   watchdog-pd ↔ service-fs-pd           (wd-fs-hb)
///   watchdog-pd ↔ network-pd              (wd-network-hb)
///   watchdog-pd ↔ service-content-pd      (wd-content-hb)
///   watchdog-pd ↔ service-people-pd       (wd-people-hb)
///   watchdog-pd ↔ service-slm-pd          (wd-slm-hb)
///   watchdog-pd ↔ service-extraction-pd   (wd-extraction-hb)
///   service-extraction-pd ↔ service-fs-pd (extraction-fs-drop)
pub mod channels {
    use crate::ChannelId;

    // ── watchdog-pd (priority 250) ────────────────────────────────────────────
    // Six heartbeat channels; one per monitored PD; local IDs 0–5.
    pub const WD_SERVICE_FS: ChannelId         = ChannelId::new(0);
    pub const WD_NETWORK_PD: ChannelId         = ChannelId::new(1);
    pub const WD_SERVICE_CONTENT: ChannelId    = ChannelId::new(2);
    pub const WD_SERVICE_PEOPLE: ChannelId     = ChannelId::new(3);
    pub const WD_SERVICE_SLM: ChannelId        = ChannelId::new(4);
    pub const WD_SERVICE_EXTRACTION: ChannelId = ChannelId::new(5);

    // ── service-fs-pd (priority 200) ─────────────────────────────────────────
    // Ch 0: heartbeat with watchdog. Ch 1: drop notification from extraction.
    pub const FS_WATCHDOG: ChannelId           = ChannelId::new(0);
    pub const FS_FROM_EXTRACTION: ChannelId    = ChannelId::new(1);

    // ── network-pd (priority 180) ─────────────────────────────────────────────
    pub const NETWORK_WATCHDOG: ChannelId      = ChannelId::new(0);

    // ── service-content-pd (priority 150) ────────────────────────────────────
    pub const CONTENT_WATCHDOG: ChannelId      = ChannelId::new(0);

    // ── service-people-pd (priority 130) ─────────────────────────────────────
    pub const PEOPLE_WATCHDOG: ChannelId       = ChannelId::new(0);

    // ── service-slm-pd (priority 120) ────────────────────────────────────────
    pub const SLM_WATCHDOG: ChannelId          = ChannelId::new(0);

    // ── service-extraction-pd (priority 110) ──────────────────────────────────
    // Ch 0: heartbeat with watchdog. Ch 1: write-drop notification to service-fs.
    pub const EXTRACTION_WATCHDOG: ChannelId   = ChannelId::new(0);
    pub const EXTRACTION_TO_FS: ChannelId      = ChannelId::new(1);

    // ── Protocol labels ───────────────────────────────────────────────────────

    /// MsgInfo label: service PD → watchdog heartbeat pulse.
    pub const HEARTBEAT_LABEL: u64 = 1;

    /// MsgInfo label: watchdog → service PD eviction warning.
    /// PD must flush state and halt; hardware watchdog timer will fire.
    pub const WATCHDOG_EVICT_LABEL: u64 = 2;

    /// MsgInfo label: service-extraction → service-fs watch-drop notification.
    /// service-fs reads from FS_WATCH_DROP_DIR on receipt.
    pub const EXTRACTION_DROP_LABEL: u64 = 1;
}

#[cfg(test)]
mod tests {
    use super::channels::*;
    use crate::ChannelId;

    #[test]
    fn watchdog_channel_ids_are_contiguous() {
        assert_eq!(WD_SERVICE_FS.value(), 0);
        assert_eq!(WD_NETWORK_PD.value(), 1);
        assert_eq!(WD_SERVICE_CONTENT.value(), 2);
        assert_eq!(WD_SERVICE_PEOPLE.value(), 3);
        assert_eq!(WD_SERVICE_SLM.value(), 4);
        assert_eq!(WD_SERVICE_EXTRACTION.value(), 5);
    }

    #[test]
    fn extraction_channels_are_ordered() {
        // Watchdog heartbeat must come before the drop notification.
        assert!(EXTRACTION_WATCHDOG.value() < EXTRACTION_TO_FS.value());
    }

    #[test]
    fn fs_channels_are_ordered() {
        assert!(FS_WATCHDOG.value() < FS_FROM_EXTRACTION.value());
    }

    #[test]
    fn label_constants_are_nonzero() {
        assert!(HEARTBEAT_LABEL > 0);
        assert!(WATCHDOG_EVICT_LABEL > 0);
        assert!(EXTRACTION_DROP_LABEL > 0);
    }
}
