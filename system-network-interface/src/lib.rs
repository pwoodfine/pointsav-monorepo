#![no_std]

// Discovery constants
pub const GCP_RELAY_IP: [u8; 4] = [34, 53, 65, 203]; // static; reserved 2026-05-30
pub const MDNS_MULTICAST: [u8; 4] = [224, 0, 0, 251]; // RFC 6762 §2
pub const MDNS_PORT: u16 = 5353; // RFC 6762 §2
pub const PAIRING_PORT: u16 = 9205; // service-ppn-pairing HTTP API

// PPN mDNS service label for DNS-SD PTR queries (RFC 6763 §4.1).
// Query: PTR _ppn._udp.local → resolves to individual node SRV records.
pub const PPN_SERVICE_LABEL: &[u8] = b"\x04_ppn\x04_udp\x05local\x00";

/// DNS message header (RFC 1035 §4.1.1, adapted for mDNS RFC 6762 §18).
/// 12 bytes: ID(2) + FLAGS(2) + QDCOUNT(2) + ANCOUNT(2) + NSCOUNT(2) + ARCOUNT(2).
#[repr(C)]
pub struct MdnsHeader {
    pub transaction_id: u16, // 0x0000 for mDNS queries
    pub flags: u16,          // 0x0000 = standard query, recursion not desired
    pub qdcount: u16,        // question count (1 for our PTR query)
    pub ancount: u16,        // answer count (0 in query)
    pub nscount: u16,        // authority count (0)
    pub arcount: u16,        // additional count (0)
}

impl MdnsHeader {
    pub const fn ppn_query() -> Self {
        MdnsHeader {
            transaction_id: 0x0000,
            flags: 0x0000,
            qdcount: 0x0001,
            ancount: 0x0000,
            nscount: 0x0000,
            arcount: 0x0000,
        }
    }
}

// Genesis Protocol handshake frame (32 bytes).
// Sent over UDP to the pairing server once peer address is known.
// The short_code is the 8-character Crockford base32 SAS string (RFC 9382 §3 SAS derivation).
#[repr(C)]
pub struct GenesisHandshakeFrame {
    pub magic: [u8; 4],      // b"GNES" — Genesis Network Establishment Sequence
    pub short_code: [u8; 8], // Crockford base32 SAS (e.g. b"AAAA-BBBB" without hyphen)
    pub node_id: [u8; 16],   // UUID v4 of this node (128-bit)
    pub reserved: [u8; 4],   // zero-filled; for future protocol fields
}

impl GenesisHandshakeFrame {
    pub const MAGIC: [u8; 4] = *b"GNES";
}

/// CPace PAKE (RFC 9382) key exchange protocol — step reference for implementation.
///
/// Initiator (joining node) flow:
///   M1: generate random scalar `a`; compute `A = a * G` on Ristretto255; send A to responder
///   M2: receive `B = b * G` from responder; compute `K = a * B` (session key)
///   SAS: derive 5 bytes from K via HKDF-SHA256 → encode as 8-char Crockford base32
///
/// The SAS is displayed on the framebuffer (HOLD phase 0x40) and entered by the operator
/// on app-console-keys (F12 panel) to complete the pairing ceremony.
///
/// Full spec: RFC 9382 §3 (CPace), §5 (SAS derivation), §6 (implementation guidance).
/// Implementation requires: Ristretto255 scalar multiplication, HKDF-SHA256.
/// Both require a working NIC for key transport — deferred to Step 3-full (NIC driver).
pub const CPACE_PROTOCOL_NOTE: &str = "CPace PAKE RFC 9382; pending NIC driver";

/// State of the node-join pairing ceremony from the joining node's perspective.
pub enum PairingCeremonyState {
    AwaitingApproval,
    Approved,
    Denied,
    TimedOut,
}

/// Result of a peer-discovery scan on the local broadcast domain.
pub enum DiscoveryResult {
    /// A PPN peer was found via mDNS zero-config discovery.
    MdnsFound { addr: [u8; 4] },
    /// A peer address was supplied by the operator (fallback for cloud-VM / cross-VLAN).
    OperatorSupplied { addr: [u8; 4] },
    /// No peers found; this node is the genesis seed.
    NotFound,
}

