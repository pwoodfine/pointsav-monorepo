//! BuildPlan — deterministic content-addressed manifest derived
//! from a [`SystemSpec`].
//!
//! Per `~/Foundry/conventions/system-substrate-doctrine.md` §6.1
//! release-artefact format and convention §6 reproducible-build
//! property. The plan is the artefact a customer-apex cosignature
//! commits to: same SystemSpec → same `plan_hash` → reproducible
//! across machines, time, and operators.
//!
//! v0.1.x scope: design + types + derivation. Actual command
//! execution lands in cluster task #14 (FUTURE session, requires
//! cross-compile toolchain + seL4 source vendoring).

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::spec::SystemSpec;

/// 32-byte SHA-256 digest. Algorithm-agile per
/// `worm-ledger-design.md` §3 D3 (future MINOR may add BLAKE3 /
/// SHA-3 alongside).
pub type Hash256 = [u8; 32];

/// Deterministic build manifest for a SystemSpec.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildPlan {
    /// SHA-256 of the canonical TOML rendering of the SystemSpec
    /// (whitespace-invariant; same spec → same hash).
    pub spec_hash: Hash256,
    /// Ordered build steps. Execution order matches `Vec` order;
    /// each step's outputs are inputs to subsequent steps that
    /// reference them.
    pub steps: Vec<BuildStep>,
    /// SHA-256 of the canonical JSON of `(spec_hash, steps)`. The
    /// value a customer-apex cosignature commits to.
    pub plan_hash: Hash256,
}

/// One build step in the plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildStep {
    /// Stable identifier (e.g., `compile-pd-hello`).
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Files this step reads.
    pub input_paths: Vec<String>,
    /// Files this step produces.
    pub output_paths: Vec<String>,
    /// The semantic operation. Variant set is a closed enum;
    /// adding a variant is a doctrine MINOR.
    pub command: BuildCommand,
}

