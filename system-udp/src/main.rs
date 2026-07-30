use std::env;
use std::net::UdpSocket;
use serde::{Serialize, Deserialize};
use chrono::Utc;

/// PointSav Digital Systems: Zero-Broker UDP Mesh
/// Standard: topic-system-udp

#[derive(Serialize, Deserialize, Debug)]
struct MeshPayload {
    sender_id: String,
    intent: String,
    target: String,
    timestamp: String,
}

// The PointSav Private Network (PPN) WireGuard Subnet
const MESH_PORT: u16 = 8090;
const BROADCAST_ADDR: &str = "10.8.0.255";

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("[ERROR] Execution failure. Usage: system-udp <listen|broadcast> [intent] [target]");
        std::process::exit(1);
    }

    let mode = &args[1];

    match mode.as_str() {
        "listen" => listen_mesh(),
        "broadcast" => {
            if args.len() < 4 {
                eprintln!("[ERROR] Broadcast execution requires specific intent and target arguments.");
                std::process::exit(1);
            }
            let intent = &args[2];
            let target = &args[3];
            let sender_id = env::var("NODE_ID").unwrap_or_else(|_| "UNKNOWN_NODE".to_string());
            broadcast_intent(&sender_id, intent, target);
        },
        _ => {
            eprintln!("[ERROR] Unrecognized execution mode. Enforce 'listen' or 'broadcast'.");
            std::process::exit(1);
        }
    }
}

fn listen_mesh() {
    let bind_addr = format!("0.0.0.0:{}", MESH_PORT);
    let socket = UdpSocket::bind(&bind_addr).expect("[FATAL] Hardware rejection. Could not bind to mesh port.");
    
    println!("[SYSTEM] Zero-Broker Mesh active. Awaiting cryptographic payloads on {}", bind_addr);

    let mut buf = [0; 2048];

    loop {
        match socket.recv_from(&mut buf) {
            Ok((size, src)) => {
                // SECURITY PERIMETER: Drop all packets not originating from the WireGuard Subnet
                if !src.ip().to_string().starts_with("10.8.") && src.ip().to_string() != "127.0.0.1" {
                    println!("[WARNING] Dropped foreign packet from unauthorized IP: {}", src);
                    continue;
                }

                let raw_data = String::from_utf8_lossy(&buf[..size]);
                if let Ok(payload) = serde_json::from_str::<MeshPayload>(&raw_data) {
                    println!("[MESH] [{}] -> {}: {}", payload.sender_id, payload.target, payload.intent);
                    // FUTURE: Hand off extraction to system-slm here.
                } else {
                    println!("[WARNING] Structural failure. Malformed JSON packet from {}", src);
                }
            }
            Err(e) => eprintln!("[ERROR] Mesh read failure: {}", e),
        }
    }
}

fn broadcast_intent(sender: &str, intent: &str, target: &str) {
    let socket = UdpSocket::bind("0.0.0.0:0").expect("[FATAL] Hardware rejection. Could not open transmit socket.");
    socket.set_broadcast(true).expect("[FATAL] Kernel rejection. Could not enable broadcast physics.");

    let payload = MeshPayload {
        sender_id: sender.to_string(),
        intent: intent.to_string(),
        target: target.to_string(),
        timestamp: Utc::now().to_rfc3339(),
    };

    let data = serde_json::to_string(&payload).unwrap();
    let target_addr = format!("{}:{}", BROADCAST_ADDR, MESH_PORT);

    socket.send_to(data.as_bytes(), &target_addr).expect("[FATAL] Transport failure. Could not penetrate network.");
    println!("[SUCCESS] Intent broadcast to PPN Mesh ({})", target_addr);
}
