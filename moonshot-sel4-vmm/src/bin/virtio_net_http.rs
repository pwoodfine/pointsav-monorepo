// Phase H8 — HTTP GET to 10.0.2.2:9080/healthz via raw TCP over VirtIO-net.
//
// QEMU user-mode networking (SLiRP) sends an ARP request before delivering
// the TCP SYN-ACK. We must reply to the ARP so SLiRP learns our MAC, then
// poll for the SYN-ACK to arrive on the receive ring.
//
// Full flow:
//   1. Offer RX descriptor (receive buffer for incoming frames)
//   2. Send TCP SYN to 10.0.2.2:9080 (DST = gateway MAC)
//   3. SLiRP sends ARP request: "who has 10.0.2.15?" → rx ring
//   4. Reply ARP with our MAC (52:54:00:12:34:56 → 10.0.2.15)
//   5. SLiRP delivers TCP SYN-ACK → extract server_seq
//   6. Send TCP ACK + HTTP GET /healthz
//   7. Poll RX for HTTP/1.1 response from Doorman
//   8. Gate: "HTTP GET gate: PASSED"
//
// QEMU user-mode networking constants:
//   Guest IP:    10.0.2.15
//   Guest MAC:   52:54:00:12:34:56 (QEMU VirtIO-net default)
//   Gateway IP:  10.0.2.2
//   Gateway MAC: 52:55:0a:00:02:02 (SLiRP router)
//   Host port:   9080 → localhost:9080 → Doorman /healthz
//
// Virtual memory layout (0x40200000–0x40205fff, one PMD):
//   0x40200000  VirtIO MMIO high page (slot 31 at +0xe00)
//   0x40201000  RX virtqueue ring (QUEUE_PFN for receiveq, queue 0)
//   0x40202000  RX used ring (VirtIO places here: rx_ring_paddr+4096)
//   0x40203000  RX receive buffer
//   0x40204000  TX virtqueue ring (QUEUE_PFN for transmitq, queue 1)
//   0x40205000  TX packet data + TX used ring (tx_ring_paddr+4096)
//
// Build:
//   CARGO_TARGET_DIR=/tmp/moonshot-h8-build cargo run \
//     --manifest-path moonshot-toolkit/Cargo.toml \
//     -- build moonshot-toolkit/examples/os-console-virtio-http.toml
//
// Boot:
//   qemu-system-aarch64 -machine virt -cpu cortex-a53 -m 1G \
//     -device virtio-net-device,netdev=n0 -netdev user,id=n0 \
//     -kernel build/system-image.bin -display none -serial stdio

#![no_std]
#![no_main]

use moonshot_sel4_vmm::bootinfo::{cap, obj_type, BootInfo};
use moonshot_sel4_vmm::{bootstrap, putchar, spin, write_bytes};
use core::ptr;
use core::sync::atomic::{fence, Ordering};

const VIRTIO_HI_VADDR: u64 = 0x4020_0000;
const SLOT31_OFFSET:   u64 = 0xe00;

const REG_MAGIC:           u64 = 0x000;
const REG_DEVICE_ID:       u64 = 0x008;
const REG_GUEST_PAGE_SIZE: u64 = 0x028;
const REG_QUEUE_SEL:       u64 = 0x030;
const REG_QUEUE_NUM_MAX:   u64 = 0x034;
const REG_QUEUE_NUM:       u64 = 0x038;
const REG_QUEUE_ALIGN:     u64 = 0x03c;
const REG_QUEUE_PFN:       u64 = 0x040;
const REG_QUEUE_NOTIFY:    u64 = 0x050;
const REG_STATUS:          u64 = 0x070;

const STATUS_ACKNOWLEDGE: u32 = 1;
const STATUS_DRIVER:      u32 = 2;
const STATUS_DRIVER_OK:   u32 = 4;

const PAGE_SIZE: u64   = 4096;
const NUM_DESC:  usize = 16;

