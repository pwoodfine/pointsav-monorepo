/// tool-wallet — Polygon USDC payment watcher + receipt writer + key utilities
///
/// Single-tenant (PointSav vendor). Watches for inbound USDC transfers to
/// POLYGON_WALLET_ADDRESS on Polygon PoS. On confirmation, writes receipts to
/// local flat files at RECEIPTS_DIR.
///
/// Also provides: `keygen` (Ed25519 signing keypair for license issuance) and
/// `generate-seed` (BIP-39 mnemonic + master Polygon wallet address).
///
/// service-wallet (Doctrine claim #53) is the multi-tenant Ring 2 service
/// for customer-side Reverse-Flow revenue. tool-wallet is distinct: it is
/// a vendor-side utility for PointSav receiving inbound license payments.
use anyhow::{Context, Result};
use bip32::{DerivationPath, XPrv};
use bip39::{Language, Mnemonic};
use chrono::Utc;
use clap::{Parser, Subcommand};
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sha3::Keccak256;
use std::{
    collections::HashMap,
    fs,
    io::{Read, Seek, SeekFrom, Write},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

// USDC contract on Polygon PoS (native USDC, not bridged)
const USDC_CONTRACT: &str = "0x3c499c542cef5e3811e1192ce70d8cc03d5c3359";
// ERC-20 Transfer event topic
const TRANSFER_TOPIC: &str = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";

// Known license prices in 6-decimal USDC units → product_id
// $1.00 USDC = 1_000_000 units; $19.00 USDC = 19_000_000 units
const PRICE_MAP: &[(u64, &str)] = &[
    (1_000_000, "os-privategit"),      // Apache 2.0 — $1.00 USDC
    (19_000_000, "os-privategit-fsl"), // FSL — $19.00 USDC
];

// ── CLI ───────────────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "tool-wallet",
    about = "Polygon USDC payment watcher and receipt writer"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Watch for incoming USDC payments and write receipts to local flat files
    Watch(WatchArgs),
    /// Check whether a given tx_hash is a confirmed payment for a product
    Check(CheckArgs),
    /// Generate a fresh HD-derived payment address for a pending order (v0.0.2)
    Address(AddressArgs),
    /// Generate a fresh Ed25519 signing keypair for LICENSE_SIGNING_KEY + VERIFY_KEY_PUB
    Keygen,
    /// Generate a fresh BIP-39 mnemonic, write to file, and print the master Polygon wallet address
    GenerateSeed(GenerateSeedArgs),
    /// Export receipts from local flat files to CSV for bookkeeping
    Export(ExportArgs),
}

#[derive(Parser)]
struct WatchArgs {
    #[arg(env = "POLYGON_RPC_URL", long, help = "Polygon JSON-RPC endpoint")]
    rpc_url: String,
    #[arg(
        env = "POLYGON_WALLET_ADDRESS",
        long,
        help = "Receiving wallet address"
    )]
    wallet_address: String,
    #[arg(
        env = "RECEIPTS_DIR",
        long,
        default_value = "/var/lib/local-software/receipts"
    )]
    receipts_dir: String,
    #[arg(long, default_value = "30", help = "Poll interval in seconds")]
    poll_secs: u64,
    #[arg(
        env = "POLYGON_RPC_FALLBACK_URLS",
        long,
        value_delimiter = ',',
        help = "Comma-separated fallback Polygon JSON-RPC endpoints tried in order on primary failure"
    )]
    rpc_fallback_urls: Vec<String>,
    #[arg(
        env = "FS_ENDPOINT",
        long,
        help = "service-fs WORM ledger endpoint; if set, receipts are POST-ed asynchronously for Rekor anchoring"
    )]
    fs_endpoint: Option<String>,
}

#[derive(Parser)]
struct GenerateSeedArgs {
    #[arg(
        long,
        help = "Path to write the generated BIP-39 mnemonic (chmod 0600)"
    )]
    output: PathBuf,
}

