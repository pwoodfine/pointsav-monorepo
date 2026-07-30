//! moonshot-toolkit CLI — Rust-only build orchestrator for
//! Foundry's seL4 unikernel images.
//!
//! Per MEMO §7 and convention `system-substrate-doctrine.md` §6.
//! Three subcommands:
//!
//! - `validate <spec.toml>` — parse + invariant-check; exit 0 on
//!   valid, non-zero on parse/validation failure
//! - `plan <spec.toml>` — parse + generate + print BuildPlan
//! - `build <spec.toml>` — cross-compile each PD to AArch64 ELF
//!   (aarch64-linux-gnu-gcc), then assemble a bootable elfloader.elf

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use moonshot_toolkit::plan::{BuildCommand, BuildPlan, BuildStep};
use moonshot_toolkit::spec::SystemSpec;

#[derive(Parser, Debug)]
#[command(
    name = "moonshot-toolkit",
    version,
    about = "Rust-only build orchestrator for Foundry seL4 unikernel images",
    long_about = "Per MEMO §7 (Microkit Python/CMake → moonshot-toolkit \
                  Rust-Only Toolchain) and convention \
                  system-substrate-doctrine.md §6 \
                  (Reproducible-Verification-On-Customer-Metal)."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Parse and validate a system-spec.toml without building.
    Validate {
        /// Path to system-spec.toml.
        spec_path: PathBuf,
    },
    /// Generate and print a BuildPlan from a system-spec.toml.
    Plan {
        /// Path to system-spec.toml.
        spec_path: PathBuf,
        /// Output format.
        #[arg(long, value_enum, default_value_t = PlanFormat::Json)]
        format: PlanFormat,
    },
    /// Cross-compile each protection domain to an AArch64 ELF using
    /// aarch64-linux-gnu-gcc, then assemble a bootable seL4 elfloader image.
    /// Requires vendor-sel4-kernel built with KernelVerificationBuild=OFF
    /// and vendor-sel4-tools/elfloader-tool present.
    Build {
        /// Path to system-spec.toml.
        spec_path: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
enum PlanFormat {
    Json,
    PrettyJson,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match dispatch(cli.command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn dispatch(command: Command) -> Result<(), String> {
    match command {
        Command::Validate { spec_path } => cmd_validate(&spec_path),
        Command::Plan { spec_path, format } => cmd_plan(&spec_path, format),
        Command::Build { spec_path } => cmd_build(&spec_path),
    }
}

fn read_spec(spec_path: &std::path::Path) -> Result<SystemSpec, String> {
    let text = std::fs::read_to_string(spec_path)
        .map_err(|e| format!("read {}: {e}", spec_path.display()))?;
    SystemSpec::from_toml_str(&text).map_err(|e| format!("parse {}: {e:?}", spec_path.display()))
}

fn cmd_validate(spec_path: &std::path::Path) -> Result<(), String> {
    let spec = read_spec(spec_path)?;
    println!(
        "✓ {} — {} protection_domain(s), {} channel(s), {} memory_region(s), {} irq_delivery",
        spec_path.display(),
        spec.protection_domains.len(),
        spec.channels.len(),
        spec.memory_regions.len(),
        spec.irq_delivery.len(),
    );
    Ok(())
}

fn cmd_plan(spec_path: &std::path::Path, format: PlanFormat) -> Result<(), String> {
    let spec = read_spec(spec_path)?;
    let plan = BuildPlan::from_spec(&spec).map_err(|e| format!("plan: {e:?}"))?;
    let rendered = match format {
        PlanFormat::Json => {
            serde_json::to_string(&plan).map_err(|e| format!("render plan: {e}"))?
        }
        PlanFormat::PrettyJson => {
            serde_json::to_string_pretty(&plan).map_err(|e| format!("render plan: {e}"))?
        }
    };
    println!("{rendered}");
    Ok(())
}

fn cmd_build(spec_path: &std::path::Path) -> Result<(), String> {
    let spec = read_spec(spec_path)?;
    let plan = BuildPlan::from_spec(&spec).map_err(|e| format!("plan: {e:?}"))?;
    println!("Building plan (plan_hash = {})", hex_short(&plan.plan_hash));
    std::fs::create_dir_all("build").map_err(|e| format!("create build/: {e}"))?;
    let n = plan.steps.len();
    for (i, step) in plan.steps.iter().enumerate() {
        println!("[{}/{}] {}", i + 1, n, step.name);
        execute_step(step)?;
    }
    println!("✓ build complete");
    Ok(())
}

fn execute_step(step: &BuildStep) -> Result<(), String> {
    match &step.command {
        BuildCommand::CompilePd {
            pd_name,
            source_path,
            binary_target,
        } => {
            // Cross-compile to bare-metal AArch64 ELF.
            // -nostdlib -nostartfiles: no libc or crt0 — PD provides _start
            // -ffreestanding: no hosted-environment assumptions
            // -static -no-pie: seL4 Microkit PDs are loaded at a fixed
            //   virtual address; dynamic linking is not available in-kernel
            // -mgeneral-regs-only: exclude FPU/SIMD (seL4 kernel doesn't save
            //   FPU state by default; PDs that need FPU must opt in explicitly)
            let output = std::process::Command::new("aarch64-linux-gnu-gcc")
                .args([
                    "-nostdlib",
                    "-nostartfiles",
                    "-ffreestanding",
                    "-static",
                    "-no-pie",
                    "-march=armv8-a",
                    "-mgeneral-regs-only",
                    // -O2: seL4 rootserver starts with SP uninitialised; without
                    // optimisation the compiler emits a stack-frame prologue at
                    // _start that immediately faults. -O2 inlines all static
                    // helpers and elides the frame. Required for all bare-metal PDs.
                    "-O2",
                    source_path,
                    "-o",
                    binary_target,
                ])
                .output()
                .map_err(|e| format!("compile-pd-{pd_name}: exec aarch64-linux-gnu-gcc: {e}"))?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(format!("compile-pd-{pd_name}:\n{stderr}"));
            }
            println!("  ✓ {binary_target}");
            Ok(())
        }
        BuildCommand::CompileRustPd {
            pd_name,
            crate_name,
            bin_name,
            binary_target,
        } => compile_rust_pd(pd_name, crate_name, bin_name, binary_target),
        BuildCommand::AssembleImage {
            pd_binary_paths,
            output_image,
            ..
        } => assemble_image(pd_binary_paths, output_image),
    }
}

fn compile_rust_pd(
    pd_name: &str,
    crate_name: &str,
    bin_name: &str,
    binary_target: &str,
) -> Result<(), String> {
    // Inherit CARGO_TARGET_DIR if set (avoids lock contention with the
    // outer `cargo run` that drives moonshot-toolkit itself).
    // Fall back to the crate's own target/ directory.
    let target_dir = std::env::var("CARGO_TARGET_DIR")
        .unwrap_or_else(|_| format!("{crate_name}/target"));

    // Stream cargo output to stdout/stderr directly (status(), not output())
    // so the user sees progress on long Rust builds.
    let status = std::process::Command::new("cargo")
        .args([
            "build",
            "--manifest-path",
            &format!("{crate_name}/Cargo.toml"),
            "--target",
            "aarch64-unknown-none",
            "--release",
            "--bin",
            bin_name,
        ])
        .env("CARGO_TARGET_DIR", &target_dir)
        .status()
        .map_err(|e| format!("compile-rust-pd-{pd_name}: exec cargo: {e}"))?;

    if !status.success() {
        return Err(format!(
            "compile-rust-pd-{pd_name}: `cargo build --bin {bin_name}` failed"
        ));
    }

    let elf_src = format!("{target_dir}/aarch64-unknown-none/release/{bin_name}");
    std::fs::copy(&elf_src, binary_target).map_err(|e| {
        format!("compile-rust-pd-{pd_name}: copy {elf_src} → {binary_target}: {e}")
    })?;
    println!("  ✓ {binary_target}");
    Ok(())
}

fn run_gcc(args: &[String], context: &str) -> Result<(), String> {
    let out = std::process::Command::new("aarch64-linux-gnu-gcc")
        .args(args)
        .output()
        .map_err(|e| format!("{context}: exec aarch64-linux-gnu-gcc: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "{context}:\n{}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(())
}

fn assemble_image(pd_binary_paths: &[String], output_image: &str) -> Result<(), String> {
    const ELFLOADER: &str = "vendor-sel4-tools/elfloader-tool";
    const KERNEL_BUILD: &str = "vendor-sel4-kernel/build/aarch64-qemu";
    const BUILD_SUPPORT: &str = "vendor-sel4-project/build-support/qemu-arm-virt";

    // Well-known prerequisites — all relative to CWD (project root)
    let required = [
        ELFLOADER,
        &format!("{KERNEL_BUILD}/kernel.elf") as &str,
        &format!("{KERNEL_BUILD}/kernel.dtb"),
        &format!("{KERNEL_BUILD}/autoconf"),
        &format!("{KERNEL_BUILD}/gen_config"),
        &format!("{BUILD_SUPPORT}/gen_config"),
        &format!("{BUILD_SUPPORT}/gen_headers"),
        &format!("{BUILD_SUPPORT}/libcpio/cpio.c"),
    ];
    for path in &required {
        if !std::path::Path::new(path).exists() {
            return Err(format!(
                "assemble-image: prerequisite not found: {path}\n\
                 Build vendor-sel4-kernel with: KernelVerificationBuild=OFF \
                 KernelDebugBuild=ON KernelPrinting=ON KernelPlatform=qemu-arm-virt \
                 KernelSel4Arch=aarch64"
            ));
        }
    }
    let rootserver = pd_binary_paths.first().ok_or(
        "assemble-image: pd_binary_paths is empty; expected rootserver ELF as first entry",
    )?;

    std::fs::create_dir_all("build/elfloader-obj")
        .map_err(|e| format!("create build/elfloader-obj: {e}"))?;
    std::fs::create_dir_all("build/libcpio").map_err(|e| format!("create build/libcpio: {e}"))?;

    // CPIO archive: kernel.elf + kernel.dtb + rootserver
    let kernel_elf = std::fs::read(format!("{KERNEL_BUILD}/kernel.elf"))
        .map_err(|e| format!("read kernel.elf: {e}"))?;
    let kernel_dtb = std::fs::read(format!("{KERNEL_BUILD}/kernel.dtb"))
        .map_err(|e| format!("read kernel.dtb: {e}"))?;
    let rootserver_bytes =
        std::fs::read(rootserver).map_err(|e| format!("read {rootserver}: {e}"))?;
    let archive = moonshot_toolkit::cpio::write_archive(&[
        ("kernel.elf", &kernel_elf),
        ("kernel.dtb", &kernel_dtb),
        ("rootserver", &rootserver_bytes),
    ]);
    std::fs::write("build/archive.cpio", &archive)
        .map_err(|e| format!("write build/archive.cpio: {e}"))?;

    // archive.S: .incbin needs an absolute path
    let cwd = std::env::current_dir().map_err(|e| format!("current_dir: {e}"))?;
    let archive_abs = cwd.join("build/archive.cpio");
    let archive_s = format!(
        ".section ._archive_cpio,\"a\"\n\
         .globl _archive_start_end\n\
         _archive_start_end_minus_archive:\n\
         .incbin \"{}\"\n\
         _archive_start_end:\n",
        archive_abs.display()
    );
    std::fs::write("build/archive.S", archive_s.as_bytes())
        .map_err(|e| format!("write build/archive.S: {e}"))?;

    // Copy libcpio to build/libcpio/:
    //   libcpio/cpio.c         → build/libcpio/cpio.c
    //   libcpio/cpio/cpio.h    → build/libcpio/cpio/cpio.h  (included as <cpio/cpio.h>)
    std::fs::copy(
        format!("{BUILD_SUPPORT}/libcpio/cpio.c"),
        "build/libcpio/cpio.c",
    )
    .map_err(|e| format!("copy libcpio/cpio.c: {e}"))?;
    std::fs::create_dir_all("build/libcpio/cpio")
        .map_err(|e| format!("create build/libcpio/cpio: {e}"))?;
    std::fs::copy(
        format!("{BUILD_SUPPORT}/libcpio/cpio/cpio.h"),
        "build/libcpio/cpio/cpio.h",
    )
    .map_err(|e| format!("copy libcpio/cpio/cpio.h: {e}"))?;

    let include_dirs: Vec<String> = vec![
        format!("{ELFLOADER}/include"),
        format!("{ELFLOADER}/include/arch-arm"),
        format!("{ELFLOADER}/include/arch-arm/64"),
        format!("{ELFLOADER}/include/arch-arm/armv/armv8-a"),
        format!("{ELFLOADER}/include/arch-arm/armv/armv8-a/64"),
        format!("{BUILD_SUPPORT}/gen_config"),
        format!("{BUILD_SUPPORT}/gen_headers"),
        "build/libcpio".to_string(),
        format!("{KERNEL_BUILD}/autoconf"),
        format!("{KERNEL_BUILD}/gen_config"),
    ];

    const CFLAGS: &[&str] = &[
        "-march=armv8-a",
        "-D__KERNEL_64__",
        "-ffreestanding",
        "-fno-common",
        "-fno-pic",
        "-fno-pie",
        "-mgeneral-regs-only",
        "-mstrict-align",
        "-D_XOPEN_SOURCE=700",
    ];

    const ELFLOADER_SRCS: &[&str] = &[
        "src/common.c",
        "src/defaults.c",
        "src/printf.c",
        "src/string.c",
        "src/fdt.c",
        "src/drivers/driver.c",
        "src/drivers/smp/common.c",
        "src/drivers/uart/8250-uart.c",
        "src/drivers/uart/bcm-uart.c",
        "src/drivers/uart/common.c",
        "src/drivers/uart/exynos-uart.c",
        "src/drivers/uart/imx-lpuart.c",
        "src/drivers/uart/imx-uart.c",
        "src/drivers/uart/meson-uart.c",
        "src/drivers/uart/msm-uart.c",
        "src/drivers/uart/pl011-uart.c",
        "src/drivers/uart/stm32mp2-uart.c",
        "src/drivers/uart/xilinx-uart.c",
        "src/drivers/timer/arm_generic_timer.c",
        "src/utils/crypt_md5.c",
        "src/utils/crypt_sha256.c",
        "src/utils/hash.c",
        "src/arch-arm/cpuid.c",
        "src/arch-arm/psci.c",
        "src/arch-arm/scu.c",
        "src/arch-arm/smp_boot.c",
        "src/arch-arm/sys_boot.c",
        "src/arch-arm/drivers/smp-imx6.c",
        "src/arch-arm/drivers/smp-psci.c",
        "src/arch-arm/drivers/smp-spin-table.c",
        "src/arch-arm/drivers/smp-zynq7000.c",
        "src/binaries/elf/elf.c",
        "src/binaries/elf/elf32.c",
        "src/binaries/elf/elf64.c",
        "src/arch-arm/64/cpuid.c",
        "src/arch-arm/64/debug.c",
        "src/arch-arm/64/mmu.c",
        "src/arch-arm/64/structures.c",
        "src/arch-arm/armv/armv8-a/64/smp.c",
        "src/arch-arm/64/crt0.S",
        "src/arch-arm/64/traps.S",
        "src/arch-arm/armv/armv8-a/64/mmu-hyp.S",
        "src/arch-arm/armv/armv8-a/64/mmu.S",
        "src/arch-arm/armv/armv8-a/64/psci_asm.S",
    ];

    // Compile archive.S first (no CFLAGS, just -march=armv8-a)
    let archive_obj = "build/elfloader-obj/000_archive.o".to_string();
    run_gcc(
        &[
            "-march=armv8-a".to_string(),
            "-c".to_string(),
            "-o".to_string(),
            archive_obj.clone(),
            "build/archive.S".to_string(),
        ],
        "compile archive.S",
    )?;

    // Compile elfloader sources
    let mut obj_paths = vec![archive_obj];
    for (i, src_rel) in ELFLOADER_SRCS.iter().enumerate() {
        let src = format!("{ELFLOADER}/{src_rel}");
        let obj = format!("build/elfloader-obj/{:03}.o", i + 1);
        let mut args: Vec<String> = CFLAGS.iter().map(|&s| s.to_string()).collect();
        args.extend(include_dirs.iter().map(|d| format!("-I{d}")));
        args.extend(["-c".to_string(), "-o".to_string(), obj.clone(), src.clone()]);
        run_gcc(&args, &format!("compile {src_rel}"))?;
        obj_paths.push(obj);
    }

    // Compile libcpio.c
    let libcpio_obj = format!("build/elfloader-obj/{:03}.o", ELFLOADER_SRCS.len() + 1);
    {
        let mut args: Vec<String> = CFLAGS.iter().map(|&s| s.to_string()).collect();
        args.extend(include_dirs.iter().map(|d| format!("-I{d}")));
        args.extend([
            "-c".to_string(),
            "-o".to_string(),
            libcpio_obj.clone(),
            "build/libcpio/cpio.c".to_string(),
        ]);
        run_gcc(&args, "compile libcpio/cpio.c")?;
    }
    obj_paths.push(libcpio_obj);

    // Preprocess linker script
    {
        let linker_lds = format!("{ELFLOADER}/src/linker.lds");
        let mut args = vec!["-march=armv8-a".to_string()];
        args.extend(include_dirs.iter().map(|d| format!("-I{d}")));
        args.extend([
            "-P".to_string(),
            "-E".to_string(),
            "-x".to_string(),
            "c".to_string(),
            linker_lds,
            "-o".to_string(),
            "build/linker.lds_pp".to_string(),
        ]);
        run_gcc(&args, "preprocess linker.lds")?;
    }

    // Link
    {
        let mut args = vec![
            "-nostdlib".to_string(),
            "-static".to_string(),
            "-Wl,--build-id=none".to_string(),
            "-Wl,-T,build/linker.lds_pp".to_string(),
            "-march=armv8-a".to_string(),
        ];
        args.extend_from_slice(&obj_paths);
        args.extend([
            "-lgcc".to_string(),
            "-o".to_string(),
            "build/elfloader.elf".to_string(),
        ]);
        run_gcc(&args, "link elfloader.elf")?;
    }

    std::fs::copy("build/elfloader.elf", output_image)
        .map_err(|e| format!("copy elfloader.elf → {output_image}: {e}"))?;
    println!("  ✓ {output_image}");
    Ok(())
}

fn hex_short(hash: &[u8; 32]) -> String {
    let mut s = String::with_capacity(16);
    for b in &hash[..8] {
        s.push_str(&format!("{b:02x}"));
    }
    s.push('…');
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_spec(text: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(text.as_bytes()).unwrap();
        f.flush().unwrap();
        f
    }

    fn minimal_spec() -> &'static str {
        r#"
[[protection_domains]]
name = "hello"
binary = "src/hello.rs"
priority = 100
stack_bytes = 4096
"#
    }

    #[test]
    fn validate_command_accepts_minimal_spec() {
        let f = write_spec(minimal_spec());
        let r = cmd_validate(f.path());
        assert!(r.is_ok(), "validate should accept; got {r:?}");
    }

    #[test]
    fn validate_command_rejects_invalid_spec() {
        let f = write_spec("this is not [valid toml");
        let r = cmd_validate(f.path());
        assert!(r.is_err());
    }

    #[test]
    fn validate_command_rejects_missing_file() {
        let r = cmd_validate(std::path::Path::new("/tmp/does-not-exist-9f8a3c.toml"));
        assert!(r.is_err());
    }

    #[test]
    fn plan_command_emits_json() {
        let f = write_spec(minimal_spec());
        let r = cmd_plan(f.path(), PlanFormat::Json);
        assert!(r.is_ok(), "plan should succeed; got {r:?}");
    }

    #[test]
    fn plan_command_emits_pretty_json() {
        let f = write_spec(minimal_spec());
        let r = cmd_plan(f.path(), PlanFormat::PrettyJson);
        assert!(r.is_ok());
    }

    #[test]
    fn build_command_errors_without_source_file() {
        // minimal_spec references "src/hello.rs" which does not exist at
        // test time. cmd_build invokes aarch64-linux-gnu-gcc; the missing
        // source file causes a compile error (or exec error if the
        // toolchain is absent).
        let f = write_spec(minimal_spec());
        let r = cmd_build(f.path());
        assert!(
            r.is_err(),
            "build should fail when source file is absent; got {r:?}"
        );
    }

    #[test]
    fn empty_spec_build_errors_at_plan_step() {
        // No protection_domains → plan generation refuses before any
        // external command is invoked.
        let f = write_spec("");
        let r = cmd_build(f.path());
        assert!(r.is_err(), "empty spec should fail plan; got {r:?}");
    }

    #[test]
    fn hex_short_renders_first_eight_bytes() {
        let h = [0xAB; 32];
        assert!(hex_short(&h).starts_with("abababab"));
    }

    #[test]
    fn assemble_image_errors_when_elfloader_missing() {
        // From the test CWD (moonshot-toolkit/), vendor-sel4-tools/ is absent.
        // assemble_image should return a descriptive error naming the missing path.
        let r = assemble_image(&[], "build/system-image.bin");
        assert!(r.is_err());
        let msg = r.unwrap_err();
        assert!(
            msg.contains("vendor-sel4-tools"),
            "error should name the missing elfloader path: {msg}"
        );
    }
}
