fn main() -> anyhow::Result<()> {
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:9201".to_string());
    system_gateway_mba::pairing_http::run_server(&addr)
}
