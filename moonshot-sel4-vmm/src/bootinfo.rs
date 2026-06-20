// seL4 BootInfo and IPC buffer types for AArch64.
//
// All sizes match seL4 v15.0.0-dev, KernelIsMCS=OFF, KernelSel4Arch=aarch64.
// CONFIG_MAX_NUM_BOOTINFO_UNTYPED_CAPS=230.
// seL4_SlotBits=5, seL4_TCBBits=11, seL4_EndpointBits=4, seL4_PageBits=12.

/// Cap slot numbers for the initial caps provided by the seL4 kernel.
pub mod cap {
    pub const NULL: u64 = 0;
    pub const INIT_TCB: u64 = 1;
    pub const INIT_CNODE: u64 = 2;
    pub const INIT_VSPACE: u64 = 3;
    pub const IRQ_CONTROL: u64 = 4;
    pub const ASID_CONTROL: u64 = 5;
    pub const INIT_ASID_POOL: u64 = 6;
    pub const BOOTINFO_FRAME: u64 = 9;
    pub const INIT_IPC_BUFFER: u64 = 10;
    pub const DOMAIN: u64 = 11;
}

/// seL4 object type numbers (non-MCS, non-SMP, AArch64).
pub mod obj_type {
    pub const UNTYPED: u64 = 0;
    pub const TCB: u64 = 1;
    pub const ENDPOINT: u64 = 2;
    pub const NOTIFICATION: u64 = 3;
    pub const CNODE: u64 = 4;
}

/// Invocation labels (non-MCS, no SMP, no DEBUG_API).
pub mod label {
    pub const UNTYPED_RETYPE: u64 = 1;
    pub const TCB_READ_REGISTERS: u64 = 2;
    pub const TCB_WRITE_REGISTERS: u64 = 3;
    pub const TCB_CONFIGURE: u64 = 5;
    pub const TCB_SET_PRIORITY: u64 = 6;
    pub const TCB_RESUME: u64 = 12;
}

/// Encode a seL4 MessageInfo word.
/// `msginfo_new(label, extra_caps, length)` — capsUnwrapped always 0 here.
#[inline(always)]
pub const fn msginfo_new(label: u64, extra_caps: u64, length: u64) -> u64 {
    (label << 12) | (extra_caps << 7) | length
}

/// A range of CNode slots [start, end).
#[repr(C)]
#[derive(Copy, Clone)]
pub struct SlotRegion {
    pub start: u64,
    pub end: u64,
}

/// Descriptor for one untyped memory region from BootInfo.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct UntypedDesc {
    pub paddr: u64,
    pub size_bits: u8,
    pub is_device: u8,
    pub _pad: [u8; 6],
}

/// seL4 IPC buffer layout (AArch64; seL4_MsgMaxLength=120, seL4_MsgMaxExtraCaps=3).
#[repr(C)]
pub struct IpcBuffer {
    pub tag: u64,
    pub msg: [u64; 120],
    pub user_data: u64,
    pub caps_or_badges: [u64; 3],
    pub receive_cnode: u64,
    pub receive_index: u64,
    pub receive_depth: u64,
}

/// seL4 BootInfo structure (non-MCS, AArch64, 230 untyped caps).
#[repr(C)]
pub struct BootInfo {
    pub extra_len: u64,
    pub node_id: u64,
    pub num_nodes: u64,
    pub num_io_pt_levels: u64,
    pub ipc_buffer: *mut IpcBuffer,
    pub empty: SlotRegion,
    pub shared_frames: SlotRegion,
    pub user_image_frames: SlotRegion,
    pub user_image_paging: SlotRegion,
    pub io_space_caps: SlotRegion,
    pub extra_bi_pages: SlotRegion,
    pub init_thread_cnode_size_bits: u64,
    pub init_thread_domain: u64,
    pub untyped: SlotRegion,
    pub untyped_list: [UntypedDesc; 230],
}

impl BootInfo {
    /// Find the cap slot of the first non-device untyped with sizeBits >= min.
    pub fn find_untyped(&self, min_size_bits: u8) -> Option<u64> {
        let count = (self.untyped.end - self.untyped.start) as usize;
        for i in 0..count {
            let d = &self.untyped_list[i];
            if d.is_device == 0 && d.size_bits >= min_size_bits {
                return Some(self.untyped.start + i as u64);
            }
        }
        None
    }
}
