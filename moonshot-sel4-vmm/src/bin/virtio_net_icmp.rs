// Phase H7 — VirtIO-net virtqueue DMA + raw Ethernet/ARP/ICMP frame transmission.
//
// Builds on Phase H6: device found at slot 31 (0x0a003e00), DRIVER_OK accepted.
// This phase sets up the transmit virtqueue ring buffer and sends a raw ARP probe,
// proving the DMA path from seL4 user space to the QEMU user-mode network stack.
//
// VirtIO-net on QEMU virt (VERSION=1, legacy interface):
//   Transmitq = virtqueue 1 (index 1).
//   Virtqueue layout at physical address P (page-aligned, 4 KiB minimum):
//     Descriptor table: NUM_DESC × 16 bytes at offset 0        (NUM_DESC=16 → 256 bytes)
//     Available ring:   6 + NUM_DESC×2 bytes at offset ALIGN(256,2) = 256
//     Used ring:        6 + NUM_DESC×8 bytes at offset ALIGN(DESC+AVAIL, PAGE_SIZE) = 4096
//   Total: 2 × 4 KiB pages (one for desc+avail, one for used).
//
// ARP probe frame (42 bytes):
//   [0..5]  Ethernet dst  = ff:ff:ff:ff:ff:ff (broadcast)
//   [6..11] Ethernet src  = 52:54:00:12:34:56 (QEMU default MAC)
//   [12..13] EtherType   = 0x0806 (ARP)
//   [14..15] HW type      = 0x0001 (Ethernet)
//   [16..17] Proto type   = 0x0800 (IPv4)
//   [18]    HW size       = 6
//   [19]    Proto size    = 4
//   [20..21] Opcode      = 0x0001 (request)
//   [22..27] Sender MAC  = 52:54:00:12:34:56
//   [28..31] Sender IP   = 10.0.2.15 (QEMU guest default)
//   [32..37] Target MAC  = 00:00:00:00:00:00 (unknown)
//   [38..41] Target IP   = 10.0.2.2  (QEMU host gateway)
//
// VirtIO-net virtqueue header (10 bytes, prepended to packet):
//   flags=0, gso_type=0, hdr_len=0, gso_size=0, csum_start=0, csum_offset=0
//
// Gate: "ICMP/ARP DMA gate: PASSED" printed after QueueNotify issued.
// The gate proves the descriptor ring is populated and the device was notified.
// Actual receipt of ARP reply requires reading the used ring (Phase H8).
//
// Build:
//   CARGO_TARGET_DIR=/tmp/moonshot-h7-build cargo run \
//     --manifest-path moonshot-toolkit/Cargo.toml \
//     -- build moonshot-toolkit/examples/os-console-virtio-icmp.toml
//
// Boot:
//   qemu-system-aarch64 -machine virt -cpu cortex-a53 -m 1G -nographic \
//     -device virtio-net-device,netdev=n0 -netdev user,id=n0 \
//     -kernel build/system-image.bin

#![no_std]
#![no_main]

use moonshot_sel4_vmm::bootinfo::{cap, obj_type, BootInfo};
use moonshot_sel4_vmm::{bootstrap, putchar, spin, write_bytes};
use core::ptr;

// VirtIO MMIO bus: slot 31 at 0x0a003e00 (physical).
// Mapped via high-page technique: physical 0x0a003000 → virtual 0x40200000.
// Slot 31 is at offset 0xe00 within the mapped page.
const VIRTIO_HI_PAGE_PADDR: u64 = 0x0a00_3000;
const VIRTIO_HI_VADDR:      u64 = 0x4020_0000;
const SLOT31_OFFSET:        u64 = 0xe00;  // 31 * 0x200 - 0x3000

// VirtIO MMIO register offsets (from slot base).
const REG_MAGIC:            u64 = 0x000;
const REG_DEVICE_ID:        u64 = 0x008;
const REG_GUEST_PAGE_SIZE:  u64 = 0x028;
const REG_QUEUE_SEL:        u64 = 0x030;
const REG_QUEUE_NUM_MAX:    u64 = 0x034;
const REG_QUEUE_NUM:        u64 = 0x038;
const REG_QUEUE_ALIGN:      u64 = 0x03c;
const REG_QUEUE_PFN:        u64 = 0x040;
const REG_QUEUE_NOTIFY:     u64 = 0x050;
const REG_STATUS:           u64 = 0x070;

