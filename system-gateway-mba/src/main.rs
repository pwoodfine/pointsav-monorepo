use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tiny_http::{Method, Response, Server};

fn deployment_dir() -> String {
    std::env::var("MBA_DEPLOYMENT_DIR")
        .unwrap_or_else(|_| "/home/mathew/deployments/woodfine-fleet-deployment".to_string())
}

fn main() {
    let server = Server::http("0.0.0.0:8080").expect("[CRITICAL] Failed to bind to port 8080");
    println!("[SYSTEM] DIAGNOSTIC GATEWAY ACTIVE ON 8080...");
    println!("[SYSTEM] Awaiting payloads to audit rejection logic...");

    for mut request in server.incoming_requests() {
        if request.method() != &Method::Post || request.url() != "/api/ingest" {
            let _ = request.respond(Response::empty(404));
            continue;
        }

        println!("--------------------------------------------------");
        println!("[DIAGNOSTIC] Incoming payload detected from Relay.");

        let mut content = String::new();
        if let Err(e) = request.as_reader().read_to_string(&mut content) {
            println!("[ERROR] Failed to read HTTP body: {:?}", e);
            let _ = request.respond(Response::empty(400));
            continue;
        }
        println!(
            "[DIAGNOSTIC] Payload stream read successfully. Size: {} bytes",
            content.len()
        );

        match serde_json::from_str::<serde_json::Value>(&content) {
            Ok(json) => {
                println!("[DIAGNOSTIC] JSON structurally valid.");

                let dest_archive = json
                    .get("destination_archive")
                    .and_then(|v| v.as_str())
                    .unwrap_or("default");
                let target_service = json
                    .get("target_service")
                    .and_then(|v| v.as_str())
                    .unwrap_or("service-input");
                let coa = json
                    .get("chart_of_accounts")
                    .and_then(|v| v.as_str())
                    .unwrap_or("00-Uncategorized");

                println!(
                    "[DIAGNOSTIC] Extracted Routing: {} -> {}",
                    dest_archive, target_service
                );

                let mut out_dir = PathBuf::from(deployment_dir());
                out_dir.push(dest_archive);
                out_dir.push("service-fs/data");
                out_dir.push(target_service);
                out_dir.push("source");

                println!("[DIAGNOSTIC] Physical Directory Target: {:?}", out_dir);

                if let Err(e) = fs::create_dir_all(&out_dir) {
                    println!(
                        "[ERROR] File System Permission Denied (Cannot create directory): {:?}",
                        e
                    );
                    let _ = request.respond(Response::empty(500));
                    continue;
                }

                let timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                let worm_id = format!("{}_{}", coa, timestamp);
                let out_path = out_dir.join(format!("{}.json", worm_id));

                println!("[DIAGNOSTIC] Attempting WORM Write: {:?}", out_path);

                match fs::write(&out_path, &content) {
                    Ok(_) => {
                        println!("[SUCCESS] WORM Write Successful!");
                        let _ = request.respond(Response::empty(200));
                    }
                    Err(e) => {
                        println!("[ERROR] File System Write Failed: {:?}", e);
                        let _ = request.respond(Response::empty(400));
                    }
                }
            }
            Err(e) => {
                println!(
                    "[ERROR] JSON Parsing Failed. The Relay is mangling the payload: {:?}",
                    e
                );
                let preview: String = content.chars().take(250).collect();
                println!("[DIAGNOSTIC] Payload Header Preview: {}", preview);
                let _ = request.respond(Response::empty(400));
            }
        }
    }
}
