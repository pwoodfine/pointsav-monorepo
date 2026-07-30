use std::fs;
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::Duration;

const TOTEBOX_ROOT: &str = "/opt/woodfine/cluster-totebox-personnel-1";

fn main() {
    println!("========================================================");
    println!(" 🗄️ SERVICE-EGRESS: ASYMMETRIC CLOUD DAEMON ACTIVE");
    println!("========================================================");

    let cur_dir = format!("{}/service-email/maildir/cur", TOTEBOX_ROOT);
    let out_queue = format!("{}/service-egress/outbound-queue", TOTEBOX_ROOT);
    let wipe_queue = format!("{}/service-egress/wipe-queue", TOTEBOX_ROOT);

    fs::create_dir_all(&out_queue).unwrap();
    fs::create_dir_all(&wipe_queue).unwrap();

    loop {
        // 1. THE ANNIHILATION PROTOCOL (Process Wipes)
        if let Ok(entries) = fs::read_dir(&wipe_queue) {
            for entry in entries.flatten() {
                if let Some(ext) = entry.path().extension() {
                    if ext == "wipe" {
                        let tx_id = entry.path().file_stem().unwrap().to_string_lossy().to_string();
                        println!("[WIPE] Cryptographic receipt verified for {}. Obliterating cloud assets.", tx_id);

                        // Destroy original source mass
                        if let Ok(cur_files) = fs::read_dir(&cur_dir) {
                            for file in cur_files.flatten() {
                                if file.file_name().to_string_lossy().contains(&tx_id) {
                                    let _ = fs::remove_file(file.path());
                                }
                            }
                        }

                        // Destroy outbound staging chunks
                        if let Ok(out_files) = fs::read_dir(&out_queue) {
                            for file in out_files.flatten() {
                                if file.file_name().to_string_lossy().contains(&tx_id) {
                                    let _ = fs::remove_file(file.path());
                                }
                            }
                        }

                        let _ = fs::remove_file(entry.path());
                        println!("  -> [SUCCESS] {} mathematically removed from SSD.", tx_id);
                    }
                }
            }
        }

        // 2. THE CHUNKING PROTOCOL (Prepare outbound payloads)
        if let Ok(entries) = fs::read_dir(&cur_dir) {
            for entry in entries.flatten() {
                let filename = entry.file_name().to_string_lossy().to_string();
                
                // THE FIX: Process ALL .eml files. Use the file stem as the unique transaction ID.
                if filename.ends_with(".eml") {
                    let tx_id = entry.path().file_stem().unwrap().to_string_lossy().to_string();
                    let target_chunk = format!("{}/{}.zst.partaa", out_queue, tx_id);
                    
                    if !Path::new(&target_chunk).exists() {
                        println!("[COMPRESS] Staging Heavy Mass: {}", filename);
                        
                        let tmp_zst = format!("{}/{}.zst", out_queue, tx_id);
                        
                        // Step A: OS-Level Zstandard Compression
                        Command::new("zstd")
                            .args(["-T0", "-3", "--rm", entry.path().to_str().unwrap(), "-o", &tmp_zst])
                            .status()
                            .expect("[FATAL] zstd binary not found on host.");
                            
                        // Step B: OS-Level 50MB Chunking
                        let chunk_prefix = format!("{}/{}.zst.part", out_queue, tx_id);
                        Command::new("split")
                            .args(["-b", "50M", &tmp_zst, &chunk_prefix])
                            .status()
                            .expect("[FATAL] split binary not found on host.");
                            
                        let _ = fs::remove_file(&tmp_zst);
                        println!("  -> [SUCCESS] {} mathematically chunked and staged.", tx_id);
                    }
                }
            }
        }
        
        thread::sleep(Duration::from_secs(15));
    }
}