const RX_RING_VADDR: u64 = 0x4020_1000;
const RX_USED_VADDR: u64 = 0x4020_2000;
const RX_BUF_VADDR:  u64 = 0x4020_3000;
const TX_RING_VADDR: u64 = 0x4020_4000;
const TX_PKT_VADDR:  u64 = 0x4020_5000;

const DESC_F_WRITE: u16 = 2;

const GUEST_MAC:   [u8; 6] = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
const GATEWAY_MAC: [u8; 6] = [0x52, 0x55, 0x0a, 0x00, 0x02, 0x02];
const GUEST_IP:    [u8; 4] = [10, 0, 2, 15];
const GATEWAY_IP:  [u8; 4] = [10, 0, 2, 2];
const SRC_PORT:    u16     = 54321;
const DST_PORT:    u16     = 9080;
const ISN:         u32     = 0x12345678;

const ETHERTYPE_IPV4: u16 = 0x0800;
const ETHERTYPE_ARP:  u16 = 0x0806;
const IP_PROTO_TCP:   u8  = 6;
const TCP_SYN:        u8  = 0x02;
const TCP_ACK:        u8  = 0x10;
const TCP_SYN_ACK:    u8  = TCP_SYN | TCP_ACK;
const VIRTIO_NET_HDR: usize = 10;

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

// --- MMIO ---

unsafe fn mmio_r32(base: u64, off: u64) -> u32 { ((base + off) as *const u32).read_volatile() }
unsafe fn mmio_w32(base: u64, off: u64, v: u32) { ((base + off) as *mut u32).write_volatile(v) }

// --- Byte order ---

fn u16_be(v: u16) -> [u8; 2] { [(v >> 8) as u8, (v & 0xff) as u8] }
fn u32_be(v: u32) -> [u8; 4] { [(v>>24) as u8, (v>>16) as u8, (v>>8) as u8, v as u8] }

fn r16_be(b: &[u8], o: usize) -> u16 { ((b[o] as u16) << 8) | b[o+1] as u16 }
fn r32_be(b: &[u8], o: usize) -> u32 {
    ((b[o] as u32)<<24)|((b[o+1] as u32)<<16)|((b[o+2] as u32)<<8)|(b[o+3] as u32)
}

unsafe fn put16le(p: *mut u8, o: usize, v: u16) {
    *p.add(o)   = (v & 0xff) as u8;
    *p.add(o+1) = (v >> 8) as u8;
}
unsafe fn put32le(p: *mut u8, o: usize, v: u32) {
    for i in 0..4 { *p.add(o+i) = ((v >> (i*8)) & 0xff) as u8; }
}
unsafe fn put64le(p: *mut u8, o: usize, v: u64) {
    for i in 0..8 { *p.add(o+i) = ((v >> (i*8)) & 0xff) as u8; }
}
unsafe fn r16le(p: *const u8, o: usize) -> u16 {
    (*p.add(o) as u16) | ((*p.add(o+1) as u16) << 8)
}

// --- Checksum ---

fn oc_sum(d: &[u8]) -> u32 {
    let mut s: u32 = 0;
    let mut i = 0;
    while i + 1 < d.len() { s += ((d[i] as u32) << 8) | d[i+1] as u32; i += 2; }
    if i < d.len() { s += (d[i] as u32) << 8; }
    s
}
fn finish_ck(mut s: u32) -> u16 { while s >> 16 != 0 { s = (s&0xffff)+(s>>16); } !(s as u16) }

fn ip_ck(h: &[u8]) -> u16 { finish_ck(oc_sum(h)) }

fn tcp_ck(seg: &[u8]) -> u16 {
    let len = seg.len() as u32;
    let mut s = oc_sum(&GUEST_IP) + oc_sum(&GATEWAY_IP) + (IP_PROTO_TCP as u32) + len;
    s += oc_sum(seg);
    finish_ck(s)
}

// --- Packet builders ---

