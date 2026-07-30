// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

//! `xtask deposit` — writes a built release binary and its manifests into the
//! `RELEASES_DIR` tree that `app-privategit-source-2` serves from, and updates
//! the matching `products.yaml` entry.
//!
//! Root-cause fix for the 2026-07-04 live-site audit finding
//! (`BRIEF-software-ng-rewrite.md`): every one of the 8 catalog products'
//! download manifests 404 live because no automated deposit tool has ever
//! existed in this repo — every past release was hand-deposited, which is
//! exactly why editions drifted out of sync with what's actually on disk
//! (`os-network-admin` alone ended up on a different version/path scheme than
//! the other 7 products).
//!
//! Scope: this tool only writes to whatever `RELEASES_DIR` resolves to on the
//! machine it runs on, and to this monorepo's `products.yaml`. It does not
//! rsync/ssh/deploy anything to foundry-prod and does not invoke
//! `push-to-prod.sh` — that sync step stays Command Session's, out of scope
//! here.
//!
//! `products.yaml` is edited by hand-rolled line scan/replace, the same
//! approach `fsl_clock.rs` already established for this file (see that
//! module's doc comment): `xtask` has no `serde_yaml` dependency today, and a
//! full parse+reserialize round-trip would reformat the entire hand-maintained
//! file for a one- or two-line change.

use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, PartialEq, Debug)]
enum PathScheme {
    /// `path: <product>/<version>` — the default; matches what
    /// `order_download`/`product_detail.rs` assume and what makes
    /// `v1_products`'s `manifest_url` formula correct.
    Fixed,
    /// `path: <product>/latest/<platform>` — only for products that
    /// deliberately depend on `app-privategit-source-2`'s dynamic
    /// `latest_redirect()` alias (confirmed real for `os-network-admin`'s own
    /// install script). Never inferred; must be requested explicitly.
    LatestAlias,
}

#[derive(Debug)]
struct NewEntry {
    name: String,
    description: String,
    license_tier: String,
    price_usdc: i64,
    platform_label: String,
    size_mb: i64,
}

#[derive(Debug)]
struct DepositArgs {
    product: String,
    version: String,
    binary: PathBuf,
    platform: String,
    releases_dir: PathBuf,
    catalog: PathBuf,
    sig: Option<PathBuf>,
    source_commit: Option<String>,
    requires_license: bool,
    path_scheme: PathScheme,
    create_entry: Option<NewEntry>,
    force: bool,
    dry_run: bool,
}

#[derive(Debug)]
pub struct DepositReport {
    pub binary_path: PathBuf,
    pub sha256: String,
    pub size_bytes: u64,
    pub version_manifest_path: PathBuf,
    pub product_manifest_path: PathBuf,
    pub product_manifest_created: bool,
    pub catalog_changed: bool,
    pub catalog_diff: Option<String>,
    pub skipped_no_op: bool,
}

const USAGE: &str =
    "usage: xtask deposit --product <id> --version <v> --binary <path> --platform <slug>\n  \
    [--releases-dir <dir>] [--catalog <path/to/products.yaml>] [--sig <path>]\n  \
    [--source-commit <sha>] [--requires-license true|false] [--path-scheme fixed|latest-alias]\n  \
    [--create-entry --name <s> --description <s> --license-tier commercial|fsl \\\n  \
                    --price-usdc <int> --platform-label <s> --size-mb <int>]\n  \
    [--force] [--dry-run]";

pub fn run(args: &[String]) -> Result<(), String> {
    let parsed = parse_args(args)?;
    let dry_run = parsed.dry_run;
    let report = do_deposit(&parsed)?;
    if dry_run {
        println!("[dry-run] deposit: would write the following, nothing was touched:");
    }
    print_report(&report);
    Ok(())
}