/// Semantic build operations. Closed variant set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BuildCommand {
    /// Cross-compile a C Protection Domain binary with aarch64-linux-gnu-gcc.
    CompilePd {
        pd_name: String,
        source_path: String,
        binary_target: String,
    },
    /// Build a Rust Protection Domain binary with `cargo build --target
    /// aarch64-unknown-none --release`. `crate_name` is the crate directory
    /// (e.g. `"moonshot-sel4-vmm"`); `bin_name` is the `--bin` target.
    CompileRustPd {
        pd_name: String,
        crate_name: String,
        bin_name: String,
        binary_target: String,
    },
    /// Assemble the final bootable image from PD binaries +
    /// system spec.
    AssembleImage {
        pd_binary_paths: Vec<String>,
        spec_hash: Hash256,
        output_image: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanGenerationError {
    /// SystemSpec produced no protection_domains; nothing to compile.
    EmptySpec,
    /// TOML serialisation of the canonical SystemSpec failed (should
    /// never happen for a SystemSpec that successfully parsed).
    CanonicalisationFailed(String),
}

impl BuildPlan {
    /// Generate a BuildPlan from a SystemSpec. Deterministic: same
    /// spec → same plan_hash.
    pub fn from_spec(spec: &SystemSpec) -> Result<Self, PlanGenerationError> {
        if spec.protection_domains.is_empty() {
            return Err(PlanGenerationError::EmptySpec);
        }

        // 1. Canonical SystemSpec hash.
        let canonical = toml::to_string(spec)
            .map_err(|e| PlanGenerationError::CanonicalisationFailed(e.to_string()))?;
        let spec_hash = sha256_bytes(canonical.as_bytes());

        // 2. Generate per-PD compile steps in declared order.
        let mut steps: Vec<BuildStep> = spec
            .protection_domains
            .iter()
            .map(|pd| {
                let binary_target = format!("build/{}.elf", pd.name);
                if let Some(bin_name) = &pd.rust_bin {
                    // Rust PD: cargo build --target aarch64-unknown-none
                    BuildStep {
                        name: format!("compile-pd-{}", pd.name),
                        description: format!(
                            "Build Rust PD `{}` (crate: {}, bin: {})",
                            pd.name, pd.binary, bin_name
                        ),
                        input_paths: vec![format!("{}/Cargo.toml", pd.binary)],
                        output_paths: vec![binary_target.clone()],
                        command: BuildCommand::CompileRustPd {
                            pd_name: pd.name.clone(),
                            crate_name: pd.binary.clone(),
                            bin_name: bin_name.clone(),
                            binary_target,
                        },
                    }
                } else {
                    // C PD: aarch64-linux-gnu-gcc
                    BuildStep {
                        name: format!("compile-pd-{}", pd.name),
                        description: format!("Cross-compile protection domain `{}`", pd.name),
                        input_paths: vec![pd.binary.clone()],
                        output_paths: vec![binary_target.clone()],
                        command: BuildCommand::CompilePd {
                            pd_name: pd.name.clone(),
                            source_path: pd.binary.clone(),
                            binary_target,
                        },
                    }
                }
            })
            .collect();

        // 3. Final assembly step.
        let pd_binaries: Vec<String> = spec
            .protection_domains
            .iter()
            .map(|pd| format!("build/{}.elf", pd.name))
            .collect();
        steps.push(BuildStep {
            name: "assemble-image".to_string(),
            description: "Assemble bootable seL4 image from PD binaries and system spec"
                .to_string(),
            input_paths: pd_binaries.clone(),
            output_paths: vec!["build/system-image.bin".to_string()],
            command: BuildCommand::AssembleImage {
                pd_binary_paths: pd_binaries,
                spec_hash,
                output_image: "build/system-image.bin".to_string(),
            },
        });

        // 4. plan_hash = SHA-256 of canonical JSON of (spec_hash, steps).
        let plan_hash = compute_plan_hash(&spec_hash, &steps)?;

        Ok(BuildPlan {
            spec_hash,
            steps,
            plan_hash,
        })
    }
}

fn sha256_bytes(bytes: &[u8]) -> Hash256 {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

fn compute_plan_hash(
    spec_hash: &Hash256,
    steps: &[BuildStep],
) -> Result<Hash256, PlanGenerationError> {
    // Canonical JSON serialisation; serde JSON preserves struct
    // field order, so the bytes are deterministic per the type
    // declaration.
    #[derive(Serialize)]
    struct Canonical<'a> {
        spec_hash: &'a Hash256,
        steps: &'a [BuildStep],
    }
    let canonical = Canonical { spec_hash, steps };
    let bytes = serde_json::to_vec(&canonical)
        .map_err(|e| PlanGenerationError::CanonicalisationFailed(e.to_string()))?;
    Ok(sha256_bytes(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::{ProtectionDomain, SystemSpec};

    fn fixture_spec_one_pd() -> SystemSpec {
        SystemSpec {
            protection_domains: vec![ProtectionDomain {
                name: "hello".to_string(),
                binary: "src/hello.rs".to_string(),
                rust_bin: None,
                priority: 100,
                stack_bytes: 4096,
            }],
            channels: vec![],
            memory_regions: vec![],
            irq_delivery: vec![],
            build: Default::default(),
        }
    }

    fn fixture_spec_two_pds() -> SystemSpec {
        SystemSpec {
            protection_domains: vec![
                ProtectionDomain {
                    name: "client".to_string(),
                    binary: "src/client.rs".to_string(),
                    rust_bin: None,
                    priority: 100,
                    stack_bytes: 4096,
                },
                ProtectionDomain {
                    name: "server".to_string(),
                    binary: "src/server.rs".to_string(),
                    rust_bin: None,
                    priority: 100,
                    stack_bytes: 4096,
                },
            ],
            channels: vec![],
            memory_regions: vec![],
            irq_delivery: vec![],
            build: Default::default(),
        }
    }

    fn fixture_spec_rust_pd() -> SystemSpec {
        SystemSpec {
            protection_domains: vec![ProtectionDomain {
                name: "os-console".to_string(),
                binary: "moonshot-sel4-vmm".to_string(),
                rust_bin: Some("console_main".to_string()),
                priority: 100,
                stack_bytes: 65536,
            }],
            channels: vec![],
            memory_regions: vec![],
            irq_delivery: vec![],
            build: Default::default(),
        }
    }

    #[test]
    fn empty_spec_errors() {
        let spec = SystemSpec {
            protection_domains: vec![],
            channels: vec![],
            memory_regions: vec![],
            irq_delivery: vec![],
            build: Default::default(),
        };
        let r = BuildPlan::from_spec(&spec);
        assert_eq!(r, Err(PlanGenerationError::EmptySpec));
    }

    #[test]
    fn single_pd_generates_two_steps() {
        let spec = fixture_spec_one_pd();
        let plan = BuildPlan::from_spec(&spec).unwrap();
        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.steps[0].name, "compile-pd-hello");
        assert_eq!(plan.steps[1].name, "assemble-image");
    }

    #[test]
    fn two_pds_generate_three_steps() {
        let spec = fixture_spec_two_pds();
        let plan = BuildPlan::from_spec(&spec).unwrap();
        assert_eq!(plan.steps.len(), 3);
        assert_eq!(plan.steps[0].name, "compile-pd-client");
        assert_eq!(plan.steps[1].name, "compile-pd-server");
        assert_eq!(plan.steps[2].name, "assemble-image");
    }

    #[test]
    fn plan_is_deterministic() {
        let spec = fixture_spec_one_pd();
        let plan1 = BuildPlan::from_spec(&spec).unwrap();
        let plan2 = BuildPlan::from_spec(&spec).unwrap();
        assert_eq!(plan1.plan_hash, plan2.plan_hash);
        assert_eq!(plan1.spec_hash, plan2.spec_hash);
        assert_eq!(plan1, plan2);
    }

    #[test]
    fn different_specs_produce_different_plans() {
        let spec_a = fixture_spec_one_pd();
        let spec_b = fixture_spec_two_pds();
        let plan_a = BuildPlan::from_spec(&spec_a).unwrap();
        let plan_b = BuildPlan::from_spec(&spec_b).unwrap();
        assert_ne!(plan_a.plan_hash, plan_b.plan_hash);
        assert_ne!(plan_a.spec_hash, plan_b.spec_hash);
    }

    #[test]
    fn pd_renaming_changes_plan_hash() {
        let mut spec = fixture_spec_one_pd();
        let plan_before = BuildPlan::from_spec(&spec).unwrap();
        spec.protection_domains[0].name = "renamed".to_string();
        let plan_after = BuildPlan::from_spec(&spec).unwrap();
        assert_ne!(plan_before.plan_hash, plan_after.plan_hash);
        // Also verify the step name reflects the rename.
        assert_eq!(plan_after.steps[0].name, "compile-pd-renamed");
    }

    #[test]
    fn plan_step_outputs_match_pd_names() {
        let spec = fixture_spec_two_pds();
        let plan = BuildPlan::from_spec(&spec).unwrap();
        assert_eq!(plan.steps[0].output_paths, vec!["build/client.elf"]);
        assert_eq!(plan.steps[1].output_paths, vec!["build/server.elf"]);
        assert_eq!(
            plan.steps[2].input_paths,
            vec!["build/client.elf", "build/server.elf"]
        );
        assert_eq!(plan.steps[2].output_paths, vec!["build/system-image.bin"]);
    }

    #[test]
    fn plan_serialises_round_trip() {
        let plan = BuildPlan::from_spec(&fixture_spec_one_pd()).unwrap();
        let json = serde_json::to_string(&plan).unwrap();
        let restored: BuildPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(plan, restored);
    }

    #[test]
    fn assemble_step_carries_spec_hash() {
        let plan = BuildPlan::from_spec(&fixture_spec_one_pd()).unwrap();
        match &plan.steps[1].command {
            BuildCommand::AssembleImage { spec_hash, .. } => {
                assert_eq!(*spec_hash, plan.spec_hash);
            }
            other => panic!("expected AssembleImage; got {other:?}"),
        }
    }

    #[test]
    fn priority_change_changes_plan_hash() {
        let mut spec = fixture_spec_one_pd();
        let plan_before = BuildPlan::from_spec(&spec).unwrap();
        spec.protection_domains[0].priority = 50;
        let plan_after = BuildPlan::from_spec(&spec).unwrap();
        assert_ne!(plan_before.plan_hash, plan_after.plan_hash);
    }

    #[test]
    fn rust_pd_generates_compile_rust_pd_step() {
        let spec = fixture_spec_rust_pd();
        let plan = BuildPlan::from_spec(&spec).unwrap();
        assert_eq!(plan.steps.len(), 2);
        match &plan.steps[0].command {
            BuildCommand::CompileRustPd {
                pd_name,
                crate_name,
                bin_name,
                binary_target,
            } => {
                assert_eq!(pd_name, "os-console");
                assert_eq!(crate_name, "moonshot-sel4-vmm");
                assert_eq!(bin_name, "console_main");
                assert_eq!(binary_target, "build/os-console.elf");
            }
            other => panic!("expected CompileRustPd; got {other:?}"),
        }
    }

    #[test]
    fn rust_pd_plan_differs_from_c_pd_plan() {
        let c_spec = fixture_spec_one_pd();
        let rust_spec = fixture_spec_rust_pd();
        let c_plan = BuildPlan::from_spec(&c_spec).unwrap();
        let rust_plan = BuildPlan::from_spec(&rust_spec).unwrap();
        assert_ne!(c_plan.plan_hash, rust_plan.plan_hash);
    }
}
