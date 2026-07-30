// ==============================================================================
// 🏭 POINTSAV DIGITAL SYSTEMS : SERVICE-HTTP (CORS & MBA GATEWAY)
// ==============================================================================
use std::fs;
use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write};

fn handle_client(mut stream: TcpStream, expected_signature: &str) {
    let mut buffer = [0; 2048];
    let bytes_read = stream.read(&mut buffer).unwrap_or(0);
    if bytes_read == 0 { return; }
    
    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
    
    // [NETWORK] Handle CORS Preflight for console.woodfinegroup.com
    if request.starts_with("OPTIONS") {
        let response = "HTTP/1.1 204 No Content\r\n\
                        Access-Control-Allow-Origin: *\r\n\
                        Access-Control-Allow-Methods: GET, OPTIONS\r\n\
                        Access-Control-Allow-Headers: X-MBA-Signature, Content-Type\r\n\r\n";
        stream.write_all(response.as_bytes()).unwrap();
        return;
    }
    
    // [SECURITY] Machine-Based Authorization (MBA) Checkpoint
    let auth_header = format!("X-MBA-Signature: {}", expected_signature);
    if !request.contains(&auth_header) {
        println!(" [WARNING] Intrusion blocked. Invalid MBA signature.");
        let response = "HTTP/1.1 401 UNAUTHORIZED\r\nAccess-Control-Allow-Origin: *\r\n\r\n[FATAL] MBA Failed.";
        stream.write_all(response.as_bytes()).unwrap();
        return;
    }

    // [ROUTING] Protected Execution Paths
    if request.starts_with("GET /api/people/") {
        let response_data = fs::read_to_string("/service-people/ledgers/peter-woodfine.yaml")
            .unwrap_or_else(|_| "404 - Ledger Not Found".to_string());
        let response = format!("HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: application/x-yaml\r\n\r\n{}", response_data);
        stream.write_all(response.as_bytes()).unwrap();
    } else {
        let response = "HTTP/1.1 403 FORBIDDEN\r\nAccess-Control-Allow-Origin: *\r\n\r\nPath Restricted.";
        stream.write_all(response.as_bytes()).unwrap();
    }
}

fn main() {
    let token_path = "/system-mba-shim/vault_identity.key";
    let expected_signature = fs::read_to_string(token_path).unwrap_or_else(|_| String::from("CRITICAL_FAILURE")).trim().to_string();
    println!(" [HTTP] Gateway bound to 10.0.2.15:8080 (CORS + MBA)");
    let listener = TcpListener::bind("0.0.0.0:8080").unwrap();
    for stream in listener.incoming() {
        if let Ok(stream) = stream { handle_client(stream, &expected_signature); }
    }
}