fn print_report(r: &DepositReport) {
    if r.skipped_no_op {
        println!(
            "[=] deposit: no-op — {} already deposited with matching sha256 ({}); \
             products.yaml already correct.",
            r.binary_path.display(),
            r.sha256
        );
        return;
    }
    println!("[+] deposit: {}", r.binary_path.display());
    println!("    sha256:           {}", r.sha256);
    println!("    size:             {} bytes", r.size_bytes);
    println!(
        "    version manifest: {}",
        r.version_manifest_path.display()
    );
    println!(
        "    product manifest: {} ({})",
        r.product_manifest_path.display(),
        if r.product_manifest_created {
            "created"
        } else {
            "already existed, untouched"
        }
    );
    match &r.catalog_diff {
        Some(diff) => println!("    products.yaml:    updated — {diff}"),
        None if r.catalog_changed => println!("    products.yaml:    updated"),
        None => println!("    products.yaml:    already up to date, untouched"),
    }
}

fn parse_args(args: &[String]) -> Result<DepositArgs, String> {
    let mut product = None;
    let mut version = None;
    let mut binary = None;
    let mut platform = None;
    let mut releases_dir = None;
    let mut catalog = None;
    let mut sig = None;
    let mut source_commit = None;
    let mut requires_license = false;
    let mut path_scheme = PathScheme::Fixed;
    let mut force = false;
    let mut dry_run = false;

    let mut create_entry_requested = false;
    let mut name = None;
    let mut description = None;
    let mut license_tier = None;
    let mut price_usdc = None;
    let mut platform_label = None;
    let mut size_mb = None;

    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--product" => product = Some(next_val(&mut it, "--product")?),
            "--version" => version = Some(next_val(&mut it, "--version")?),
            "--binary" => binary = Some(PathBuf::from(next_val(&mut it, "--binary")?)),
            "--platform" => platform = Some(next_val(&mut it, "--platform")?),
            "--releases-dir" => {
                releases_dir = Some(PathBuf::from(next_val(&mut it, "--releases-dir")?))
            }
            "--catalog" => catalog = Some(PathBuf::from(next_val(&mut it, "--catalog")?)),
            "--sig" => sig = Some(PathBuf::from(next_val(&mut it, "--sig")?)),
            "--source-commit" => source_commit = Some(next_val(&mut it, "--source-commit")?),
            "--requires-license" => {
                let v = next_val(&mut it, "--requires-license")?;
                requires_license = parse_bool(&v, "--requires-license")?;
            }
            "--path-scheme" => {
                let v = next_val(&mut it, "--path-scheme")?;
                path_scheme = match v.as_str() {
                    "fixed" => PathScheme::Fixed,
                    "latest-alias" => PathScheme::LatestAlias,
                    other => {
                        return Err(format!(
                            "deposit: unknown --path-scheme '{other}' (expected fixed|latest-alias)"
                        ))
                    }
                };
            }
            "--create-entry" => create_entry_requested = true,
            "--name" => name = Some(next_val(&mut it, "--name")?),
            "--description" => description = Some(next_val(&mut it, "--description")?),
            "--license-tier" => license_tier = Some(next_val(&mut it, "--license-tier")?),
            "--price-usdc" => {
                let v = next_val(&mut it, "--price-usdc")?;
                price_usdc =
                    Some(v.parse::<i64>().map_err(|_| {
                        format!("deposit: --price-usdc must be an integer, got '{v}'")
                    })?);
            }
            "--platform-label" => platform_label = Some(next_val(&mut it, "--platform-label")?),
            "--size-mb" => {
                let v = next_val(&mut it, "--size-mb")?;
                size_mb =
                    Some(v.parse::<i64>().map_err(|_| {
                        format!("deposit: --size-mb must be an integer, got '{v}'")
                    })?);
            }
            "--force" => force = true,
            "--dry-run" => dry_run = true,
            other => return Err(format!("deposit: unknown argument '{other}'\n{USAGE}")),
        }
    }

    let product = product.ok_or(format!("deposit: --product <id> is required\n{USAGE}"))?;
    let version = version.ok_or(format!("deposit: --version <v> is required\n{USAGE}"))?;
    let binary = binary.ok_or(format!("deposit: --binary <path> is required\n{USAGE}"))?;
    let platform = platform.ok_or(format!(
        "deposit: --platform <slug> is required, no default — the 8 catalog products \
         disagree on platform-slug convention today, so guessing one would silently paper \
         over a real inconsistency\n{USAGE}"
    ))?;

    let releases_dir = releases_dir
        .or_else(|| std::env::var("RELEASES_DIR").ok().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("/var/lib/local-software/releases"));
    let catalog = catalog
        .unwrap_or_else(|| PathBuf::from("app-privategit-marketplace/catalog/products.yaml"));

    let create_entry = if create_entry_requested {
        Some(NewEntry {
            name: name.ok_or(format!("deposit: --create-entry requires --name\n{USAGE}"))?,
            description: description.ok_or(format!(
                "deposit: --create-entry requires --description\n{USAGE}"
            ))?,
            license_tier: {
                let t = license_tier.ok_or(format!(
                    "deposit: --create-entry requires --license-tier\n{USAGE}"
                ))?;
                if t != "commercial" && t != "fsl" {
                    return Err(format!(
                        "deposit: --license-tier must be 'commercial' or 'fsl', got '{t}'"
                    ));
                }
                t
            },
            price_usdc: price_usdc.ok_or(format!(
                "deposit: --create-entry requires --price-usdc\n{USAGE}"
            ))?,
            platform_label: platform_label.ok_or(format!(
                "deposit: --create-entry requires --platform-label\n{USAGE}"
            ))?,
            size_mb: size_mb.ok_or(format!(
                "deposit: --create-entry requires --size-mb\n{USAGE}"
            ))?,
        })
    } else {
        None
    };

    Ok(DepositArgs {
        product,
        version,
        binary,
        platform,
        releases_dir,
        catalog,
        sig,
        source_commit,
        requires_license,
        path_scheme,
        create_entry,
        force,
        dry_run,
    })
}

