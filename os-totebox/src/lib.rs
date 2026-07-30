/// os-totebox — NetBSD 10.1 compat-bottom image builder metadata.
///
/// The guest VM image for `vm-intelligence` (8 GiB RAM, OLMo 7B, Doorman,
/// `service-content`, `system-ledger-server`). Built with `scripts/build-image.sh`
/// which uses NetBSD cross tools (`nbmakefs`, `nbinstallboot`) on the GCP Ubuntu host.
/// OLMo weights live on a separate data QCOW2 (`scripts/provision-data-disk.sh`).

/// NetBSD version pinned for this image.
pub const NETBSD_VERSION: &str = "10.1";

/// Target architecture for the guest image.
pub const TARGET_ARCH: &str = "amd64";

/// Base image size (root filesystem QCOW2).
pub const BASE_IMAGE_SIZE: &str = "4g";

/// Data disk size for OLMo 7B weights and cluster-totebox mounts.
pub const DATA_DISK_SIZE: &str = "8g";

/// Unix socket path for the capability ledger daemon.
pub const LEDGER_SOCK: &str = "/run/system-ledger/ledger.sock";

/// Doorman listen address (SLM routing).
pub const DOORMAN_ADDR: &str = "127.0.0.1:8011";

/// llama-server listen address for OLMo 7B inference.
pub const LLAMA_SERVER_ADDR: &str = "127.0.0.1:11434";

/// WireGuard interface name on the guest.
pub const WG_INTERFACE: &str = "wg0";

/// PPN address assigned to vm-intelligence.
pub const PPN_ADDRESS: &str = "10.8.0.7/24";