#[derive(Parser)]
struct ExportArgs {
    #[arg(
        long,
        default_value = "csv",
        help = "Output format (only csv supported)"
    )]
    format: String,
    #[arg(
        env = "RECEIPTS_DIR",
        long,
        default_value = "/var/lib/local-software/receipts",
        help = "Directory containing receipt JSON files"
    )]
    receipts_dir: String,
    #[arg(long, help = "Earliest date to include, YYYY-MM-DD (inclusive)")]
    from: Option<String>,
    #[arg(long, help = "Latest date to include, YYYY-MM-DD (inclusive)")]
    to: Option<String>,
    #[arg(long, short = 'o', help = "Output file path (default: stdout)")]
    output: Option<PathBuf>,
}

#[derive(Parser)]
struct CheckArgs {
    #[arg(help = "Transaction hash to verify")]
    tx_hash: String,
    #[arg(env = "POLYGON_RPC_URL", long)]
    rpc_url: String,
    #[arg(
        env = "POLYGON_RPC_FALLBACK_URLS",
        long,
        value_delimiter = ',',
        help = "Comma-separated fallback Polygon JSON-RPC endpoints tried in order on primary failure"
    )]
    rpc_fallback_urls: Vec<String>,
    #[arg(env = "POLYGON_WALLET_ADDRESS", long)]
    wallet_address: String,
}

#[derive(Parser)]
struct AddressArgs {
    #[arg(help = "Order ID — used to derive a unique per-order address")]
    order_id: String,
    #[arg(
        env = "WALLET_SEED_PATH",
        long,
        help = "Path to BIP-39 mnemonic or 64-byte hex seed file"
    )]
    wallet_seed_path: Option<String>,
    #[arg(
        env = "ORDER_INDEX_PATH",
        long,
        default_value = "/var/lib/local-software/data/order-index.json",
        help = "Path to per-order derivation index JSON file"
    )]
    order_index_path: String,
}

// ── Order index (HD address counter) ─────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Default)]
struct OrderIndex {
    next_index: u32,
    orders: HashMap<String, u32>,
}

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Debug, Clone)]
struct LicenseReceipt {
    product_id: String,
    license_tier: String, // "apache" | "fsl"
    version: String,
    customer_ref: String,
    price_usdc: u64,
    tx_hash: String,
    chain: String,
    confirmed_at: String,
    block_number: u64,
    license_key: String,
}

// ── RPC helpers ───────────────────────────────────────────────────────────────

async fn rpc_call(
    client: &reqwest::Client,
    rpc_url: &str,
    method: &str,
    params: Value,
) -> Result<Value> {
    let body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params
    });
    let resp = client
        .post(rpc_url)
        .json(&body)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .context("RPC request failed")?;
    let json: Value = resp.json().await.context("RPC response parse")?;
    if let Some(err) = json.get("error") {
        anyhow::bail!("RPC error: {err}");
    }
    Ok(json["result"].clone())
}

async fn try_rpc_with_fallback(
    client: &reqwest::Client,
    urls: &[String],
    method: &str,
    params: Value,
) -> Result<Value> {
    let mut last_err = anyhow::anyhow!("no RPC URLs configured");
    for url in urls {
        match rpc_call(client, url, method, params.clone()).await {
            Ok(v) => return Ok(v),
            Err(e) => {
                tracing::warn!(url = %url, method = %method, "RPC call failed, trying fallback: {e}");
                last_err = e;
            }
        }
    }
    tracing::error!(method = %method, "all {} RPC endpoints failed", urls.len());
    Err(last_err)
}

fn pad_address(addr: &str) -> String {
    let clean = addr.trim_start_matches("0x").to_lowercase();
    format!("0x{:0>64}", clean)
}

fn parse_usdc_amount(data: &str) -> u64 {
    // data is a 32-byte hex uint256 (0x-prefixed)
    let hex = data.trim_start_matches("0x");
    if hex.len() < 64 {
        return 0;
    }
    // Take last 8 hex bytes (4 bytes / 32 bits) for amounts < u64::MAX
    let tail = &hex[hex.len().saturating_sub(16)..];
    u64::from_str_radix(tail, 16).unwrap_or(0)
}