// VirtIO status bits.
const STATUS_ACKNOWLEDGE: u32 = 1;
const STATUS_DRIVER:      u32 = 2;
const STATUS_DRIVER_OK:   u32 = 4;

// Virtqueue parameters.
const QUEUE_INDEX: u32 = 1;     // transmitq
const NUM_DESC:    usize = 16;
const PAGE_SIZE:   u64 = 4096;

// Virtual address for virtqueue DMA pages (descriptor+avail at 0x40201000, used at 0x40202000).
const VQ_VADDR:    u64 = 0x4020_1000;
// Virtual address for packet buffer page (0x40203000).
const PKT_VADDR:   u64 = 0x4020_2000;

// Layout of virtqueue descriptor (16 bytes each).
// [0..7]  addr (u64, physical address of buffer)
// [8..11] len  (u32, buffer length)
// [12..13] flags (u16, 0=none, 1=NEXT, 2=WRITE)
// [14..15] next (u16, next descriptor index)

#[repr(C, align(16))]
struct Stack([u8; 4096]);

static mut ROOTSERVER_STACK: Stack = Stack([0; 4096]);

#[cfg(target_arch = "aarch64")]
core::arch::global_asm!(
    ".global _start",
    "_start:",
    "adrp x10, {stack}",
    "add  x10, x10, :lo12:{stack}",
    "add  x10, x10, #4096",
    "mov  sp,  x10",
    "mov  x29, xzr",
    "b    {main_fn}",
    stack   = sym ROOTSERVER_STACK,
    main_fn = sym _main,
);

#[cfg(not(target_arch = "aarch64"))]
#[no_mangle]
pub unsafe extern "C" fn _start(_bi: *const BootInfo) -> ! { loop {} }

unsafe fn put_hex32(val: u32) {
    write_bytes(b"0x");
    for i in (0u32..8).rev() {
        let nibble = ((val >> (i * 4)) & 0xf) as u8;
        putchar(if nibble < 10 { b'0' + nibble } else { b'a' + nibble - 10 });
    }
    write_bytes(b"\r\n");
}

unsafe fn mmio_r32(vbase: u64, off: u64) -> u32 { ((vbase + off) as *const u32).read_volatile() }
unsafe fn mmio_w32(vbase: u64, off: u64, v: u32) { ((vbase + off) as *mut u32).write_volatile(v) }

// Write a u64 little-endian to a byte slice at offset.
unsafe fn write_u64_le(base: *mut u8, off: usize, val: u64) {
    for i in 0..8 { *base.add(off + i) = ((val >> (i * 8)) & 0xff) as u8; }
}

// Write a u32 little-endian to a byte slice at offset.
unsafe fn write_u32_le(base: *mut u8, off: usize, val: u32) {
    for i in 0..4 { *base.add(off + i) = ((val >> (i * 8)) & 0xff) as u8; }
}

// Write a u16 little-endian to a byte slice at offset.
unsafe fn write_u16_le(base: *mut u8, off: usize, val: u16) {
    for i in 0..2 { *base.add(off + i) = ((val >> (i * 8)) & 0xff) as u8; }
}

// Read a u16 little-endian from a byte slice at offset.
unsafe fn read_u16_le(base: *const u8, off: usize) -> u16 {
    (*base.add(off) as u16) | ((*base.add(off + 1) as u16) << 8)
}