fn build_syn(buf: &mut [u8]) -> usize {
    let eth = VIRTIO_NET_HDR; // 10
    let ip  = eth + 14;       // 24
    let tcp = ip  + 20;       // 44
    let end = tcp + 20;       // 64

    for i in 0..eth { buf[i] = 0; }

    buf[eth..eth+6].copy_from_slice(&GATEWAY_MAC);
    buf[eth+6..eth+12].copy_from_slice(&GUEST_MAC);
    buf[eth+12..eth+14].copy_from_slice(&u16_be(ETHERTYPE_IPV4));

    buf[ip]   = 0x45;
    buf[ip+1] = 0;
    buf[ip+2..ip+4].copy_from_slice(&u16_be(40));
    buf[ip+4..ip+6].copy_from_slice(&u16_be(1));
    buf[ip+6..ip+8].copy_from_slice(&u16_be(0));
    buf[ip+8]  = 64;
    buf[ip+9]  = IP_PROTO_TCP;
    buf[ip+10..ip+12].copy_from_slice(&[0,0]);
    buf[ip+12..ip+16].copy_from_slice(&GUEST_IP);
    buf[ip+16..ip+20].copy_from_slice(&GATEWAY_IP);
    let ck = ip_ck(&buf[ip..ip+20]);
    buf[ip+10..ip+12].copy_from_slice(&u16_be(ck));

    buf[tcp..tcp+2].copy_from_slice(&u16_be(SRC_PORT));
    buf[tcp+2..tcp+4].copy_from_slice(&u16_be(DST_PORT));
    buf[tcp+4..tcp+8].copy_from_slice(&u32_be(ISN));
    buf[tcp+8..tcp+12].copy_from_slice(&u32_be(0));
    buf[tcp+12] = 0x50;
    buf[tcp+13] = TCP_SYN;
    buf[tcp+14..tcp+16].copy_from_slice(&u16_be(65535));
    buf[tcp+16..tcp+18].copy_from_slice(&[0,0]);
    buf[tcp+18..tcp+20].copy_from_slice(&[0,0]);
    let tck = tcp_ck(&buf[tcp..end]);
    buf[tcp+16..tcp+18].copy_from_slice(&u16_be(tck));

    end
}

fn build_ack_get(buf: &mut [u8], seq: u32, ack: u32) -> usize {
    const GET: &[u8] = b"GET /healthz HTTP/1.1\r\nHost: 10.0.2.2:9080\r\nConnection: close\r\n\r\n";
    let eth = VIRTIO_NET_HDR;
    let ip  = eth + 14;
    let tcp = ip  + 20;
    let dat = tcp + 20;
    let end = dat + GET.len();

    for i in 0..eth { buf[i] = 0; }

    buf[eth..eth+6].copy_from_slice(&GATEWAY_MAC);
    buf[eth+6..eth+12].copy_from_slice(&GUEST_MAC);
    buf[eth+12..eth+14].copy_from_slice(&u16_be(ETHERTYPE_IPV4));

    buf[ip]   = 0x45;
    buf[ip+1] = 0;
    buf[ip+2..ip+4].copy_from_slice(&u16_be((end - eth - 14) as u16));
    buf[ip+4..ip+6].copy_from_slice(&u16_be(2));
    buf[ip+6..ip+8].copy_from_slice(&u16_be(0));
    buf[ip+8]  = 64;
    buf[ip+9]  = IP_PROTO_TCP;
    buf[ip+10..ip+12].copy_from_slice(&[0,0]);
    buf[ip+12..ip+16].copy_from_slice(&GUEST_IP);
    buf[ip+16..ip+20].copy_from_slice(&GATEWAY_IP);
    let ck = ip_ck(&buf[ip..ip+20]);
    buf[ip+10..ip+12].copy_from_slice(&u16_be(ck));

    buf[tcp..tcp+2].copy_from_slice(&u16_be(SRC_PORT));
    buf[tcp+2..tcp+4].copy_from_slice(&u16_be(DST_PORT));
    buf[tcp+4..tcp+8].copy_from_slice(&u32_be(seq));
    buf[tcp+8..tcp+12].copy_from_slice(&u32_be(ack));
    buf[tcp+12] = 0x50;
    buf[tcp+13] = TCP_ACK;
    buf[tcp+14..tcp+16].copy_from_slice(&u16_be(65535));
    buf[tcp+16..tcp+18].copy_from_slice(&[0,0]);
    buf[tcp+18..tcp+20].copy_from_slice(&[0,0]);
    buf[dat..dat+GET.len()].copy_from_slice(GET);
    let tck = tcp_ck(&buf[tcp..end]);
    buf[tcp+16..tcp+18].copy_from_slice(&u16_be(tck));

    end
}