fn generate_license_key(product_id: &str, tx_hash: &str, customer_ref: &str) -> String {
    let h = hex::encode(Sha256::digest(
        format!("{product_id}:{tx_hash}:{customer_ref}").as_bytes(),
    ));
    format!("{}-{}-{}-{}", &h[0..8], &h[8..16], &h[16..24], &h[24..32])
}

// ── Receipt writer ────────────────────────────────────────────────────────────

async fn confirm_and_write_receipt(receipt: &LicenseReceipt, receipts_dir: &str) -> Result<()> {
    let now = Utc::now();
    let local_path = PathBuf::from(receipts_dir)
        .join(now.format("%Y").to_string())
        .join(now.format("%m").to_string())
        .join(format!("{}.json", receipt.tx_hash));

    if let Some(parent) = local_path.parent() {
        fs::create_dir_all(parent).context("create receipts dir")?;
    }
    let raw = serde_json::to_string_pretty(receipt).context("serialize receipt")?;
    fs::write(&local_path, &raw).context("write receipt")?;
    tracing::info!(tx_hash = %receipt.tx_hash, path = %local_path.display(), "receipt written");
    Ok(())
}

// ── tx-log writer ─────────────────────────────────────────────────────────────

async fn append_tx_log(receipt: &LicenseReceipt, receipts_dir: &str) {
    use tokio::io::AsyncWriteExt;
    let log_path = PathBuf::from(receipts_dir)
        .parent()
        .unwrap_or(Path::new("/var/lib/local-software"))
        .join("tx-log.jsonl");
    let entry = json!({
        "date": &receipt.confirmed_at[..10],
        "sku": &receipt.product_id,
        "tx_hash": &receipt.tx_hash,
        "crypto_amount_usdc": receipt.price_usdc as f64 / 1_000_000.0,
        "license_tier": &receipt.license_tier,
        "spot_rate_cad": null,
        "cad_equivalent": null,
        "chain": &receipt.chain,
    });
    let line = serde_json::to_string(&entry).unwrap_or_default() + "\n";
    match tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .await
    {
        Ok(mut f) => {
            if let Err(e) = f.write_all(line.as_bytes()).await {
                tracing::warn!("tx-log.jsonl write failed: {e}");
            } else {
                tracing::info!(sku = %receipt.product_id, "tx-log.jsonl appended");
            }
        }
        Err(e) => tracing::warn!("tx-log.jsonl open failed: {e}"),
    }
}

// ── service-fs relay (fire-and-forget; Rekor anchoring via local-fs-anchoring) ─

fn post_receipt_to_fs(client: &reqwest::Client, endpoint: &str, receipt: &LicenseReceipt) {
    let client = client.clone();
    let receipt = receipt.clone();
    let now = chrono::Utc::now();
    let url = format!(
        "{}/vault/source/license-receipts/{}/{}/{}.json",
        endpoint.trim_end_matches('/'),
        now.format("%Y"),
        now.format("%m"),
        receipt.tx_hash
    );
    tokio::spawn(async move {
        match client
            .post(&url)
            .json(&receipt)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                tracing::info!(tx_hash = %receipt.tx_hash, "receipt posted to service-fs");
            }
            Ok(resp) => {
                tracing::warn!(
                    tx_hash = %receipt.tx_hash,
                    status = %resp.status(),
                    "service-fs returned non-success status"
                );
            }
            Err(e) => {
                tracing::warn!(tx_hash = %receipt.tx_hash, "service-fs post failed: {e}");
            }
        }
    });
}

// ── watch ─────────────────────────────────────────────────────────────────────