fn next_val(it: &mut std::slice::Iter<String>, flag: &str) -> Result<String, String> {
    it.next()
        .cloned()
        .ok_or_else(|| format!("deposit: {flag} requires a value"))
}

fn parse_bool(v: &str, flag: &str) -> Result<bool, String> {
    match v {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(format!(
            "deposit: {flag} must be 'true' or 'false', got '{other}'"
        )),
    }
}

fn do_deposit(args: &DepositArgs) -> Result<DepositReport, String> {
    let binary_bytes = fs::read(&args.binary)
        .map_err(|e| format!("deposit: read --binary {}: {e}", args.binary.display()))?;
    let sha256 = hex::encode(Sha256::digest(&binary_bytes));
    let size_bytes = binary_bytes.len() as u64;

    let product_dir = args.releases_dir.join(&args.product);
    let version_dir = product_dir.join(&args.version);
    let binary_target = version_dir.join(&args.platform);
    let manifest_target = version_dir.join("MANIFEST.json");
    let product_manifest_path = product_dir.join("MANIFEST.json");

    let existing_matches = if binary_target.exists() {
        let existing = fs::read(&binary_target)
            .map_err(|e| format!("deposit: read existing {}: {e}", binary_target.display()))?;
        let existing_sha = hex::encode(Sha256::digest(&existing));
        if existing_sha == sha256 {
            true
        } else if !args.force {
            return Err(format!(
                "deposit: {} already exists with different content (sha256 {existing_sha} vs \
                 new {sha256}) — pass --force to overwrite",
                binary_target.display()
            ));
        } else {
            false
        }
    } else {
        false
    };

    let path_value = match args.path_scheme {
        PathScheme::Fixed => format!("{}/{}", args.product, args.version),
        PathScheme::LatestAlias => format!("{}/latest/{}", args.product, args.platform),
    };

    let catalog_raw = fs::read_to_string(&args.catalog)
        .map_err(|e| format!("deposit: read --catalog {}: {e}", args.catalog.display()))?;
    let (new_catalog, catalog_changed, catalog_diff) = edit_products_yaml(
        &catalog_raw,
        &args.product,
        &args.version,
        &path_value,
        args.create_entry.as_ref(),
    )?;

    let product_manifest_created = !product_manifest_path.exists();

    if args.dry_run {
        return Ok(DepositReport {
            binary_path: binary_target,
            sha256,
            size_bytes,
            version_manifest_path: manifest_target,
            product_manifest_path,
            product_manifest_created,
            catalog_changed,
            catalog_diff,
            skipped_no_op: false,
        });
    }

    if existing_matches && !catalog_changed {
        return Ok(DepositReport {
            binary_path: binary_target,
            sha256,
            size_bytes,
            version_manifest_path: manifest_target,
            product_manifest_path,
            product_manifest_created: false,
            catalog_changed: false,
            catalog_diff: None,
            skipped_no_op: true,
        });
    }

    fs::create_dir_all(&version_dir)
        .map_err(|e| format!("deposit: create {}: {e}", version_dir.display()))?;
    fs::write(&binary_target, &binary_bytes)
        .map_err(|e| format!("deposit: write {}: {e}", binary_target.display()))?;

    if let Some(sig_path) = &args.sig {
        let sig_bytes = fs::read(sig_path)
            .map_err(|e| format!("deposit: read --sig {}: {e}", sig_path.display()))?;
        let sig_target = version_dir.join(format!("{}.sig", args.platform));
        fs::write(&sig_target, &sig_bytes)
            .map_err(|e| format!("deposit: write {}: {e}", sig_target.display()))?;
    }

    let built_at_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let manifest_json = serde_json::json!({
        "sha256": sha256,
        "size_bytes": size_bytes,
        "platform": args.platform,
        "version": args.version,
        "product": args.product,
        "built_at_unix": built_at_unix,
        "source_commit": args.source_commit,
    });
    fs::write(
        &manifest_target,
        serde_json::to_string_pretty(&manifest_json).unwrap(),
    )
    .map_err(|e| format!("deposit: write {}: {e}", manifest_target.display()))?;

    if product_manifest_created {
        let pm = serde_json::json!({ "requires_license": args.requires_license });
        fs::write(
            &product_manifest_path,
            serde_json::to_string_pretty(&pm).unwrap(),
        )
        .map_err(|e| format!("deposit: write {}: {e}", product_manifest_path.display()))?;
    }

    if catalog_changed {
        fs::write(&args.catalog, &new_catalog)
            .map_err(|e| format!("deposit: write {}: {e}", args.catalog.display()))?;
    }

    Ok(DepositReport {
        binary_path: binary_target,
        sha256,
        size_bytes,
        version_manifest_path: manifest_target,
        product_manifest_path,
        product_manifest_created,
        catalog_changed,
        catalog_diff,
        skipped_no_op: false,
    })
}

