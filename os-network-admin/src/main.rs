use std::time::Duration;

fn main() {
    let pairing_server =
        std::env::var("PAIRING_SERVER").unwrap_or_else(|_| "http://10.8.0.9:9205".to_string());

    println!("os-network-admin: polling {pairing_server}/v1/node-join/pending");
    println!("os-network-admin: press Ctrl-C to exit");
    println!("os-network-admin: to approve a pending code run:");
    println!("  curl -s -X POST {pairing_server}/v1/node-join/approve \\");
    println!("       -H 'Content-Type: application/json' -d '{{\"code\":\"XXXX-XXXX\"}}'");
    println!();

    loop {
        match ureq::get(&format!("{pairing_server}/v1/node-join/pending")).call() {
            Ok(resp) => {
                if let Ok(text) = resp.into_string() {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                        let pending = val["pending"].as_array().map(|a| a.len()).unwrap_or(0);
                        if pending == 0 {
                            print!("\r  [waiting for node-join requests...]    ");
                            use std::io::Write;
                            std::io::stdout().flush().ok();
                        } else {
                            println!("\n  {pending} pending node-join request(s):");
                            if let Some(items) = val["pending"].as_array() {
                                for item in items {
                                    let code = item["code"].as_str().unwrap_or("?");
                                    let node_id = item["node_id"].as_str().unwrap_or("?");
                                    let bottom = item["bottom"].as_str().unwrap_or("?");
                                    let arch = item["arch"].as_str().unwrap_or("?");
                                    let created = item["created_at"].as_str().unwrap_or("?");
                                    println!("  code={code}  node={node_id}  {bottom}/{arch}  at={created}");
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => eprintln!("\ros-network-admin: pairing server unreachable: {e}"),
        }
        std::thread::sleep(Duration::from_secs(5));
    }
}