async fn watch(args: WatchArgs) -> Result<()> {
    tracing::info!(
        wallet = %args.wallet_address,
        poll_secs = args.poll_secs,
        "tool-wallet: watching for USDC transfers on Polygon PoS"
    );

    let client = reqwest::Client::new();
    let wallet_padded = pad_address(&args.wallet_address);
    let all_rpcs: Vec<String> = std::iter::once(args.rpc_url.clone())
        .chain(args.rpc_fallback_urls.iter().cloned())
        .collect();

    // Fetch current block to start from
    let head_hex = try_rpc_with_fallback(&client, &all_rpcs, "eth_blockNumber", json!([])).await?;
    let head_str = head_hex.as_str().unwrap_or("0x0").trim_start_matches("0x");
    let mut last_block = u64::from_str_radix(head_str, 16)
        .unwrap_or(0)
        .saturating_sub(1);
    tracing::info!(start_block = last_block, "starting watch from block");

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(args.poll_secs)).await;

        // Get current block
        let head_hex =
            match try_rpc_with_fallback(&client, &all_rpcs, "eth_blockNumber", json!([])).await {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!("eth_blockNumber failed: {e}");
                    continue;
                }
            };
        let head_str = head_hex.as_str().unwrap_or("0x0").trim_start_matches("0x");
        let current_block = match u64::from_str_radix(head_str, 16) {
            Ok(n) => n,
            Err(_) => continue,
        };

        if current_block <= last_block {
            continue;
        }

        let from_block = format!("0x{:x}", last_block + 1);
        let to_block = format!("0x{:x}", current_block.saturating_sub(1)); // 1 confirmation

        let logs = match try_rpc_with_fallback(
            &client,
            &all_rpcs,
            "eth_getLogs",
            json!([{
                "address": USDC_CONTRACT,
                "fromBlock": from_block,
                "toBlock": to_block,
                "topics": [TRANSFER_TOPIC, null, wallet_padded]
            }]),
        )
        .await
        {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("eth_getLogs failed: {e}");
                continue;
            }
        };

        if let Some(log_arr) = logs.as_array() {
            for log in log_arr {
                let tx_hash = log["transactionHash"].as_str().unwrap_or("").to_lowercase();
                let block_hex = log["blockNumber"]
                    .as_str()
                    .unwrap_or("0x0")
                    .trim_start_matches("0x");
                let block_number = u64::from_str_radix(block_hex, 16).unwrap_or(0);
                let data = log["data"].as_str().unwrap_or("0x");
                let amount = parse_usdc_amount(data);

                let from_padded = log["topics"].get(1).and_then(|v| v.as_str()).unwrap_or("");
                let customer_ref = format!(
                    "0x{}",
                    from_padded.trim_start_matches("0x").trim_start_matches('0')
                );

                let product_id = PRICE_MAP
                    .iter()
                    .find(|(p, _)| *p == amount)
                    .map(|(_, id)| id.to_string())
                    .unwrap_or_else(|| format!("unknown-{amount}"));

                let license_tier = if amount == 1_000_000 {
                    "apache"
                } else if amount == 19_000_000 {
                    "fsl"
                } else {
                    "unknown"
                }
                .to_string();

                let license_key = generate_license_key(&product_id, &tx_hash, &customer_ref);

                let receipt = LicenseReceipt {
                    product_id: product_id.clone(),
                    license_tier: license_tier.clone(),
                    version: "0.0.1".into(), // matches promoted binary version
                    customer_ref: customer_ref.clone(),
                    price_usdc: amount,
                    tx_hash: tx_hash.clone(),
                    chain: "polygon-pos".into(),
                    confirmed_at: Utc::now().to_rfc3339(),
                    block_number,
                    license_key: license_key.clone(),
                };

                tracing::info!(
                    tx_hash = %tx_hash,
                    product_id = %product_id,
                    amount_usdc = amount as f64 / 1_000_000.0,
                    "confirmed USDC payment — writing receipt"
                );

                match confirm_and_write_receipt(&receipt, &args.receipts_dir).await {
                    Ok(()) => {
                        append_tx_log(&receipt, &args.receipts_dir).await;
                        if let Some(ref endpoint) = args.fs_endpoint {
                            post_receipt_to_fs(&client, endpoint, &receipt);
                        }
                    }
                    Err(e) => tracing::error!("receipt write failed: {e:#}"),
                }
            }
        }

        last_block = current_block;
    }
}

