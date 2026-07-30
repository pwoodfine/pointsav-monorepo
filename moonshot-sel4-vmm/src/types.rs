// seL4 Microkit IPC types for AArch64 bare-metal Protection Domains.
//
// seL4 message info layout (AArch64, 64-bit word):
//   Bits [63:12]  label
//   Bits [11:9]   caps_unwrapped count
//   Bits [8:7]    extra_caps count
//   Bits [6:0]    message registers used

/// seL4 message info word.
#[derive(Copy, Clone, Debug)]
#[repr(transparent)]
pub struct MsgInfo(u64);

impl MsgInfo {
    pub fn new(label: u64, caps_unwrapped: u64, extra_caps: u64, length: u64) -> Self {
        Self((label << 12) | ((caps_unwrapped & 0x7) << 9) | ((extra_caps & 0x3) << 7) | (length & 0x7f))
    }

    pub fn label(self) -> u64 {
        self.0 >> 12
    }

    pub fn length(self) -> u64 {
        self.0 & 0x7f
    }

    pub fn raw(self) -> u64 {
        self.0
    }
}

impl From<u64> for MsgInfo {
    fn from(v: u64) -> Self {
        Self(v)
    }
}

/// Microkit channel identifier.
///
/// Channels connect Protection Domains via seL4 Notification or Reply capabilities.
/// The channel ID is agreed at system-spec time (hello-world.toml / os-console-sel4.toml).
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct ChannelId(pub u64);

impl ChannelId {
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn value(self) -> u64 {
        self.0
    }
}