// Build ARP reply into buf, given the ARP request frame (with VirtIO header) in req.
// req layout: [vnet_hdr(10)] [eth_dst(6) eth_src(6) ethertype(2)] [arp(28)]
fn build_arp_reply(buf: &mut [u8], req: &[u8]) -> usize {
    let eth = VIRTIO_NET_HDR; // 10
    let arp = eth + 14;       // 24

    for i in 0..eth { buf[i] = 0; } // VirtIO-net header

    // Ethernet: reply to request sender
    buf[eth..eth+6].copy_from_slice(&req[eth+6..eth+12]); // dst = request src MAC
    buf[eth+6..eth+12].copy_from_slice(&GUEST_MAC);        // src = our MAC
    buf[eth+12] = 0x08; buf[eth+13] = 0x06;                // EtherType = ARP

    // ARP reply
    buf[arp]   = 0x00; buf[arp+1] = 0x01; // hardware type Ethernet
    buf[arp+2] = 0x08; buf[arp+3] = 0x00; // protocol type IPv4
    buf[arp+4] = 6;                         // hw addr len
    buf[arp+5] = 4;                         // proto addr len
    buf[arp+6] = 0x00; buf[arp+7] = 0x02;  // opcode = reply
    buf[arp+8..arp+14].copy_from_slice(&GUEST_MAC);          // sender MAC = us
    buf[arp+14..arp+18].copy_from_slice(&GUEST_IP);          // sender IP  = us
    buf[arp+18..arp+24].copy_from_slice(&req[arp+8..arp+14]); // target MAC = request sender
    buf[arp+24..arp+28].copy_from_slice(&req[arp+14..arp+18]); // target IP  = request sender

    52
}

fn is_arp_for_us(f: &[u8], len: usize) -> bool {
    if len < 52 { return false; }
    let eth = VIRTIO_NET_HDR;
    let arp = eth + 14;
    r16_be(f, eth+12) == ETHERTYPE_ARP
        && r16_be(f, arp+6) == 1              // opcode = request
        && f[arp+24..arp+28] == GUEST_IP       // target IP = us
}

fn parse_tcp_synack(f: &[u8], len: usize) -> Option<u32> {
    if len < VIRTIO_NET_HDR + 14 + 20 + 20 { return None; }
    let eth = VIRTIO_NET_HDR;
    if r16_be(f, eth+12) != ETHERTYPE_IPV4 { return None; }
    if f[eth+6..eth+12] != GATEWAY_MAC { return None; } // src must be gateway
    let ip  = eth + 14;
    if f[ip+9] != IP_PROTO_TCP { return None; }
    let ihl = ((f[ip] & 0x0f) as usize) * 4;
    let tcp = ip + ihl;
    if len < tcp + 20 { return None; }
    if r16_be(f, tcp+2) != SRC_PORT { return None; }  // dst port = our port
    if f[tcp+13] != TCP_SYN_ACK { return None; }
    Some(r32_be(f, tcp+4)) // server SEQ
}

// --- Print ---
unsafe fn put_hex32(v: u32) {
    write_bytes(b"0x");
    for i in (0u32..8).rev() {
        let n = ((v >> (i*4)) & 0xf) as u8;
        putchar(if n < 10 { b'0'+n } else { b'a'+n-10 });
    }
    write_bytes(b"\r\n");
}

// --- Virtqueue helpers ---