// ── check ─────────────────────────────────────────────────────────────────────

async fn check(args: CheckArgs) -> Result<()> {
    let client = reqwest::Client::new();
    let tx_hash = args.tx_hash.to_lowercase();
    let wallet_padded = pad_address(&args.wallet_address);
    let all_rpcs: Vec<String> = std::iter::once(args.rpc_url.clone())
        .chain(args.rpc_fallback_urls.iter().cloned())
        .collect();

    let receipt = try_rpc_with_fallback(
        &client,
        &all_rpcs,
        "eth_getTransactionReceipt",
        json!([tx_hash]),
    )
    .await?;

    if receipt.is_null() {
        let out = json!({"confirmed": false, "reason": "transaction not found or not mined"});
        println!("{}", serde_json::to_string(&out)?);
        std::process::exit(1);
    }

    // Verify contract
    let to = receipt["to"].as_str().unwrap_or("").to_lowercase();
    if to != USDC_CONTRACT {
        let out = json!({"confirmed": false, "reason": "transaction is not a USDC transfer"});
        println!("{}", serde_json::to_string(&out)?);
        std::process::exit(1);
    }

    // Find matching Transfer log
    let logs = receipt["logs"].as_array().cloned().unwrap_or_default();
    let matching = logs.iter().find(|log| {
        let topics = log["topics"].as_array();
        let topic0 = topics
            .and_then(|t| t.first())
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let topic2 = topics
            .and_then(|t| t.get(2))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        topic0.to_lowercase() == TRANSFER_TOPIC
            && topic2.to_lowercase() == wallet_padded.to_lowercase()
    });

    match matching {
        Some(log) => {
            let data = log["data"].as_str().unwrap_or("0x");
            let amount = parse_usdc_amount(data);
            let block_hex = receipt["blockNumber"]
                .as_str()
                .unwrap_or("0x0")
                .trim_start_matches("0x");
            let block_number = u64::from_str_radix(block_hex, 16).unwrap_or(0);
            let from_padded = log["topics"]
                .as_array()
                .and_then(|t| t.get(1))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let from = format!(
                "0x{}",
                from_padded.trim_start_matches("0x").trim_start_matches('0')
            );

            let out = json!({
                "confirmed": true,
                "amount_usdc": amount as f64 / 1_000_000.0,
                "amount_units": amount,
                "from": from,
                "block": block_number,
                "tx_hash": tx_hash
            });
            println!("{}", serde_json::to_string(&out)?);
            Ok(())
        }
        None => {
            let out = json!({
                "confirmed": false,
                "reason": "no matching USDC Transfer log to this wallet address"
            });
            println!("{}", serde_json::to_string(&out)?);
            std::process::exit(1);
        }
    }
}

// ── keygen ────────────────────────────────────────────────────────────────────

fn keygen() {
    let signing_key = SigningKey::generate(&mut OsRng);
    let verifying_key: VerifyingKey = signing_key.verifying_key();
    println!(
        "LICENSE_SIGNING_KEY={}",
        hex::encode(signing_key.to_bytes())
    );
    println!("VERIFY_KEY_PUB={}", hex::encode(verifying_key.to_bytes()));
}

// ── generate-seed ─────────────────────────────────────────────────────────────

fn generate_seed(args: GenerateSeedArgs) -> Result<()> {
    let mnemonic = Mnemonic::generate_in(Language::English, 24)
        .map_err(|e| anyhow::anyhow!("mnemonic generation failed: {e}"))?;
    let mnemonic_str = mnemonic.to_string();

    fs::write(&args.output, &mnemonic_str)
        .with_context(|| format!("writing seed to {}", args.output.display()))?;
    fs::set_permissions(&args.output, fs::Permissions::from_mode(0o600))
        .context("setting seed file permissions to 0600")?;

    let seed_bytes: [u8; 64] = mnemonic.to_seed("");
    let eth_address = derive_eth_address_at_path(&seed_bytes, "m/44'/60'/0'/0/0")?;

    eprintln!("seed written to {}", args.output.display());
    println!("POLYGON_WALLET_ADDRESS={eth_address}");
    Ok(())
}

