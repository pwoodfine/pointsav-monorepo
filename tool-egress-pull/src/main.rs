use std::env;
use std::fs;
use std::process::Command;

const REMOTE_TARGET: &str = "node-cloud-relay";
const REMOTE_OUTBOUND: &str = "/opt/woodfine/cluster-totebox-personnel-1/service-egress/outbound-queue/";
const REMOTE_WIPE_QUEUE: &str = "/opt/woodfine/cluster-totebox-personnel-1/service-egress/wipe-queue/";

fn main() {
    println!("========================================================");
    println!(" 🧰 TOOL-EGRESS-PULL: THE ASYMMETRIC DIODE");
    println!("========================================================");

    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("[ERROR] Usage: tool-egress-pull <ABSOLUTE_USB_PATH>");
        std::process::exit(1);
    }

    let usb_root = &args[1];
    let staging_dir = format!("{}/.staging_queue", usb_root);
    let final_dir = format!("{}/cluster-totebox-personnel-1/service-email/maildir/cur", usb_root);

    fs::create_dir_all(&staging_dir).expect("[FATAL] Could not write to USB drive.");
    fs::create_dir_all(&final_dir).expect("[FATAL] Could not construct 1D Mirror.");

    println!("[SYSTEM] Initiating Secure Rsync Pull from Tier-2 Cloud Shield...");
    let remote_source = format!("{}:{}", REMOTE_TARGET, REMOTE_OUTBOUND);

    let rsync_status = Command::new("rsync")
        .args(["-avz", "--progress", "--rsync-path=sudo rsync", "-e", "ssh", &remote_source, &staging_dir])
        .status()
        .expect("[FATAL] rsync failure.");

    if !rsync_status.success() {
        eprintln!("[FATAL] Network transfer failed. Aborting sequence.");
        std::process::exit(1);
    }

    println!("\n[SYSTEM] Chunks secured locally. Initiating reassembly...");
    let mut processed_txs = std::collections::HashSet::new();

    if let Ok(entries) = fs::read_dir(&staging_dir) {
        for entry in entries.flatten() {
            let filename = entry.file_name().to_string_lossy().to_string();
            // THE FIX: Isolate the transaction ID up to the `.zst` extension
            if let Some(end_idx) = filename.find(".zst") {
                let tx_id = &filename[..end_idx];
                processed_txs.insert(tx_id.to_string());
            }
        }
    }

    if processed_txs.is_empty() {
        println!("  -> [STATUS] No new payloads extracted.");
        std::process::exit(0);
    }

    for tx_id in processed_txs {
        println!("[REBUILD] Processing Transaction: {}", tx_id);
        
        let chunk_pattern = format!("{}/{}.zst.part*", staging_dir, tx_id);
        let monolithic_zst = format!("{}/{}.zst", staging_dir, tx_id);
        let final_eml = format!("{}/{}.eml", final_dir, tx_id); 

        let cat_cmd = format!("cat {} > {}", chunk_pattern, monolithic_zst);
        Command::new("sh").args(["-c", &cat_cmd]).status().unwrap();

        let zstd_status = Command::new("zstd")
            .args(["-d", "--rm", &monolithic_zst, "-o", &final_eml])
            .status()
            .expect("[FATAL] zstd binary missing on host Mac.");

        if zstd_status.success() {
            println!("  -> [SUCCESS] {} successfully rebuilt and decompressed to 1D Mirror.", tx_id);

            println!("  -> [NETWORK] Transmitting WIPE authorization to Cloud Shield...");
            let wipe_cmd = format!("sudo touch {}{}.wipe", REMOTE_WIPE_QUEUE, tx_id);
            Command::new("ssh")
                .args([REMOTE_TARGET, &wipe_cmd])
                .status()
                .unwrap();
                
            let rm_cmd = format!("rm -f {}", chunk_pattern);
            Command::new("sh").args(["-c", &rm_cmd]).status().unwrap();
        } else {
            eprintln!("  -> [FAULT] Checksum failure during decompression. Wipe authorization withheld.");
        }
    }

    println!("========================================================");
    println!("[SUCCESS] Asymmetric Egress complete. Cloud SSD space will be reclaimed automatically.");
}
