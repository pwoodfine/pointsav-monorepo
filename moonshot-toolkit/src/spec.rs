//! SystemSpec — Rust-native equivalent of Microkit 2.2.0's
//! system-description XML schema. Parsed from TOML on disk.
//!
//! Per `~/Foundry/conventions/system-substrate-doctrine.md` §6
//! and `https://docs.sel4.systems/projects/microkit/manual/latest/`.
//!
//! # Schema summary
//!
//! - **Protection Domains** (PDs): isolated single-threaded
//!   components; ≤ 63 per system per Microkit limits.
//! - **Channels**: point-to-point PPC or notification between PDs;
//!   ≤ 63 per PD.
//! - **Memory Regions**: physical regions with caching + permissions
//!   + optional prefill.
//! - **IRQ Delivery**: hardware IRQ → PD mapping.
//!
//! Validation at parse time: counts ≤ limits; references resolve;
//! memory regions don't overlap.

use serde::{Deserialize, Serialize};

/// Microkit hard limit on protection domains per system.
pub const MAX_PROTECTION_DOMAINS: usize = 63;
/// Microkit hard limit on channels per protection domain.
pub const MAX_CHANNELS_PER_PD: usize = 63;

/// Build configuration consumed by `moonshot-toolkit build`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildConfig {
    /// Microkit board identifier (e.g. `x86_64_generic`, `qemu_virt_aarch64`).
    #[serde(default)]
    pub board: String,
    /// Microkit build configuration (`debug` or `release`).
    #[serde(default = "default_config")]
    pub config: String,
    /// Path to the Microkit 2.2.0 SDK root.
    #[serde(default = "default_sdk")]
    pub sdk: String,
    /// Directory for build outputs relative to the spec file. Default: `build`.
    #[serde(default = "default_output_dir")]
    pub output_dir: String,
}

fn default_config() -> String {
    "debug".to_string()
}
fn default_sdk() -> String {
    "/opt/microkit-sdk-2.2.0".to_string()
}
fn default_output_dir() -> String {
    "build".to_string()
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            board: String::new(),
            config: default_config(),
            sdk: default_sdk(),
            output_dir: default_output_dir(),
        }
    }
}

/// Top-level system specification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemSpec {
    /// Protection domains (isolated components). Length ≤ 63.
    #[serde(default)]
    pub protection_domains: Vec<ProtectionDomain>,
    /// Inter-PD channels (PPC or notification). Length ≤ 63 per PD.
    #[serde(default)]
    pub channels: Vec<Channel>,
    /// Physical memory regions.
    #[serde(default)]
    pub memory_regions: Vec<MemoryRegion>,
    /// Hardware IRQ → PD bindings.
    #[serde(default)]
    pub irq_delivery: Vec<IrqDelivery>,
    /// Build configuration for `moonshot-toolkit build`.
    #[serde(default)]
    pub build: BuildConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtectionDomain {
    pub name: String,
    /// For C PDs: path to the `.c` source file compiled by aarch64-linux-gnu-gcc.
    /// For Rust PDs (when `rust_bin` is set): the crate directory name
    /// (e.g. `"moonshot-sel4-vmm"`); the crate's Cargo.toml is resolved
    /// as `<binary>/Cargo.toml` relative to the monorepo root.
    pub binary: String,
    /// When set, this PD is a Rust `[[bin]]` target compiled with
    /// `cargo build --target aarch64-unknown-none --release`.
    /// Value is the `--bin` target name (e.g. `"console_main"`).
    /// `binary` is interpreted as the crate directory name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rust_bin: Option<String>,
    /// Scheduling priority (0 = highest; matches Microkit / seL4).
    #[serde(default)]
    pub priority: u8,
    /// Stack size in bytes. Default Microkit value: 4 KiB.
    #[serde(default = "default_stack_bytes")]
    pub stack_bytes: u64,
}