// ── Ethereum address derivation helper ────────────────────────────────────────

fn derive_eth_address_at_path(seed_bytes: &[u8; 64], path_str: &str) -> Result<String> {
    let path: DerivationPath = path_str
        .parse()
        .with_context(|| format!("parsing derivation path: {path_str}"))?;
    let child_xprv =
        XPrv::derive_from_path(seed_bytes, &path).context("BIP-32 key derivation failed")?;
    let xpub = child_xprv.public_key();
    let vk = xpub.public_key();
    let encoded = vk.to_encoded_point(false);
    let raw = &encoded.as_bytes()[1..];
    let hash = Keccak256::digest(raw);
    let addr_bytes = &hash[12..];
    Ok(format!("0x{}", hex::encode(addr_bytes)))
}

// ── address ───────────────────────────────────────────────────────────────────

async fn address(args: AddressArgs) -> Result<()> {
    let seed_path = match args.wallet_seed_path {
        Some(p) => p,
        None => {
            eprintln!(
                "address: WALLET_SEED_PATH not configured — set env var to activate HD address derivation"
            );
            std::process::exit(2);
        }
    };

    let seed_content = fs::read_to_string(&seed_path)
        .with_context(|| format!("reading WALLET_SEED_PATH: {seed_path}"))?;
    let seed_content = seed_content.trim();

    let seed_bytes: [u8; 64] =
        if seed_content.len() == 128 && seed_content.chars().all(|c| c.is_ascii_hexdigit()) {
            let decoded = hex::decode(seed_content).context("decoding hex seed")?;
            decoded
                .try_into()
                .map_err(|_| anyhow::anyhow!("hex seed must be exactly 64 bytes (128 hex chars)"))?
        } else {
            let mnemonic: Mnemonic = seed_content
                .parse()
                .map_err(|e| anyhow::anyhow!("invalid BIP-39 mnemonic: {e}"))?;
            mnemonic.to_seed("")
        };

    let derivation_index = assign_order_index(&args.order_index_path, &args.order_id)
        .context("assigning order derivation index")?;

    let path_str = format!("m/44'/60'/0'/0/{derivation_index}");
    let eth_address = derive_eth_address_at_path(&seed_bytes, &path_str)?;

    let output = json!({
        "order_id": args.order_id,
        "address": eth_address,
        "derivation_index": derivation_index,
        "derivation_path": path_str,
        "chain": "polygon-pos",
        "token": "USDC"
    });

    tracing::info!(
        order_id = %args.order_id,
        address = %eth_address,
        derivation_index = derivation_index,
        "HD address assigned"
    );

    println!("{}", serde_json::to_string(&output)?);
    Ok(())
}

fn assign_order_index(index_path: &str, order_id: &str) -> Result<u32> {
    use fd_lock::RwLock;
    use std::fs::OpenOptions;

    if let Some(parent) = Path::new(index_path).parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating order index dir: {}", parent.display()))?;
        }
    }

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(index_path)
        .with_context(|| format!("opening order index: {index_path}"))?;

    let mut lock = RwLock::new(file);
    let mut guard = lock.write().context("acquiring order index write lock")?;

    let mut content = String::new();
    guard
        .read_to_string(&mut content)
        .context("reading order index")?;

    let mut index: OrderIndex = if content.trim().is_empty() {
        OrderIndex::default()
    } else {
        serde_json::from_str(&content).context("parsing order index JSON")?
    };

    // Idempotent: same order_id returns the cached derivation slot
    if let Some(&cached) = index.orders.get(order_id) {
        return Ok(cached);
    }

    let assigned = index.next_index;
    index.next_index = index.next_index.saturating_add(1);
    index.orders.insert(order_id.to_string(), assigned);

    let new_json = serde_json::to_string_pretty(&index)?;
    guard
        .seek(SeekFrom::Start(0))
        .context("seeking order index")?;
    guard.set_len(0).context("truncating order index")?;
    guard
        .write_all(new_json.as_bytes())
        .context("writing order index")?;

    Ok(assigned)
}

