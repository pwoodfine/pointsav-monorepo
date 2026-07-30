use warp::Filter;
use serde::Deserialize;
use std::fs::OpenOptions;
use std::io::Write;
use std::env;
use std::net::SocketAddr;

#[derive(Deserialize)]
struct TelemetryPayload {
    uri: String,
    timestamp: String,
    user_agent: String,
}

#[tokio::main]
async fn main() {
    let port_str = env::var("PORT").unwrap_or_else(|_| "8081".to_string());
    let port: u16 = port_str.parse().expect("PORT must be a valid u16");
    
    // MATHEMATICAL LOCK: Bind to 0.0.0.0 (All Interfaces) to accept Wireguard traffic
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    let telemetry_route = warp::post()
        .and(warp::path("telemetry-endpoint"))
        .and(warp::header::optional::<String>("x-forwarded-for"))
        .and(warp::body::json())
        .map(|ip_header: Option<String>, payload: TelemetryPayload| {
            let mut ip = ip_header.unwrap_or_else(|| "0.0.0.0".to_string());
            if let Some(first_ip) = ip.split(',').next() {
                ip = first_ip.trim().to_string();
            }

            // Write to the verified shallow assets path
            if let Ok(mut file) = OpenOptions::new()
                .create(true)
                .append(true)
                .open("assets/ledger_telemetry.csv")
            {
                let csv_line = format!("\"{}\",\"{}\",\"{}\",\"{}\"\n", 
                    ip, payload.timestamp, payload.uri, payload.user_agent);
                let _ = file.write_all(csv_line.as_bytes());
            }

            warp::reply::json(&"Accepted")
        });

    let cors = warp::cors()
        .allow_any_origin()
        .allow_methods(vec!["POST"])
        .allow_headers(vec!["Content-Type"]);

    let routes = telemetry_route.with(cors);

    println!("[SYSTEM] Telemetry Daemon active and listening on {}", addr);
    warp::serve(routes).run(addr).await;
}
