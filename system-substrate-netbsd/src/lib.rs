/// NetBSD 10.1 — canonical compat-bottom version for all os-* image builders.
/// Image builds use NetBSD's own `build.sh tools` to get `nbmakefs` + `nbinstallboot`
/// running on the Ubuntu GCP host. FFS2 is the canonical filesystem.
/// Veriexec provides OS-level binary signature verification at runtime.
pub const NETBSD_VERSION: &str = "10.1";
pub const NETBSD_ARCH: &str = "amd64";
pub const NETBSD_SETS_BASE_URL: &str =
    "https://cdn.netbsd.org/pub/NetBSD/NetBSD-10.1/amd64/binary/sets";

/// Veriexec entry: path + SHA-256 hex digest + flags.
/// Serialised as `path SHA-256 VERIEXEC_DIRECT` lines in `/etc/signatures`.
pub struct VeriexecEntry {
    pub path: &'static str,
    pub sha256_hex: &'static str,
    pub flags: &'static str,
}

/// Well-known binaries installed by os-totebox.
/// Entries are populated by `scripts/build-image.sh` at image build time.
pub const OS_TOTEBOX_BINARIES: &[&str] = &[
    "/usr/bin/system-ledger-server",
    "/usr/bin/slm-doorman-server",
    "/usr/bin/service-content",
    "/usr/bin/llama-server",
];

/// Well-known binaries installed by os-orchestration.
pub const OS_ORCHESTRATION_BINARIES: &[&str] =
    &["/usr/bin/orchestration-slm-server", "/usr/sbin/nginx"];