// ── export ────────────────────────────────────────────────────────────────────

fn run_export(args: ExportArgs) -> Result<()> {
    if args.format != "csv" {
        anyhow::bail!(
            "unsupported format '{}'; only 'csv' is supported",
            args.format
        );
    }

    let receipts_dir = PathBuf::from(&args.receipts_dir);
    if !receipts_dir.exists() {
        anyhow::bail!(
            "receipts directory does not exist: {}",
            receipts_dir.display()
        );
    }

    let mut receipts: Vec<LicenseReceipt> = Vec::new();

    for year_entry in fs::read_dir(&receipts_dir).context("reading receipts dir")? {
        let year_entry = year_entry?;
        if !year_entry.file_type()?.is_dir() {
            continue;
        }
        for month_entry in fs::read_dir(year_entry.path()).context("reading month dir")? {
            let month_entry = month_entry?;
            if !month_entry.file_type()?.is_dir() {
                continue;
            }
            for file_entry in fs::read_dir(month_entry.path()).context("reading receipt files")? {
                let file_entry = file_entry?;
                let path = file_entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                let content = fs::read_to_string(&path)
                    .with_context(|| format!("reading {}", path.display()))?;
                match serde_json::from_str::<LicenseReceipt>(&content) {
                    Ok(r) => receipts.push(r),
                    Err(e) => {
                        eprintln!(
                            "warning: skipping unparseable receipt {}: {e}",
                            path.display()
                        );
                    }
                }
            }
        }
    }

    // Date filters — compare against confirmed_at (RFC 3339); from uses full string,
    // to uses YYYY-MM-DD prefix of confirmed_at
    if let Some(ref from) = args.from {
        receipts.retain(|r| r.confirmed_at.as_str() >= from.as_str());
    }
    if let Some(ref to) = args.to {
        let to_str = to.as_str();
        receipts.retain(|r| r.confirmed_at.get(..10).unwrap_or("") <= to_str);
    }

    receipts.sort_by(|a, b| a.confirmed_at.cmp(&b.confirmed_at));

    let header =
        "tx_hash,product_id,license_tier,price_usdc_units,price_usdc_display,block_number,confirmed_at,chain,customer_ref\n";
    let mut out = String::with_capacity(header.len() + receipts.len() * 128);
    out.push_str(header);

    for r in &receipts {
        let price_display = format!("{:.6}", r.price_usdc as f64 / 1_000_000.0);
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{}\n",
            r.tx_hash,
            r.product_id,
            r.license_tier,
            r.price_usdc,
            price_display,
            r.block_number,
            r.confirmed_at,
            r.chain,
            r.customer_ref,
        ));
    }

    match args.output {
        Some(ref path) => {
            fs::write(path, &out).with_context(|| format!("writing CSV to {}", path.display()))?;
            eprintln!("wrote {} receipts to {}", receipts.len(), path.display());
        }
        None => {
            std::io::stdout()
                .write_all(out.as_bytes())
                .context("writing CSV to stdout")?;
        }
    }

    Ok(())
}

// ── Main ──────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    // Write logs to stderr so stdout carries only machine-readable output (JSON).
    // The address/check subcommands are called as subprocesses by app-privategit-marketplace,
    // which captures stdout to parse JSON responses.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .init();

    let cli = Cli::parse();

    match cli.command {
        Command::Watch(args) => watch(args).await,
        Command::Check(args) => check(args).await,
        Command::Address(args) => address(args).await,
        Command::Keygen => {
            keygen();
            Ok(())
        }
        Command::GenerateSeed(args) => generate_seed(args),
        Command::Export(args) => run_export(args),
    }
}