/// Surgical edit of `products.yaml`: locate the `- id: <product>` block, replace
/// only its `edition:`/`path:` line values (preserving each line's existing
/// quoting and indentation), and leave every other byte untouched. Returns
/// `(new_content, changed, human_readable_diff)`.
fn edit_products_yaml(
    raw: &str,
    product: &str,
    new_version: &str,
    new_path: &str,
    create_entry: Option<&NewEntry>,
) -> Result<(String, bool, Option<String>), String> {
    let lines: Vec<&str> = raw.lines().collect();
    let mut block_start = None;
    let mut block_end = lines.len();

    let mut i = 0;
    while i < lines.len() {
        let trimmed = lines[i].trim();
        if let Some(rest) = trimmed.strip_prefix("- id:") {
            let id = rest.trim().trim_matches('"');
            if id == product {
                block_start = Some(i);
                let mut j = i + 1;
                while j < lines.len() && !lines[j].trim().starts_with("- id:") {
                    j += 1;
                }
                block_end = j;
                break;
            }
        }
        i += 1;
    }

    let Some(start) = block_start else {
        let Some(entry) = create_entry else {
            return Err(format!(
                "deposit: product '{product}' not found in products.yaml — pass --create-entry \
                 with --name/--description/--license-tier/--price-usdc/--platform-label/--size-mb \
                 to add it"
            ));
        };
        let mut new_lines: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
        if !new_lines.is_empty() && !new_lines.last().unwrap().trim().is_empty() {
            new_lines.push(String::new());
        }
        new_lines.push(format!("  - id: {product}"));
        new_lines.push(format!("    name: {}", entry.name));
        new_lines.push(format!("    description: {}", entry.description));
        new_lines.push(format!("    edition: \"{new_version}\""));
        new_lines.push(format!("    platform: \"{}\"", entry.platform_label));
        new_lines.push(format!("    size_mb: {}", entry.size_mb));
        new_lines.push(format!("    path: {new_path}"));
        new_lines.push(format!("    license_tier: {}", entry.license_tier));
        new_lines.push(format!("    price_usdc: {}", entry.price_usdc));
        let result = new_lines.join("\n") + "\n";
        return Ok((
            result,
            true,
            Some(format!("appended new entry for '{product}'")),
        ));
    };

    let mut out_lines: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
    let mut changed = false;
    let mut diff_parts = Vec::new();

    for idx in start..block_end {
        let line = lines[idx];
        let trimmed = line.trim_start();
        let indent = &line[..line.len() - trimmed.len()];

        if let Some(rest) = trimmed.strip_prefix("edition:") {
            let old_val = rest.trim();
            let quoted = old_val.starts_with('"');
            let new_val_rendered = if quoted {
                format!("\"{new_version}\"")
            } else {
                new_version.to_string()
            };
            let new_line = format!("{indent}edition: {new_val_rendered}");
            if line != new_line {
                diff_parts.push(format!("edition: {old_val} -> {new_val_rendered}"));
                out_lines[idx] = new_line;
                changed = true;
            }
        } else if let Some(rest) = trimmed.strip_prefix("path:") {
            let old_val = rest.trim();
            let quoted = old_val.starts_with('"');
            let new_val_rendered = if quoted {
                format!("\"{new_path}\"")
            } else {
                new_path.to_string()
            };
            let new_line = format!("{indent}path: {new_val_rendered}");
            if line != new_line {
                diff_parts.push(format!("path: {old_val} -> {new_val_rendered}"));
                out_lines[idx] = new_line;
                changed = true;
            }
        }
    }

    let result = out_lines.join("\n") + if raw.ends_with('\n') { "\n" } else { "" };
    let diff = if diff_parts.is_empty() {
        None
    } else {
        Some(diff_parts.join("; "))
    };
    Ok((result, changed, diff))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static SEQ: AtomicUsize = AtomicUsize::new(0);

    /// Fresh, unique scratch directory under /tmp for one test — same pattern
    /// as `app-privategit-source-2`'s test module.
    fn scratch_dir(tag: &str) -> PathBuf {
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "xtask-deposit-test-{tag}-{}-{n}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    const FIXTURE: &str = r#"installers:
  - id: os-console
    name: PointSav Console OS
    description: Hosted application
    edition: "2026.05.144"
    platform: "macOS · Win · Linux"
    size_mb: 412
    path: os-console/2026.05.144
    license_tier: commercial
    price_usdc: 0

  - id: os-network-admin
    name: PointSav Network OS
    description: Orchestrates VM map
    edition: "0.1.0-beta.1"
    platform: "Linux x86_64"
    size_mb: 1
    path: os-network-admin/latest/x86_64
    license_tier: fsl
    price_usdc: 0
"#;

    fn base_args(scratch: &std::path::Path, binary: &std::path::Path) -> DepositArgs {
        DepositArgs {
            product: "os-console".to_string(),
            version: "0.3.0".to_string(),
            binary: binary.to_path_buf(),
            platform: "linux-x86_64".to_string(),
            releases_dir: scratch.join("releases"),
            catalog: scratch.join("products.yaml"),
            sig: None,
            source_commit: None,
            requires_license: false,
            path_scheme: PathScheme::Fixed,
            create_entry: None,
            force: false,
            dry_run: false,
        }
    }

    fn write_fixture_catalog(scratch: &std::path::Path) -> PathBuf {
        let path = scratch.join("products.yaml");
        fs::write(&path, FIXTURE).unwrap();
        path
    }

    fn write_binary(scratch: &std::path::Path, name: &str, bytes: &[u8]) -> PathBuf {
        let path = scratch.join(name);
        fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn sha256_and_size_computed_correctly() {
        let scratch = scratch_dir("sha");
        write_fixture_catalog(&scratch);
        let binary = write_binary(&scratch, "bin", b"hello world");
        let args = base_args(&scratch, &binary);

        let report = do_deposit(&args).unwrap();
        assert_eq!(report.sha256, hex::encode(Sha256::digest(b"hello world")));
        assert_eq!(report.size_bytes, 11);
    }

    #[test]
    fn fresh_deposit_writes_expected_tree_and_manifests() {
        let scratch = scratch_dir("fresh");
        write_fixture_catalog(&scratch);
        let binary = write_binary(&scratch, "bin", b"binary-bytes");
        let sig = write_binary(&scratch, "bin.sig", b"sig-bytes");
        let mut args = base_args(&scratch, &binary);
        args.sig = Some(sig);
        args.source_commit = Some("abc123".to_string());

        let report = do_deposit(&args).unwrap();
        assert!(!report.skipped_no_op);

        let version_dir = scratch.join("releases/os-console/0.3.0");
        assert_eq!(
            fs::read(version_dir.join("linux-x86_64")).unwrap(),
            b"binary-bytes"
        );
        assert_eq!(
            fs::read(version_dir.join("linux-x86_64.sig")).unwrap(),
            b"sig-bytes"
        );

        let manifest: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(version_dir.join("MANIFEST.json")).unwrap())
                .unwrap();
        assert_eq!(
            manifest["sha256"],
            hex::encode(Sha256::digest(b"binary-bytes"))
        );
        assert_eq!(manifest["size_bytes"], 12);
        assert_eq!(manifest["platform"], "linux-x86_64");
        assert_eq!(manifest["version"], "0.3.0");
        assert_eq!(manifest["product"], "os-console");
        assert_eq!(manifest["source_commit"], "abc123");
    }

    #[test]
    fn product_root_manifest_created_if_absent_untouched_if_present() {
        let scratch = scratch_dir("prodmanifest");
        write_fixture_catalog(&scratch);
        let binary = write_binary(&scratch, "bin", b"v1");
        let args = base_args(&scratch, &binary);

        let report = do_deposit(&args).unwrap();
        assert!(report.product_manifest_created);
        let pm_path = scratch.join("releases/os-console/MANIFEST.json");
        let pm: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&pm_path).unwrap()).unwrap();
        assert_eq!(pm["requires_license"], false);

        // Pre-seed a differently-shaped product-root manifest, then deposit a
        // new version — it must survive byte-for-byte.
        let scratch2 = scratch_dir("prodmanifest-existing");
        write_fixture_catalog(&scratch2);
        let binary2 = write_binary(&scratch2, "bin", b"v1");
        fs::create_dir_all(scratch2.join("releases/os-console")).unwrap();
        let existing_pm = r#"{"requires_license": true, "custom_field": "kept"}"#;
        fs::write(
            scratch2.join("releases/os-console/MANIFEST.json"),
            existing_pm,
        )
        .unwrap();
        let mut args2 = base_args(&scratch2, &binary2);
        args2.requires_license = true; // shouldn't matter — file already exists
        let report2 = do_deposit(&args2).unwrap();
        assert!(!report2.product_manifest_created);
        assert_eq!(
            fs::read_to_string(scratch2.join("releases/os-console/MANIFEST.json")).unwrap(),
            existing_pm
        );
    }

    #[test]
    fn identical_redeposit_is_idempotent_no_op() {
        let scratch = scratch_dir("idempotent");
        write_fixture_catalog(&scratch);
        let binary = write_binary(&scratch, "bin", b"same-bytes");
        let args = base_args(&scratch, &binary);

        // Deposit at a version already matching the catalog so the second run
        // is a true no-op on both the filesystem AND products.yaml.
        let mut args = args;
        args.version = "2026.05.144".to_string();
        let first = do_deposit(&args).unwrap();
        assert!(!first.skipped_no_op);

        let second = do_deposit(&args).unwrap();
        assert!(second.skipped_no_op);
        assert_eq!(second.sha256, first.sha256);
    }

    #[test]
    fn differing_bytes_without_force_errors_original_untouched() {
        let scratch = scratch_dir("conflict-noforce");
        write_fixture_catalog(&scratch);
        let binary = write_binary(&scratch, "bin", b"version-a");
        let args = base_args(&scratch, &binary);
        do_deposit(&args).unwrap();

        let binary2 = write_binary(&scratch, "bin2", b"version-b-different");
        let mut args2 = base_args(&scratch, &binary2);
        args2.force = false;
        let err = do_deposit(&args2).unwrap_err();
        assert!(
            err.contains("--force"),
            "error should mention --force: {err}"
        );

        let version_dir = scratch.join("releases/os-console/0.3.0");
        assert_eq!(
            fs::read(version_dir.join("linux-x86_64")).unwrap(),
            b"version-a",
            "original binary must be untouched after a rejected conflicting deposit"
        );
    }

    #[test]
    fn differing_bytes_with_force_overwrites() {
        let scratch = scratch_dir("conflict-force");
        write_fixture_catalog(&scratch);
        let binary = write_binary(&scratch, "bin", b"version-a");
        let args = base_args(&scratch, &binary);
        do_deposit(&args).unwrap();

        let binary2 = write_binary(&scratch, "bin2", b"version-b-different");
        let mut args2 = base_args(&scratch, &binary2);
        args2.force = true;
        let report = do_deposit(&args2).unwrap();
        assert!(!report.skipped_no_op);
        assert_eq!(
            report.sha256,
            hex::encode(Sha256::digest(b"version-b-different"))
        );

        let version_dir = scratch.join("releases/os-console/0.3.0");
        assert_eq!(
            fs::read(version_dir.join("linux-x86_64")).unwrap(),
            b"version-b-different"
        );
    }

    #[test]
    fn products_yaml_edit_touches_only_target_entrys_two_lines() {
        let (new_catalog, changed, _diff) =
            edit_products_yaml(FIXTURE, "os-console", "0.4.0", "os-console/0.4.0", None).unwrap();
        assert!(changed);

        // The os-network-admin block must be byte-identical.
        let na_before = FIXTURE.lines().skip(9).collect::<Vec<_>>().join("\n");
        let na_after = new_catalog.lines().skip(9).collect::<Vec<_>>().join("\n");
        assert_eq!(
            na_before, na_after,
            "os-network-admin block must be untouched"
        );

        assert!(new_catalog.contains(r#"edition: "0.4.0""#));
        assert!(new_catalog.contains("path: os-console/0.4.0"));
        assert!(!new_catalog.contains(r#"edition: "2026.05.144""#));

        // Every other byte (name/description/comments/blank lines) preserved.
        assert!(new_catalog.contains("name: PointSav Console OS"));
        assert!(new_catalog.contains("description: Hosted application"));
    }

    #[test]
    fn products_yaml_edit_is_idempotent_when_values_already_match() {
        let (_new_catalog, changed, diff) = edit_products_yaml(
            FIXTURE,
            "os-console",
            "2026.05.144",
            "os-console/2026.05.144",
            None,
        )
        .unwrap();
        assert!(!changed, "no-op edit must report changed=false");
        assert!(diff.is_none());
    }

    #[test]
    fn unknown_id_without_create_entry_errors() {
        let err = edit_products_yaml(
            FIXTURE,
            "os-nonexistent",
            "1.0.0",
            "os-nonexistent/1.0.0",
            None,
        )
        .unwrap_err();
        assert!(
            err.contains("--create-entry"),
            "error should mention --create-entry: {err}"
        );
    }

    #[test]
    fn unknown_id_with_create_entry_appends_new_block() {
        let entry = NewEntry {
            name: "New Product".to_string(),
            description: "A brand new product".to_string(),
            license_tier: "fsl".to_string(),
            price_usdc: 0,
            platform_label: "Linux server".to_string(),
            size_mb: 50,
        };
        let (new_catalog, changed, diff) = edit_products_yaml(
            FIXTURE,
            "os-newthing",
            "1.0.0",
            "os-newthing/1.0.0",
            Some(&entry),
        )
        .unwrap();
        assert!(changed);
        assert!(diff.unwrap().contains("appended"));
        assert!(new_catalog.contains("- id: os-newthing"));
        assert!(new_catalog.contains("name: New Product"));
        assert!(new_catalog.contains(r#"edition: "1.0.0""#));
        assert!(new_catalog.contains("path: os-newthing/1.0.0"));
        assert!(new_catalog.contains("license_tier: fsl"));
        // Original entries still present, untouched.
        assert!(new_catalog.contains(r#"edition: "2026.05.144""#));
    }

    #[test]
    fn path_scheme_latest_alias_vs_fixed() {
        let scratch = scratch_dir("path-scheme");
        write_fixture_catalog(&scratch);
        let binary = write_binary(&scratch, "bin", b"x");

        let mut fixed_args = base_args(&scratch, &binary);
        fixed_args.product = "os-network-admin".to_string();
        fixed_args.version = "0.2.0".to_string();
        fixed_args.path_scheme = PathScheme::Fixed;
        do_deposit(&fixed_args).unwrap();
        let catalog_after_fixed = fs::read_to_string(scratch.join("products.yaml")).unwrap();
        assert!(catalog_after_fixed.contains("path: os-network-admin/0.2.0"));

        let scratch2 = scratch_dir("path-scheme-latest");
        write_fixture_catalog(&scratch2);
        let binary2 = write_binary(&scratch2, "bin", b"x");
        let mut latest_args = base_args(&scratch2, &binary2);
        latest_args.product = "os-network-admin".to_string();
        latest_args.version = "0.2.0".to_string();
        latest_args.path_scheme = PathScheme::LatestAlias;
        do_deposit(&latest_args).unwrap();
        let catalog_after_latest = fs::read_to_string(scratch2.join("products.yaml")).unwrap();
        assert!(catalog_after_latest.contains("path: os-network-admin/latest/linux-x86_64"));
    }

    #[test]
    fn missing_required_flags_produce_usage_error() {
        let err = parse_args(&["--product".to_string(), "os-console".to_string()]).unwrap_err();
        assert!(err.contains("--version"));

        let err2 = parse_args(&[
            "--product".to_string(),
            "os-console".to_string(),
            "--version".to_string(),
            "1.0.0".to_string(),
            "--binary".to_string(),
            "/tmp/x".to_string(),
        ])
        .unwrap_err();
        assert!(err2.contains("--platform"));
    }

    #[test]
    fn dry_run_performs_zero_filesystem_mutation() {
        let scratch = scratch_dir("dry-run");
        write_fixture_catalog(&scratch);
        let binary = write_binary(&scratch, "bin", b"dry-run-bytes");
        let mut args = base_args(&scratch, &binary);
        args.dry_run = true;

        let catalog_before = fs::read_to_string(scratch.join("products.yaml")).unwrap();
        let report = do_deposit(&args).unwrap();
        assert!(!report.skipped_no_op);
        assert!(report.catalog_changed);

        let catalog_after = fs::read_to_string(scratch.join("products.yaml")).unwrap();
        assert_eq!(
            catalog_before, catalog_after,
            "dry-run must not write products.yaml"
        );
        assert!(
            !scratch.join("releases").exists(),
            "dry-run must not create the releases directory tree"
        );
    }
}