unsafe fn vq_write_desc(vq: *mut u8, idx: usize, addr: u64, len: u32, flags: u16) {
    let o = idx * 16;
    put64le(vq, o,    addr);
    put32le(vq, o+8,  len);
    put16le(vq, o+12, flags);
    put16le(vq, o+14, 0);
}

// avail ring starts at offset NUM_DESC*16 = 256.
// Call with (ring, current_avail_idx, desc_id): sets ring[current & mask]=desc, idx=current+1.
unsafe fn vq_avail_offer(vq: *mut u8, cur_idx: u16, desc: u16) {
    let base = NUM_DESC * 16;
    put16le(vq, base+0, 0);
    put16le(vq, base+2, cur_idx.wrapping_add(1));
    let slot = (cur_idx as usize) & (NUM_DESC-1);
    put16le(vq, base+4+slot*2, desc);
}

// Poll used ring at used_vbase until idx > last. Returns (elem_id, elem_len).
unsafe fn vq_poll_used(used_vbase: *mut u8, last: u16) -> (u16, u32) {
    loop {
        fence(Ordering::Acquire);
        let cur = r16le(used_vbase as *const u8, 2);
        if cur != last {
            let o = 4 + ((last as usize) & (NUM_DESC-1)) * 8;
            let id  = (*(used_vbase.add(o) as *const u32)).to_le();
            let len = (*(used_vbase.add(o+4) as *const u32)).to_le();
            return (id as u16, len);
        }
    }
}

// Offer a fresh RX descriptor and notify the device.
unsafe fn rx_offer(rx_ring: *mut u8, rx_buf_paddr: u64, rx_avail: u16, slot31: u64) {
    vq_write_desc(rx_ring, 0, rx_buf_paddr, 2048, DESC_F_WRITE);
    vq_avail_offer(rx_ring, rx_avail, 0);
    fence(Ordering::Release);
    mmio_w32(slot31, REG_QUEUE_NOTIFY, 0);
}

// Send a packet from tx_pkt and wait for TX used confirmation.
unsafe fn tx_send(
    tx_ring: *mut u8, tx_used: *mut u8, tx_pkt_paddr: u64,
    tx_avail: u16, tx_used_idx: u16, len: usize, slot31: u64,
) {
    vq_write_desc(tx_ring, 0, tx_pkt_paddr, len as u32, 0);
    vq_avail_offer(tx_ring, tx_avail, 0);
    fence(Ordering::Release);
    mmio_w32(slot31, REG_QUEUE_NOTIFY, 1);
    vq_poll_used(tx_used, tx_used_idx);
}

