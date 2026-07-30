use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::Instant,
};

/// Spawn a background thread serving Prometheus text exposition on `127.0.0.1:{port}`.
/// Binds only to loopback — prometheus scrapes locally; nginx proxies if needed.
/// Silently no-ops if the port is already in use.
pub fn spawn_metrics_server(port: u16) {
    thread::spawn(move || {
        let start = Instant::now();
        let listener = match TcpListener::bind(format!("127.0.0.1:{port}")) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("metrics: failed to bind 127.0.0.1:{port}: {e}");
                return;
            }
        };
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let uptime = start.elapsed().as_secs();
            let body = format!(
                "# HELP os_console_up 1 while os-console is running\n\
                 # TYPE os_console_up gauge\n\
                 os_console_up 1\n\
                 # HELP os_console_uptime_seconds Seconds elapsed since process start\n\
                 # TYPE os_console_uptime_seconds counter\n\
                 os_console_uptime_seconds {uptime}\n\
                 # HELP os_console_info Build metadata\n\
                 # TYPE os_console_info gauge\n\
                 os_console_info{{version=\"{version}\"}} 1\n",
                uptime = uptime,
                version = env!("CARGO_PKG_VERSION"),
            );
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain; version=0.0.4\r\n\
                 Content-Length: {len}\r\nConnection: close\r\n\r\n{body}",
                len = body.len(),
                body = body,
            );
            // Drain the HTTP request line before responding.
            let mut buf = [0u8; 512];
            let _ = stream.read(&mut buf);
            let _ = stream.write_all(response.as_bytes());
        }
    });
}