fn default_stack_bytes() -> u64 {
    4096
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Channel {
    pub name: String,
    /// First endpoint PD name.
    pub end_a: String,
    /// Second endpoint PD name.
    pub end_b: String,
    /// `Ppc` for protected-procedure-call (synchronous);
    /// `Notification` for asynchronous signal.
    pub kind: ChannelKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChannelKind {
    Ppc,
    Notification,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryRegion {
    pub name: String,
    /// Physical base address.
    pub phys_addr: u64,
    /// Size in bytes.
    pub size_bytes: u64,
    /// Caching policy.
    #[serde(default)]
    pub caching: CachingPolicy,
    /// Permissions (read/write/execute, composable).
    #[serde(default)]
    pub permissions: Vec<Permission>,
    /// Optional path to a binary blob to prefill this region.
    pub prefill_from: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CachingPolicy {
    #[default]
    Cached,
    Uncached,
    DeviceMemory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Permission {
    Read,
    Write,
    Execute,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IrqDelivery {
    /// Hardware IRQ number.
    pub irq: u32,
    /// Target PD name.
    pub target_pd: String,
    /// Optional channel ID inside the PD.
    pub target_channel: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecParseError {
    /// TOML parsing failed.
    TomlError(String),
    /// More than [`MAX_PROTECTION_DOMAINS`] PDs declared.
    TooManyProtectionDomains { actual: usize },
    /// PD declares more than [`MAX_CHANNELS_PER_PD`] channels.
    TooManyChannelsForPd { pd_name: String, actual: usize },
    /// Two memory regions overlap.
    OverlappingMemoryRegions { name_a: String, name_b: String },
    /// Channel endpoint references a PD that wasn't declared.
    UnknownChannelEndpoint {
        channel_name: String,
        endpoint: String,
    },
    /// IRQ delivery target references a PD that wasn't declared.
    UnknownIrqTarget { irq: u32, target_pd: String },
    /// Two PDs share a name.
    DuplicatePdName(String),
}

impl SystemSpec {
    /// Parse a `system-spec.toml` from text. Validates all
    /// invariants; returns the first failure encountered.
    pub fn from_toml_str(text: &str) -> Result<Self, SpecParseError> {
        let spec: SystemSpec =
            toml::from_str(text).map_err(|e| SpecParseError::TomlError(e.to_string()))?;
        spec.validate()?;
        Ok(spec)
    }

    /// Validate all invariants. Called automatically by
    /// [`SystemSpec::from_toml_str`]; can be called manually after programmatic
    /// construction.
    pub fn validate(&self) -> Result<(), SpecParseError> {
        // PD count.
        if self.protection_domains.len() > MAX_PROTECTION_DOMAINS {
            return Err(SpecParseError::TooManyProtectionDomains {
                actual: self.protection_domains.len(),
            });
        }

        // Duplicate PD names.
        let mut seen = std::collections::HashSet::new();
        for pd in &self.protection_domains {
            if !seen.insert(pd.name.as_str()) {
                return Err(SpecParseError::DuplicatePdName(pd.name.clone()));
            }
        }

        // Channels-per-PD count.
        let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for ch in &self.channels {
            *counts.entry(ch.end_a.as_str()).or_insert(0) += 1;
            *counts.entry(ch.end_b.as_str()).or_insert(0) += 1;
        }
        for (pd, count) in &counts {
            if *count > MAX_CHANNELS_PER_PD {
                return Err(SpecParseError::TooManyChannelsForPd {
                    pd_name: (*pd).to_string(),
                    actual: *count,
                });
            }
        }

        // Channel endpoints reference declared PDs.
        let pd_names: std::collections::HashSet<&str> = self
            .protection_domains
            .iter()
            .map(|p| p.name.as_str())
            .collect();
        for ch in &self.channels {
            if !pd_names.contains(ch.end_a.as_str()) {
                return Err(SpecParseError::UnknownChannelEndpoint {
                    channel_name: ch.name.clone(),
                    endpoint: ch.end_a.clone(),
                });
            }
            if !pd_names.contains(ch.end_b.as_str()) {
                return Err(SpecParseError::UnknownChannelEndpoint {
                    channel_name: ch.name.clone(),
                    endpoint: ch.end_b.clone(),
                });
            }
        }

        // IRQ targets reference declared PDs.
        for irq in &self.irq_delivery {
            if !pd_names.contains(irq.target_pd.as_str()) {
                return Err(SpecParseError::UnknownIrqTarget {
                    irq: irq.irq,
                    target_pd: irq.target_pd.clone(),
                });
            }
        }

        // Memory regions don't overlap.
        for i in 0..self.memory_regions.len() {
            for j in (i + 1)..self.memory_regions.len() {
                let a = &self.memory_regions[i];
                let b = &self.memory_regions[j];
                let a_end = a.phys_addr.saturating_add(a.size_bytes);
                let b_end = b.phys_addr.saturating_add(b.size_bytes);
                let overlap = a.phys_addr < b_end && b.phys_addr < a_end;
                if overlap {
                    return Err(SpecParseError::OverlappingMemoryRegions {
                        name_a: a.name.clone(),
                        name_b: b.name.clone(),
                    });
                }
            }
        }

        Ok(())
    }

    /// Render a Microkit 2.2.0 XML system description from this spec.
    /// The XML is passed to the `microkit` assembler tool as the
    /// system-description argument. PD `<program_image>` paths use
    /// `{pd_name}.elf` — the `build_exec` module writes ELFs by that name.
    pub fn to_microkit_xml(&self) -> String {
        let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<system>\n");

        for pd in &self.protection_domains {
            xml.push_str(&format!(
                "    <protection_domain name=\"{}\" priority=\"{}\" stack_size=\"0x{:x}\">\n",
                pd.name, pd.priority, pd.stack_bytes
            ));
            xml.push_str(&format!(
                "        <program_image path=\"{}.elf\" />\n",
                pd.name
            ));
            xml.push_str("    </protection_domain>\n");
        }

        // Assign per-PD channel IDs in declaration order.
        let mut pd_ch_id: std::collections::HashMap<&str, u8> = std::collections::HashMap::new();
        for ch in &self.channels {
            let id_a = {
                let c = pd_ch_id.entry(ch.end_a.as_str()).or_insert(0);
                let v = *c;
                *c += 1;
                v
            };
            let id_b = {
                let c = pd_ch_id.entry(ch.end_b.as_str()).or_insert(0);
                let v = *c;
                *c += 1;
                v
            };
            xml.push_str("    <channel>\n");
            xml.push_str(&format!(
                "        <end pd=\"{}\" id=\"{}\" />\n",
                ch.end_a, id_a
            ));
            xml.push_str(&format!(
                "        <end pd=\"{}\" id=\"{}\" />\n",
                ch.end_b, id_b
            ));
            xml.push_str("    </channel>\n");
        }

        xml.push_str("</system>\n");
        xml
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_hello_world_toml() -> &'static str {
        r#"
[[protection_domains]]
name = "hello"
binary = "hello.elf"
priority = 100
stack_bytes = 8192

[[memory_regions]]
name = "uart"
phys_addr = 0x09000000
size_bytes = 4096
caching = "device-memory"
permissions = ["read", "write"]
"#
    }

    #[test]
    fn minimal_hello_world_parses() {
        let spec = SystemSpec::from_toml_str(minimal_hello_world_toml())
            .expect("hello-world spec should parse");
        assert_eq!(spec.protection_domains.len(), 1);
        assert_eq!(spec.protection_domains[0].name, "hello");
        assert_eq!(spec.protection_domains[0].priority, 100);
        assert_eq!(spec.protection_domains[0].stack_bytes, 8192);
        assert_eq!(spec.memory_regions.len(), 1);
        assert_eq!(spec.memory_regions[0].caching, CachingPolicy::DeviceMemory);
    }

    #[test]
    fn empty_spec_validates() {
        let spec: SystemSpec = toml::from_str("").unwrap();
        assert!(spec.validate().is_ok());
    }

    #[test]
    fn round_trip_via_toml() {
        let spec = SystemSpec::from_toml_str(minimal_hello_world_toml()).unwrap();
        let serialised = toml::to_string(&spec).unwrap();
        let restored = SystemSpec::from_toml_str(&serialised).unwrap();
        assert_eq!(spec, restored);
    }

    #[test]
    fn rejects_too_many_pds() {
        let mut pds = String::new();
        for i in 0..64 {
            pds.push_str(&format!(
                "[[protection_domains]]\nname = \"pd{i}\"\nbinary = \"x.elf\"\n\n"
            ));
        }
        let r = SystemSpec::from_toml_str(&pds);
        assert!(matches!(
            r,
            Err(SpecParseError::TooManyProtectionDomains { actual: 64 })
        ));
    }

    #[test]
    fn rejects_duplicate_pd_names() {
        let toml = r#"
[[protection_domains]]
name = "dup"
binary = "a.elf"

[[protection_domains]]
name = "dup"
binary = "b.elf"
"#;
        let r = SystemSpec::from_toml_str(toml);
        assert!(matches!(r, Err(SpecParseError::DuplicatePdName(_))));
    }

    #[test]
    fn rejects_overlapping_memory_regions() {
        let toml = r#"
[[memory_regions]]
name = "a"
phys_addr = 0x1000
size_bytes = 0x2000

[[memory_regions]]
name = "b"
phys_addr = 0x2000
size_bytes = 0x1000
"#;
        let r = SystemSpec::from_toml_str(toml);
        assert!(matches!(
            r,
            Err(SpecParseError::OverlappingMemoryRegions { .. })
        ));
    }

    #[test]
    fn adjacent_memory_regions_dont_overlap() {
        let toml = r#"
[[memory_regions]]
name = "a"
phys_addr = 0x1000
size_bytes = 0x1000

[[memory_regions]]
name = "b"
phys_addr = 0x2000
size_bytes = 0x1000
"#;
        // a covers 0x1000..0x2000; b covers 0x2000..0x3000 — adjacent
        // but disjoint.
        let r = SystemSpec::from_toml_str(toml);
        assert!(r.is_ok(), "adjacent regions should validate; got {r:?}");
    }

    #[test]
    fn rejects_unknown_channel_endpoint() {
        let toml = r#"
[[protection_domains]]
name = "a"
binary = "a.elf"

[[channels]]
name = "ch"
end_a = "a"
end_b = "ghost"
kind = "ppc"
"#;
        let r = SystemSpec::from_toml_str(toml);
        assert!(matches!(
            r,
            Err(SpecParseError::UnknownChannelEndpoint { .. })
        ));
    }

    #[test]
    fn rejects_unknown_irq_target() {
        let toml = r#"
[[protection_domains]]
name = "a"
binary = "a.elf"

[[irq_delivery]]
irq = 33
target_pd = "ghost"
"#;
        let r = SystemSpec::from_toml_str(toml);
        assert!(matches!(r, Err(SpecParseError::UnknownIrqTarget { .. })));
    }

    #[test]
    fn rejects_too_many_channels_for_pd() {
        // 64 channels all sharing one endpoint → that PD has 64 channels.
        let mut toml = String::from(
            r#"
[[protection_domains]]
name = "hub"
binary = "hub.elf"

[[protection_domains]]
name = "leaf0"
binary = "x.elf"
"#,
        );
        for i in 0..64 {
            toml.push_str(&format!(
                "\n[[channels]]\nname = \"ch{i}\"\nend_a = \"hub\"\nend_b = \"leaf0\"\nkind = \"notification\"\n"
            ));
        }
        let r = SystemSpec::from_toml_str(&toml);
        assert!(
            matches!(r, Err(SpecParseError::TooManyChannelsForPd { .. })),
            "64 channels on one PD should be rejected; got {r:?}"
        );
    }

    #[test]
    fn rejects_malformed_toml() {
        let r = SystemSpec::from_toml_str("this is not [valid toml");
        assert!(matches!(r, Err(SpecParseError::TomlError(_))));
    }

    #[test]
    fn ppc_channel_round_trip() {
        let toml = r#"
[[protection_domains]]
name = "client"
binary = "c.elf"

[[protection_domains]]
name = "server"
binary = "s.elf"

[[channels]]
name = "rpc"
end_a = "client"
end_b = "server"
kind = "ppc"
"#;
        let spec = SystemSpec::from_toml_str(toml).unwrap();
        assert_eq!(spec.channels[0].kind, ChannelKind::Ppc);
    }

    #[test]
    fn to_microkit_xml_single_pd() {
        let spec = SystemSpec::from_toml_str(
            r#"
[build]
board = "x86_64_generic"
config = "debug"

[[protection_domains]]
name = "hello"
binary = "pd/hello.c"
priority = 100
stack_bytes = 4096
"#,
        )
        .unwrap();
        let xml = spec.to_microkit_xml();
        assert!(xml.contains("<system>"), "should have system element");
        assert!(
            xml.contains("<protection_domain name=\"hello\""),
            "should have hello PD"
        );
        assert!(
            xml.contains("<program_image path=\"hello.elf\""),
            "should reference hello.elf"
        );
        assert!(xml.contains("priority=\"100\""), "should include priority");
        assert!(
            xml.contains("stack_size=\"0x1000\""),
            "should include stack_size in hex"
        );
    }

    #[test]
    fn to_microkit_xml_channel_assigns_ids() {
        let spec = SystemSpec::from_toml_str(
            r#"
[[protection_domains]]
name = "client"
binary = "client.c"

[[protection_domains]]
name = "server"
binary = "server.c"

[[channels]]
name = "rpc"
end_a = "client"
end_b = "server"
kind = "ppc"
"#,
        )
        .unwrap();
        let xml = spec.to_microkit_xml();
        assert!(xml.contains("<channel>"), "should have channel element");
        assert!(
            xml.contains("pd=\"client\" id=\"0\""),
            "client should get id 0"
        );
        assert!(
            xml.contains("pd=\"server\" id=\"0\""),
            "server should get id 0"
        );
    }

    #[test]
    fn build_config_parses_from_toml() {
        let spec = SystemSpec::from_toml_str(
            r#"
[build]
board = "qemu_virt_aarch64"
config = "release"
sdk = "/opt/microkit-custom"
output_dir = "out"

[[protection_domains]]
name = "p"
binary = "p.c"
"#,
        )
        .unwrap();
        assert_eq!(spec.build.board, "qemu_virt_aarch64");
        assert_eq!(spec.build.config, "release");
        assert_eq!(spec.build.sdk, "/opt/microkit-custom");
        assert_eq!(spec.build.output_dir, "out");
    }

    #[test]
    fn build_config_defaults_when_absent() {
        let spec = SystemSpec::from_toml_str(
            r#"
[[protection_domains]]
name = "p"
binary = "p.c"
"#,
        )
        .unwrap();
        assert_eq!(spec.build.board, "");
        assert_eq!(spec.build.config, "debug");
        assert_eq!(spec.build.sdk, "/opt/microkit-sdk-2.2.0");
        assert_eq!(spec.build.output_dir, "build");
    }
}
