use std::net::SocketAddr;
use warp::Filter;

mod routes_content;
mod routes_email;
mod routes_people;

/// PointSav Digital Systems
/// service-http: The Sovereign API Gateway (Tier-5)

#[tokio::main]
async fn main() {
    println!("========================================================");
    println!(" 🏛️ SERVICE-HTTP: TOTEBOX API GATEWAY");
    println!("========================================================");
    println!("[SYSTEM] Bootstrapping internal routing matrix...");

    // 1. Content Surveyor Routes (service-content)
    let content_api = routes_content::api_filters();

    // 2. Email Administration Routes (service-email)
    let email_api = routes_email::api_filters();

    // 3. Personnel Ledger Routes (service-people)
    let people_api = routes_people::api_filters();

    // Combine all API routes under /api/v1/
    let api_routes = warp::path("api")
        .and(warp::path("v1"))
        .and(content_api.or(email_api).or(people_api));

    // CORS Injection: Authorize the local HTML dashboard to query the API
    let cors = warp::cors()
        .allow_any_origin()
        .allow_methods(vec!["GET", "POST", "DELETE"])
        .allow_headers(vec!["Content-Type"]);

    let routes_with_cors = api_routes.with(cors);

    // Bind to 0.0.0.0 to allow os-console (via PSST tunnel) to connect
    let addr: SocketAddr = "0.0.0.0:8080".parse().unwrap();
    println!("[SUCCESS] service-http mathematically locked to port 8080 within the Totebox boundary.");
    
    warp::serve(routes_with_cors).run(addr).await;
}
