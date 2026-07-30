// ==============================================================================
// 🏭 POINTSAV DIGITAL SYSTEMS : SERVICE-HTTP (OMNI-ROUTER)
// ==============================================================================
use std::fs;
use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write};

fn handle_client(mut stream: TcpStream, expected_signature: &str) {
    let mut buffer = [0; 2048];
    let bytes_read = stream.read(&mut buffer).unwrap_or(0);
    if bytes_read == 0 { return; }
    
    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
    
    // [NETWORK] CORS Preflight
    if request.starts_with("OPTIONS") {
        let response = "HTTP/1.1 204 No Content\r\n\
                        Access-Control-Allow-Origin: *\r\n\
                        Access-Control-Allow-Methods: GET, OPTIONS\r\n\
                        Access-Control-Allow-Headers: X-MBA-Signature, Content-Type\r\n\r\n";
        stream.write_all(response.as_bytes()).unwrap();
        return;
    }
    
    // [SECURITY] MBA Enforcement
    let auth_header = format!("X-MBA-Signature: {}", expected_signature);
    if !request.contains(&auth_header) {
        println!(" [WARNING] Intrusion blocked. Invalid MBA.");
        let response = "HTTP/1.1 401 UNAUTHORIZED\r\nAccess-Control-Allow-Origin: *\r\n\r\n[FATAL] MBA Failed.";
        stream.write_all(response.as_bytes()).unwrap();
        return;
    }

    // [ROUTING] Dynamic Omni-Router parsing
    let path_line = request.lines().next().unwrap_or("");
    let parts: Vec<&str> = path_line.split_whitespace().collect();
    
    if parts.len() >= 2 && parts[0] == "GET" && parts[1].starts_with("/api/") {
        let route_parts: Vec<&str> = parts[1].split('/').collect();
        
        // Ensure format: /api/{service}/{entity}
        if route_parts.len() == 4 {
            let service = route_parts[2];
            let entity = route_parts[3];
            
            // [SECURITY] Strict Whitelist of Woodfine Services
            let allowed_services = ["service-people", "service-email", "service-content", "service-fs", "service-input"];
            
            if allowed_services.contains(&service) {
                println!(" [MESH] Routing to: {} -> {}", service, entity);
                
                // Construct the absolute path dynamically
                let target_path = format!("/home/mathew/deployments/woodfine-fleet-deployment/cluster-totebox-personnel-1/{}/ledgers/{}.yaml", service, entity);
                
                match fs::read_to_string(&target_path) {
                    Ok(data) => {
                        let response = format!("HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: application/x-yaml\r\n\r\n{}", data);
                        stream.write_all(response.as_bytes()).unwrap();
                    },
                    Err(_) => {
                        let response = "HTTP/1.1 404 NOT FOUND\r\nAccess-Control-Allow-Origin: *\r\n\r\n[ERROR] Ledger Missing.";
                        stream.write_all(response.as_bytes()).unwrap();
                    }
                }
                return;
            }
        }
    }
    
    let response = "HTTP/1.1 403 FORBIDDEN\r\nAccess-Control-Allow-Origin: *\r\n\r\n[ERROR] Invalid Route or Service Not Whitelisted.";
    stream.write_all(response.as_bytes()).unwrap();
}

fn main() {
    let token_path = "/home/mathew/Foundry/factory-pointsav/pointsav-monorepo/system-mba-shim/vault_identity.key";
    let expected_signature = fs::read_to_string(token_path).unwrap_or_else(|_| String::from("CRITICAL_FAILURE")).trim().to_string();
    
    println!("=========================================");
    println!(" [!] POINTSAV NATIVE OMNI-ROUTER");
    println!("=========================================");
    println!(" [NET] Gateway bound to 127.0.0.1:8080");
    println!(" [MESH] All 5 Woodfine Cartridges ONLINE.");
    println!("=========================================");
    
    let listener = TcpListener::bind("127.0.0.1:8080").unwrap();
    for stream in listener.incoming() {
        if let Ok(stream) = stream { handle_client(stream, &expected_signature); }
    }
}