/// Scan for existing PPN peers via mDNS (RFC 6762).
///
/// Phase 1 — mDNS multicast (NIC driver required):
///   Transmit DNS-SD PTR query for `_ppn._udp.local` to MDNS_MULTICAST:MDNS_PORT.
///   Wait up to 2 seconds for a PTR response carrying a peer's A record.
///   Returns MdnsFound { addr } on first valid response.
///
/// Phase 2 — operator-supplied fallback (cloud-VM / cross-VLAN):
///   When Phase 1 times out and a relay address is pre-configured (e.g. GCP_RELAY_IP),
///   returns OperatorSupplied { addr: GCP_RELAY_IP }.
///
/// Returns NotFound (genesis-seed path) when neither phase succeeds.
/// Stub: NIC driver not yet implemented; returns NotFound until Step 3-full lands.
pub fn scan_for_peers() -> DiscoveryResult {
    // TODO Phase 1: init NIC from system_substrate_broadcom::probe_nic() MMIO base
    // TODO Phase 1: build MdnsHeader::ppn_query() + PPN_SERVICE_LABEL question section
    // TODO Phase 1: transmit UDP frame to MDNS_MULTICAST:MDNS_PORT via NIC MMIO
    // TODO Phase 1: poll RX ring for up to 2s; parse PTR response → extract A record
    // TODO Phase 2: if Phase 1 times out → return OperatorSupplied { addr: GCP_RELAY_IP }
    DiscoveryResult::NotFound
}

/// Send a Genesis Protocol handshake frame to the pairing server at `peer_addr`.
///
/// Constructs a GenesisHandshakeFrame with the 8-char Crockford base32 `short_code`
/// and transmits it via UDP to `peer_addr:PAIRING_PORT`.
///
/// CPace PAKE exchange (RFC 9382 §3) follows the frame transmission:
///   Initiator M1 (A = a*G) → Responder M2 (B = b*G) → session key K = a*B
///   SAS = HKDF-SHA256(K, "ppn-genesis-sas", 5 bytes) → 8-char Crockford base32
///
/// Returns true if the pairing server acknowledged the frame; false on timeout or TX failure.
/// Stub: returns false until NIC driver enables UDP transmission.
pub fn send_genesis_handshake(_peer_addr: [u8; 4], short_code: &[u8]) -> bool {
    let mut frame = GenesisHandshakeFrame {
        magic: GenesisHandshakeFrame::MAGIC,
        short_code: [0u8; 8],
        node_id: [0u8; 16], // TODO: derive from WireGuard pubkey hash
        reserved: [0u8; 4],
    };
    let copy_len = if short_code.len() < 8 {
        short_code.len()
    } else {
        8
    };
    frame.short_code[..copy_len].copy_from_slice(&short_code[..copy_len]);
    // TODO: transmit frame via NIC MMIO to _peer_addr:PAIRING_PORT
    let _ = frame;
    false
}

/// Conduct the full node-join pairing ceremony with the pairing server.
///
/// Requires an active NIC and TCP/IP stack. Flow (service-ppn-pairing HTTP API at :9205):
///   1. POST /v1/node-join/request → NodeJoinResponseBody { request_id, code, expires_at }
///      Request body: NodeJoinRequestBody { node_id, wireguard_pubkey, bottom, arch }
///      Wire types: system-pairing-codes/src/lib.rs
///   2. Display `code` on framebuffer (HOLD phase 0x40) — operator enters on
///      app-console-keys F12 panel via app-network-admin /authorize route.
///   3. Poll GET /v1/node-join/status/{request_id} every 5 seconds for up to 600 seconds.
///   4. Return PairingCeremonyState based on `state` field: "approved" / "denied" / timeout.
///
/// Stub: returns TimedOut until NIC driver enables HTTP over raw Ethernet.
pub fn conduct_pairing_ceremony(_peer_addr: [u8; 4]) -> PairingCeremonyState {
    // TODO Step 3-full: initialize NIC via system_substrate_broadcom::probe_nic()
    // TODO Step 3-full: implement ARP + IP + TCP + HTTP over BCM57765 MMIO
    // TODO then: POST _peer_addr:PAIRING_PORT/v1/node-join/request
    // TODO then: poll /v1/node-join/status/{request_id} every 5s, max 600s
    PairingCeremonyState::TimedOut
}

pub fn system_status() -> &'static str {
    "system-network-interface: skeleton (NIC driver pending)"
}