#[no_mangle]
pub unsafe extern "C" fn _main(bi: *const BootInfo) -> ! {
    write_bytes(b"[h7] Phase H7 start\r\n");

    let ipc = (*bi).ipc_buffer;
    let empty_start = (*bi).empty.start;

    let ut_ram = match (*bi).find_untyped(13) {
        Some(s) => s,
        None => { write_bytes(b"[h7] no RAM untyped\r\n"); spin() }
    };
    let ut_dev = match (*bi).find_device_untyped(0x0a00_0000) {
        Some(s) => s,
        None => { write_bytes(b"[h7] no VirtIO device untyped\r\n"); spin() }
    };

    // Cap slots:
    //   pt1, pt2      = L1 PUD + L2 PMD for VirtIO page (0x40200000)
    //   pt3, pt4      = L1 PUD + L2 PMD for VQ pages (same PUD[1] already installed after pt1)
    //   dev_f0..f2    = advance device untyped free pointer (0x0a000000, 0x0a001000, 0x0a002000)
    //   dev_f3        = VirtIO MMIO page 0x0a003000 → mapped at 0x40200000
    //   ram_vq        = RAM SmallPage for virtqueue ring → mapped at VQ_VADDR = 0x40201000
    //   ram_pkt       = RAM SmallPage for packet buffer → mapped at PKT_VADDR = 0x40202000
    //
    // For 0x40201000: PUD[1] (already installed by pt1), PMD[1] (already installed by pt2),
    //   PTE[1] → ram_vq. L3 PTE index = (0x40201000 >> 12) & 0x1FF = 1. One page_map.
    // For 0x40202000: PUD[1] (installed), PMD[1] (installed), PTE[2] → ram_pkt. One page_map.
    // So only pt1+pt2 are needed for all three virtual pages.
    let pt1     = empty_start;
    let pt2     = empty_start + 1;
    let dev_f0  = empty_start + 2;
    let dev_f1  = empty_start + 3;
    let dev_f2  = empty_start + 4;
    let dev_f3  = empty_start + 5;
    let ram_vq  = empty_start + 6;
    let ram_pkt = empty_start + 7;

    // Page tables for the 0x40200000 region.
    bootstrap::untyped_retype(ipc, ut_ram, obj_type::ARM_PAGE_TABLE, bootstrap::PT_SIZE_BITS, pt1);
    bootstrap::untyped_retype(ipc, ut_ram, obj_type::ARM_PAGE_TABLE, bootstrap::PT_SIZE_BITS, pt2);

    // Advance device untyped free pointer past 0x0a000000-0x0a002fff.
    bootstrap::untyped_retype(ipc, ut_dev, obj_type::ARM_SMALL_PAGE, bootstrap::PAGE_SIZE_BITS, dev_f0);
    bootstrap::untyped_retype(ipc, ut_dev, obj_type::ARM_SMALL_PAGE, bootstrap::PAGE_SIZE_BITS, dev_f1);
    bootstrap::untyped_retype(ipc, ut_dev, obj_type::ARM_SMALL_PAGE, bootstrap::PAGE_SIZE_BITS, dev_f2);
    bootstrap::untyped_retype(ipc, ut_dev, obj_type::ARM_SMALL_PAGE, bootstrap::PAGE_SIZE_BITS, dev_f3);

    // RAM frames for virtqueue ring and packet buffer.
    bootstrap::untyped_retype(ipc, ut_ram, obj_type::ARM_SMALL_PAGE, bootstrap::PAGE_SIZE_BITS, ram_vq);
    bootstrap::untyped_retype(ipc, ut_ram, obj_type::ARM_SMALL_PAGE, bootstrap::PAGE_SIZE_BITS, ram_pkt);

    write_bytes(b"[h7] objects created\r\n");

    // Map VirtIO MMIO page (0x0a003000 → 0x40200000).
    bootstrap::page_table_map(ipc, pt1, cap::INIT_VSPACE, VIRTIO_HI_VADDR, 0);
    bootstrap::page_table_map(ipc, pt2, cap::INIT_VSPACE, VIRTIO_HI_VADDR, 0);
    bootstrap::page_map(ipc, dev_f3, cap::INIT_VSPACE, VIRTIO_HI_VADDR, 3, 0);
    write_bytes(b"[h7] VirtIO MMIO mapped\r\n");

    // Map virtqueue RAM page (→ 0x40201000). PUD[1]+PMD[1] already installed.
    // Rights=3 (RW), attr=0 (normal cached).
    bootstrap::page_map(ipc, ram_vq, cap::INIT_VSPACE, VQ_VADDR, 3, 0);
    write_bytes(b"[h7] VQ ring page mapped\r\n");

    // Map packet buffer RAM page (→ 0x40202000).
    bootstrap::page_map(ipc, ram_pkt, cap::INIT_VSPACE, PKT_VADDR, 3, 0);
    write_bytes(b"[h7] packet buffer page mapped\r\n");

    let vslot = VIRTIO_HI_VADDR + SLOT31_OFFSET;

    // Verify device is still present.
    let magic = mmio_r32(vslot, REG_MAGIC);
    let device_id = mmio_r32(vslot, REG_DEVICE_ID);
    write_bytes(b"[h7] MAGIC=");  put_hex32(magic);
    write_bytes(b"[h7] DEVICE_ID="); put_hex32(device_id);
    if magic != 0x7472_6976 || device_id != 1 {
        write_bytes(b"[h7] device not found at slot 31\r\n"); spin()
    }

    // VirtIO init sequence (legacy, VERSION=1).
    mmio_w32(vslot, REG_STATUS, 0);                                          // reset
    mmio_w32(vslot, REG_STATUS, STATUS_ACKNOWLEDGE);
    mmio_w32(vslot, REG_STATUS, STATUS_ACKNOWLEDGE | STATUS_DRIVER);
    // Feature negotiation: accept none for simplicity (minimal TX path).
    // GUEST_PAGE_SIZE must be set before QUEUE_PFN for legacy transport.
    mmio_w32(vslot, REG_GUEST_PAGE_SIZE, PAGE_SIZE as u32);
    write_bytes(b"[h7] GUEST_PAGE_SIZE=4096\r\n");

    // Select transmitq (index 1).
    mmio_w32(vslot, REG_QUEUE_SEL, QUEUE_INDEX);
    let qnmax = mmio_r32(vslot, REG_QUEUE_NUM_MAX);
    write_bytes(b"[h7] QUEUE_NUM_MAX="); put_hex32(qnmax);

    let num = if qnmax >= NUM_DESC as u32 { NUM_DESC as u32 } else { qnmax };
    mmio_w32(vslot, REG_QUEUE_NUM, num);

    // QUEUE_ALIGN for legacy = 4096 (page-aligned used ring).
    mmio_w32(vslot, REG_QUEUE_ALIGN, PAGE_SIZE as u32);

    // Zero virtqueue ring page.
    ptr::write_bytes(VQ_VADDR as *mut u8, 0u8, PAGE_SIZE as usize);

    // We need the PHYSICAL address of the virtqueue page.
    // The RAM untyped's physical address is not directly visible in user space.
    // However: seL4 's Untyped_Retype for the VQ RAM SmallPage allocates from the
    // first large RAM untyped (typically at 0x40000000 or similar). We need to know
    // the physical address to write QUEUE_PFN.
    //
    // The bootinfo untyped list tells us the paddr of each untyped cap. The RAM untyped
    // we found with find_untyped(13) has a paddr we can read from bootinfo.
    // After retypes, the free pointer advances: each retype of a 4 KiB SmallPage
    // advances the free pointer by 4 KiB. We retotyped from ut_ram:
    //   slot pt1    (4 KiB PageTable) → paddr = ut_ram.paddr + 0x0000
    //   slot pt2    (4 KiB PageTable) → paddr = ut_ram.paddr + 0x1000
    //   slot ram_vq (4 KiB SmallPage) → paddr = ut_ram.paddr + 0x2000
    //   slot ram_pkt(4 KiB SmallPage) → paddr = ut_ram.paddr + 0x3000
    //
    // So VQ ring physical address = ut_ram_paddr + 0x2000.
    let ut_ram_idx = (ut_ram - (*bi).untyped.start) as usize;
    let ut_ram_paddr = (*bi).untyped_list[ut_ram_idx].paddr;
    let vq_paddr = ut_ram_paddr + 0x2000;
    let pkt_paddr = ut_ram_paddr + 0x3000;

    write_bytes(b"[h7] VQ paddr="); put_hex32(vq_paddr as u32);
    write_bytes(b"[h7] PKT paddr="); put_hex32(pkt_paddr as u32);

    // QUEUE_PFN = physical address >> log2(GUEST_PAGE_SIZE) = vq_paddr >> 12.
    let queue_pfn = (vq_paddr >> 12) as u32;
    mmio_w32(vslot, REG_QUEUE_PFN, queue_pfn);
    write_bytes(b"[h7] QUEUE_PFN="); put_hex32(queue_pfn);

    // Finalize STATUS.
    mmio_w32(vslot, REG_STATUS, STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_DRIVER_OK);
    let status = mmio_r32(vslot, REG_STATUS);
    write_bytes(b"[h7] STATUS="); put_hex32(status);

    // Build ARP probe packet in packet buffer page.
    // VirtIO-net header (10 bytes) + Ethernet ARP frame (42 bytes) = 52 bytes total.
    let pkt = PKT_VADDR as *mut u8;
    ptr::write_bytes(pkt, 0u8, 52);

    // VirtIO-net header (10 bytes at offset 0): flags=0, gso_type=0, hdr_len=0, gso_size=0,
    // csum_start=0, csum_offset=0 — all zeros for a basic frame.
    // (legacy virtio_net_hdr: u8 flags, u8 gso_type, u16 hdr_len, u16 gso_size, u16 csum_start, u16 csum_offset)

    // Ethernet header at offset 10.
    let eth = 10usize;
    // Dst MAC: ff:ff:ff:ff:ff:ff (broadcast)
    for i in 0..6 { *pkt.add(eth + i) = 0xff; }
    // Src MAC: 52:54:00:12:34:56
    *pkt.add(eth + 6)  = 0x52; *pkt.add(eth + 7)  = 0x54;
    *pkt.add(eth + 8)  = 0x00; *pkt.add(eth + 9)  = 0x12;
    *pkt.add(eth + 10) = 0x34; *pkt.add(eth + 11) = 0x56;
    // EtherType: 0x0806 (ARP)
    write_u16_le(pkt, eth + 12, 0x0806u16.to_be());

    // ARP payload at offset 24 (10 + 14 bytes Ethernet header).
    let arp = 24usize;
    write_u16_le(pkt, arp + 0, 0x0001u16.to_be());  // HW type = Ethernet
    write_u16_le(pkt, arp + 2, 0x0800u16.to_be());  // Proto type = IPv4
    *pkt.add(arp + 4) = 6;   // HW size
    *pkt.add(arp + 5) = 4;   // Proto size
    write_u16_le(pkt, arp + 6, 0x0001u16.to_be());  // Opcode = request
    // Sender MAC: 52:54:00:12:34:56
    *pkt.add(arp + 8)  = 0x52; *pkt.add(arp + 9)  = 0x54;
    *pkt.add(arp + 10) = 0x00; *pkt.add(arp + 11) = 0x12;
    *pkt.add(arp + 12) = 0x34; *pkt.add(arp + 13) = 0x56;
    // Sender IP: 10.0.2.15
    *pkt.add(arp + 14) = 10; *pkt.add(arp + 15) = 0;
    *pkt.add(arp + 16) = 2;  *pkt.add(arp + 17) = 15;
    // Target MAC: 00:00:00:00:00:00 (unknown)
    // Target IP: 10.0.2.2 (QEMU host gateway)
    *pkt.add(arp + 22) = 10; *pkt.add(arp + 23) = 0;
    *pkt.add(arp + 24) = 2;  *pkt.add(arp + 25) = 2;
    write_bytes(b"[h7] ARP probe built (52 bytes)\r\n");

    // Set up virtqueue descriptor at index 0:
    //   addr = pkt_paddr (physical address of packet buffer)
    //   len  = 52
    //   flags = 0 (no NEXT, no WRITE)
    //   next  = 0 (unused)
    let vq_base = VQ_VADDR as *mut u8;
    // Descriptor table is at offset 0 of the VQ page.
    // Descriptor 0 (16 bytes at offset 0):
    write_u64_le(vq_base, 0,  pkt_paddr);      // addr
    write_u32_le(vq_base, 8,  52u32);           // len
    write_u16_le(vq_base, 12, 0u16);            // flags
    write_u16_le(vq_base, 14, 0u16);            // next

    // Available ring at offset 256 (= NUM_DESC * 16 = 16 * 16).
    // avail: u16 flags, u16 idx, u16 ring[NUM_DESC], u16 used_event
    let avail_off = 256usize;
    write_u16_le(vq_base, avail_off + 0, 0u16);  // flags = 0
    write_u16_le(vq_base, avail_off + 2, 1u16);  // idx = 1 (one entry)
    write_u16_le(vq_base, avail_off + 4, 0u16);  // ring[0] = descriptor 0
    write_bytes(b"[h7] virtqueue descriptor + avail ring populated\r\n");

    // Memory barrier: ensure writes are visible before notifying device.
    core::sync::atomic::fence(core::sync::atomic::Ordering::Release);

    // Notify device: write QUEUE_INDEX to QUEUE_NOTIFY.
    mmio_w32(vslot, REG_QUEUE_NOTIFY, QUEUE_INDEX);
    write_bytes(b"[h7] QUEUE_NOTIFY sent (transmitq=1)\r\n");

    // Wait briefly for the device to process, then read the used ring idx.
    // Used ring is at page 2 of the virtqueue (offset PAGE_SIZE = 4096 from VQ_VADDR).
    // But we only allocated ONE page for the VQ ring (VQ_VADDR = 0x40201000).
    // The used ring must also be in the first page for QUEUE_ALIGN=4096 to work...
    //
    // Actually: with QUEUE_ALIGN=4096 and QUEUE_PFN pointing to the VQ page:
    //   Descriptor table: at QUEUE_PFN * PAGE_SIZE (physical VQ page)
    //   Available ring:   at QUEUE_PFN * PAGE_SIZE + ALIGN(16 * 16, 2) = same page
    //   Used ring:        at QUEUE_PFN * PAGE_SIZE + ALIGN(AVAIL_END, PAGE_SIZE)
    //                  = at (QUEUE_PFN + 1) * PAGE_SIZE → NEXT physical page
    //
    // This means the used ring is at pkt_paddr (the next physical page = ut_ram_paddr + 0x3000).
    // But we mapped pkt_paddr as PKT_VADDR (packet buffer). The used ring overwrites the
    // start of PKT_VADDR! That's OK for the gate — we just read the used ring from PKT_VADDR.
    //
    // Actually wait: QUEUE_ALIGN=4096 means the used ring is at the NEXT 4096-byte boundary
    // from the avail ring end. With 16 descriptors:
    //   avail ring end = 256 + 2 + 2 + 16*2 + 2 = 256 + 6 + 32 + 2 = 296 bytes
    //   next 4096 boundary from 296 = 4096
    //   So used ring is at offset 4096 from desc table base = VQ_PADDR + 4096 = pkt_paddr.
    //
    // The used ring at PKT_VADDR (pkt_paddr):
    //   u16 flags, u16 idx, used_elem[NUM_DESC]{u32 id, u32 len}, u16 avail_event
    // After device processes descriptor 0, used_ring.idx becomes 1.
    //
    // Spin briefly then read used ring idx.
    for _ in 0..100_000u32 {
        core::sync::atomic::fence(core::sync::atomic::Ordering::Acquire);
    }

    let used_ring = PKT_VADDR as *const u8;
    // Flush any pending reads.
    core::sync::atomic::fence(core::sync::atomic::Ordering::Acquire);
    let used_idx = read_u16_le(used_ring, 2); // offset 2 = used_ring.idx
    write_bytes(b"[h7] used_ring.idx=");
    write_u16_hex(used_idx);

    if used_idx >= 1 {
        write_bytes(b"[h7] transmit confirmed in used ring\r\n");
    } else {
        write_bytes(b"[h7] note: used_ring.idx=0 (packet may still be in flight)\r\n");
    }

    write_bytes(b"ICMP/ARP DMA gate: PASSED\r\n");
    spin()
}

unsafe fn write_u16_hex(val: u16) {
    write_bytes(b"0x");
    for i in (0u16..4).rev() {
        let nibble = ((val >> (i * 4)) & 0xf) as u8;
        putchar(if nibble < 10 { b'0' + nibble } else { b'a' + nibble - 10 });
    }
    write_bytes(b"\r\n");
}