#[no_mangle]
pub unsafe extern "C" fn _main(bi: *const BootInfo) -> ! {
    write_bytes(b"[h8] Phase H8 start\r\n");

    let ipc         = (*bi).ipc_buffer;
    let empty_start = (*bi).empty.start;

    let ut_ram = match (*bi).find_untyped(13) {
        Some(s) => s,
        None => { write_bytes(b"[h8] no RAM untyped\r\n"); spin() }
    };
    let ut_dev = match (*bi).find_device_untyped(0x0a00_0000) {
        Some(s) => s,
        None => { write_bytes(b"[h8] no device untyped\r\n"); spin() }
    };

    // Cap slots.
    let pt1         = empty_start;
    let pt2         = empty_start + 1;
    let dev_f0      = empty_start + 2;
    let dev_f1      = empty_start + 3;
    let dev_f2      = empty_start + 4;
    let dev_f3      = empty_start + 5;
    let ram_rx_ring = empty_start + 6;
    let ram_rx_used = empty_start + 7;
    let ram_rx_buf  = empty_start + 8;
    let ram_tx_ring = empty_start + 9;
    let ram_tx_pkt  = empty_start + 10;

    // Retype.
    bootstrap::untyped_retype(ipc, ut_ram, obj_type::ARM_PAGE_TABLE, bootstrap::PT_SIZE_BITS, pt1);
    bootstrap::untyped_retype(ipc, ut_ram, obj_type::ARM_PAGE_TABLE, bootstrap::PT_SIZE_BITS, pt2);
    bootstrap::untyped_retype(ipc, ut_dev, obj_type::ARM_SMALL_PAGE, bootstrap::PAGE_SIZE_BITS, dev_f0);
    bootstrap::untyped_retype(ipc, ut_dev, obj_type::ARM_SMALL_PAGE, bootstrap::PAGE_SIZE_BITS, dev_f1);
    bootstrap::untyped_retype(ipc, ut_dev, obj_type::ARM_SMALL_PAGE, bootstrap::PAGE_SIZE_BITS, dev_f2);
    bootstrap::untyped_retype(ipc, ut_dev, obj_type::ARM_SMALL_PAGE, bootstrap::PAGE_SIZE_BITS, dev_f3);
    bootstrap::untyped_retype(ipc, ut_ram, obj_type::ARM_SMALL_PAGE, bootstrap::PAGE_SIZE_BITS, ram_rx_ring);
    bootstrap::untyped_retype(ipc, ut_ram, obj_type::ARM_SMALL_PAGE, bootstrap::PAGE_SIZE_BITS, ram_rx_used);
    bootstrap::untyped_retype(ipc, ut_ram, obj_type::ARM_SMALL_PAGE, bootstrap::PAGE_SIZE_BITS, ram_rx_buf);
    bootstrap::untyped_retype(ipc, ut_ram, obj_type::ARM_SMALL_PAGE, bootstrap::PAGE_SIZE_BITS, ram_tx_ring);
    bootstrap::untyped_retype(ipc, ut_ram, obj_type::ARM_SMALL_PAGE, bootstrap::PAGE_SIZE_BITS, ram_tx_pkt);
    write_bytes(b"[h8] objects created\r\n");

    // Page table setup.
    bootstrap::page_table_map(ipc, pt1, cap::INIT_VSPACE, VIRTIO_HI_VADDR, 0);
    bootstrap::page_table_map(ipc, pt2, cap::INIT_VSPACE, VIRTIO_HI_VADDR, 0);
    bootstrap::page_map(ipc, dev_f3,      cap::INIT_VSPACE, 0x4020_0000, 3, 0);
    bootstrap::page_map(ipc, ram_rx_ring, cap::INIT_VSPACE, RX_RING_VADDR, 3, 0);
    bootstrap::page_map(ipc, ram_rx_used, cap::INIT_VSPACE, RX_USED_VADDR, 3, 0);
    bootstrap::page_map(ipc, ram_rx_buf,  cap::INIT_VSPACE, RX_BUF_VADDR, 3, 0);
    bootstrap::page_map(ipc, ram_tx_ring, cap::INIT_VSPACE, TX_RING_VADDR, 3, 0);
    bootstrap::page_map(ipc, ram_tx_pkt,  cap::INIT_VSPACE, TX_PKT_VADDR, 3, 0);
    write_bytes(b"[h8] pages mapped\r\n");

    // Physical addresses.
    let ut_idx    = (ut_ram - (*bi).untyped.start) as usize;
    let ut_paddr  = (*bi).untyped_list[ut_idx].paddr;
    let rx_ring_paddr = ut_paddr + 0x2000; // object 2 from RAM untyped
    let rx_buf_paddr  = ut_paddr + 0x4000; // object 4
    let tx_ring_paddr = ut_paddr + 0x5000; // object 5
    let tx_pkt_paddr  = ut_paddr + 0x6000; // object 6

    write_bytes(b"[h8] rx_ring="); put_hex32(rx_ring_paddr as u32);
    write_bytes(b"[h8] tx_ring="); put_hex32(tx_ring_paddr as u32);

    // Zero all ring and buffer pages.
    ptr::write_bytes(RX_RING_VADDR as *mut u8, 0, PAGE_SIZE as usize);
    ptr::write_bytes(RX_USED_VADDR as *mut u8, 0, PAGE_SIZE as usize);
    ptr::write_bytes(RX_BUF_VADDR  as *mut u8, 0, PAGE_SIZE as usize);
    ptr::write_bytes(TX_RING_VADDR as *mut u8, 0, PAGE_SIZE as usize);
    ptr::write_bytes(TX_PKT_VADDR  as *mut u8, 0, PAGE_SIZE as usize);

    // VirtIO init.
    let slot31 = VIRTIO_HI_VADDR + SLOT31_OFFSET;
    let magic  = mmio_r32(slot31, REG_MAGIC);
    let dev_id = mmio_r32(slot31, REG_DEVICE_ID);
    if magic != 0x7472_6976 || dev_id != 1 {
        write_bytes(b"[h8] bad VirtIO\r\n"); spin()
    }

    mmio_w32(slot31, REG_STATUS, 0);
    mmio_w32(slot31, REG_STATUS, STATUS_ACKNOWLEDGE);
    mmio_w32(slot31, REG_STATUS, STATUS_ACKNOWLEDGE | STATUS_DRIVER);
    mmio_w32(slot31, REG_GUEST_PAGE_SIZE, PAGE_SIZE as u32);

    // receiveq (queue 0)
    mmio_w32(slot31, REG_QUEUE_SEL, 0);
    let rx_qmax = mmio_r32(slot31, REG_QUEUE_NUM_MAX);
    mmio_w32(slot31, REG_QUEUE_NUM, rx_qmax.min(NUM_DESC as u32));
    mmio_w32(slot31, REG_QUEUE_ALIGN, PAGE_SIZE as u32);
    mmio_w32(slot31, REG_QUEUE_PFN, (rx_ring_paddr >> 12) as u32);

    // transmitq (queue 1)
    mmio_w32(slot31, REG_QUEUE_SEL, 1);
    let tx_qmax = mmio_r32(slot31, REG_QUEUE_NUM_MAX);
    mmio_w32(slot31, REG_QUEUE_NUM, tx_qmax.min(NUM_DESC as u32));
    mmio_w32(slot31, REG_QUEUE_ALIGN, PAGE_SIZE as u32);
    mmio_w32(slot31, REG_QUEUE_PFN, (tx_ring_paddr >> 12) as u32);

    mmio_w32(slot31, REG_STATUS, STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_DRIVER_OK);
    write_bytes(b"[h8] STATUS="); put_hex32(mmio_r32(slot31, REG_STATUS));

    let rx_ring = RX_RING_VADDR as *mut u8;
    let rx_used = RX_USED_VADDR as *mut u8;
    let tx_ring = TX_RING_VADDR as *mut u8;
    let tx_used = TX_PKT_VADDR  as *mut u8; // TX used ring is at tx_ring_paddr+4096 = TX_PKT_VADDR
    let tx_pkt  = TX_PKT_VADDR  as *mut u8;
    let rx_buf  = RX_BUF_VADDR  as *mut u8;

    // Index trackers.
    let mut rx_avail:    u16 = 0;
    let mut rx_used_idx: u16 = 0;
    let mut tx_avail:    u16 = 0;
    let mut tx_used_idx: u16 = 0;

    // Offer RX descriptor 0 (device will write incoming frames here).
    rx_offer(rx_ring, rx_buf_paddr, rx_avail, slot31);
    rx_avail = rx_avail.wrapping_add(1);

    // Send TCP SYN.
    {
        let buf = core::slice::from_raw_parts_mut(tx_pkt, 2048);
        let len = build_syn(buf);
        tx_send(tx_ring, tx_used, tx_pkt_paddr, tx_avail, tx_used_idx, len, slot31);
    }
    tx_avail    = tx_avail.wrapping_add(1);
    tx_used_idx = tx_used_idx.wrapping_add(1);
    write_bytes(b"[h8] SYN sent\r\n");

    // Receive loop: handle ARP requests; break on SYN-ACK.
    let server_seq: u32;
    loop {
        let (_, frame_len) = vq_poll_used(rx_used, rx_used_idx);
        rx_used_idx = rx_used_idx.wrapping_add(1);

        let frame = core::slice::from_raw_parts(rx_buf as *const u8, frame_len as usize);

        if is_arp_for_us(frame, frame_len as usize) {
            write_bytes(b"[h8] ARP request received\r\n");
            // Send ARP reply.
            ptr::write_bytes(tx_pkt, 0, 2048);
            let buf = core::slice::from_raw_parts_mut(tx_pkt, 52);
            build_arp_reply(buf, frame);
            tx_send(tx_ring, tx_used, tx_pkt_paddr, tx_avail, tx_used_idx, 52, slot31);
            tx_avail    = tx_avail.wrapping_add(1);
            tx_used_idx = tx_used_idx.wrapping_add(1);
            // Re-offer RX.
            ptr::write_bytes(rx_buf, 0, 2048);
            rx_offer(rx_ring, rx_buf_paddr, rx_avail, slot31);
            rx_avail = rx_avail.wrapping_add(1);
            continue;
        }

        if let Some(seq) = parse_tcp_synack(frame, frame_len as usize) {
            server_seq = seq;
            write_bytes(b"[h8] SYN-ACK! server_seq="); put_hex32(server_seq);
            break;
        }

        // Unknown frame — re-offer RX and keep waiting.
        write_bytes(b"[h8] unknown frame len="); put_hex32(frame_len);
        ptr::write_bytes(rx_buf, 0, 2048);
        rx_offer(rx_ring, rx_buf_paddr, rx_avail, slot31);
        rx_avail = rx_avail.wrapping_add(1);
    }

    // Send TCP ACK + HTTP GET.
    let my_seq = ISN.wrapping_add(1);
    let my_ack = server_seq.wrapping_add(1);
    {
        ptr::write_bytes(tx_pkt, 0, 2048);
        let buf = core::slice::from_raw_parts_mut(tx_pkt, 2048);
        let len = build_ack_get(buf, my_seq, my_ack);
        tx_send(tx_ring, tx_used, tx_pkt_paddr, tx_avail, tx_used_idx, len, slot31);
    }
    tx_avail    = tx_avail.wrapping_add(1);
    tx_used_idx = tx_used_idx.wrapping_add(1);
    write_bytes(b"[h8] ACK+GET sent\r\n");

    // Re-offer RX for HTTP response.
    ptr::write_bytes(rx_buf, 0, 2048);
    rx_offer(rx_ring, rx_buf_paddr, rx_avail, slot31);
    rx_avail = rx_avail.wrapping_add(1);

    // Receive loop: find HTTP response.
    let mut found = false;
    for _ in 0..20u32 {
        let (_, frame_len) = vq_poll_used(rx_used, rx_used_idx);
        rx_used_idx = rx_used_idx.wrapping_add(1);
        let frame = core::slice::from_raw_parts(rx_buf as *const u8, frame_len as usize);
        write_bytes(b"[h8] rx len="); put_hex32(frame_len);

        // Scan for "HTTP" past the network headers.
        let scan_start = (VIRTIO_NET_HDR + 14 + 20 + 20).min(frame_len as usize);
        let payload = &frame[scan_start..];
        for i in 0..payload.len().saturating_sub(3) {
            if &payload[i..i+4] == b"HTTP" {
                found = true;
                write_bytes(b"[h8] response: ");
                for &b in &payload[..payload.len().min(60)] {
                    if b.is_ascii_graphic() || b == b' ' { putchar(b); }
                    else if b == b'\r' { } // skip CR
                    else if b == b'\n' { write_bytes(b" "); }
                }
                write_bytes(b"\r\n");
                break;
            }
        }
        if found { break; }

        // Re-offer RX and keep waiting (might receive TCP ACK from server first).
        ptr::write_bytes(rx_buf, 0, 2048);
        rx_offer(rx_ring, rx_buf_paddr, rx_avail, slot31);
        rx_avail = rx_avail.wrapping_add(1);
    }

    if found {
        write_bytes(b"HTTP GET gate: PASSED\r\n");
    } else {
        write_bytes(b"HTTP GET gate: INCOMPLETE\r\n");
    }

    spin()
}
