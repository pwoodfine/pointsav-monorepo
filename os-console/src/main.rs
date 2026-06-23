#[cfg(feature = "ssh-server")]
mod ssh_server;

mod mba_client;
mod metrics;
mod tunnel;

fn main() -> anyhow::Result<()> {
    inner_main()
}

/// Returns true if the pairing server is reachable AND responding with HTTP.
/// Uses a raw TCP probe rather than reqwest so it doesn't depend on the tokio runtime.
/// A dead SSH child can hold port 9205 open while the underlying SSH connection is gone;
/// this probe detects that case (TCP connect succeeds, but no HTTP bytes come back).
fn pairing_server_alive() -> bool {
    use std::io::{Read, Write};
    let addr: std::net::SocketAddr = "127.0.0.1:9205".parse().unwrap();
    let Ok(mut stream) =
        std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(500))
    else {
        return false;
    };
    if stream
        .write_all(b"GET /v1/node-join/pending HTTP/1.0\r\nHost: 127.0.0.1\r\n\r\n")
        .is_err()
    {
        return false;
    }
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(3)))
        .ok();
    let mut buf = [0u8; 12];
    matches!(stream.read_exact(&mut buf), Ok(())) && buf.starts_with(b"HTTP/1.")
}

#[cfg(feature = "ssh-server")]
fn inner_main() -> anyhow::Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(ssh_server::run())
}

#[cfg(not(feature = "ssh-server"))]
fn inner_main() -> anyhow::Result<()> {
    use app_console_content::cartridge::ContentCartridge;
    use app_console_email::EmailCartridge;
    use app_console_input::InputCartridge;
    use app_console_keys::{pairing, AppConsoleKeys, ConsoleConfig};
    use app_console_people::PeopleCartridge;
    use app_console_slm::SlmCartridge;
    use app_console_system::SystemCartridge;

    let cfg = ConsoleConfig::load();
    let p = &cfg.profile;

    // Port list used by both tunnel paths.
    let tunnel_forwards: &[(u16, u16)] = &[
        (9080, 9080), // Doorman
        (9081, 9081), // service-content
        (9092, 9092), // service-proofreader (F4)
        (9100, 9100), // service-input (F12)
        (9093, 9093), // service-email (F3)
        (9205, 9205), // pairing-server (F11)
        (2222, 2222), // MBA SSH
    ];

    // Start embedded SSH tunnel if gce_host is configured
    let mut _ssh_child: Option<std::process::Child> = None;
    if !p.gce_host.is_empty() {
        // Check if the pairing server is actually reachable through an existing tunnel —
        // not just whether the port is bound. A stale SSH child from a previous run can
        // hold port 9205 open while the underlying SSH connection is dead, causing all
        // HTTP requests to fail silently.
        if !pairing_server_alive() {
            // Kill any stale SSH holding port 9205 before spawning fresh.
            let _ = std::process::Command::new("pkill")
                .args(["-f", "9205:localhost:9205"])
                .status();
            std::thread::sleep(std::time::Duration::from_millis(400));

            tunnel::spawn_tunnel(tunnel::TunnelConfig {
                gce_host: p.gce_host.clone(),
                gce_port: p.gce_ssh_port,
                username: p.gce_user.clone(),
                key_path: p.ssh_key_path.clone(),
                forwards: tunnel_forwards.to_vec(),
            });
            // Wait up to 5s for russh to bind ports.
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            loop {
                if std::net::TcpStream::connect("127.0.0.1:9205").is_ok() {
                    break;
                }
                if std::time::Instant::now() >= deadline {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
            if std::net::TcpStream::connect("127.0.0.1:9205").is_ok() {
                eprintln!("os-console: tunnel: russh bound port 9205");
            } else {
                eprintln!("os-console: tunnel: russh did not bind port 9205 within 5s");
            }
        } else {
            eprintln!("os-console: tunnel: existing tunnel alive — skipping spawn");
        }

        // Russh didn't bind ports — fall back to system ssh.
        if !pairing_server_alive() {
            let mut cmd = std::process::Command::new("ssh");
            cmd.arg("-N")
                .arg("-o")
                .arg("StrictHostKeyChecking=accept-new")
                .arg("-o")
                .arg("ServerAliveInterval=30")
                .arg("-o")
                .arg("ServerAliveCountMax=3")
                .arg("-p")
                .arg(p.gce_ssh_port.to_string())
                .arg("-i")
                .arg(&p.ssh_key_path);
            for &(local_port, remote_port) in tunnel_forwards {
                cmd.arg("-L")
                    .arg(format!("{local_port}:localhost:{remote_port}"));
            }
            cmd.arg(format!("{}@{}", p.gce_user, p.gce_host));
            cmd.stdout(std::process::Stdio::null());
            cmd.stderr(std::process::Stdio::null());
            match cmd.spawn() {
                Ok(child) => {
                    eprintln!("os-console: tunnel: system ssh spawned pid {}", child.id());
                    _ssh_child = Some(child);
                }
                Err(e) => eprintln!("os-console: tunnel: system ssh spawn failed: {e}"),
            }

            // Wait up to 15s for system ssh to bind ports.
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
            loop {
                if std::net::TcpStream::connect("127.0.0.1:9205").is_ok() {
                    break;
                }
                if std::time::Instant::now() >= deadline {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
            if std::net::TcpStream::connect("127.0.0.1:9205").is_ok() {
                eprintln!("os-console: tunnel: system ssh bound port 9205");
            } else {
                eprintln!("os-console: tunnel: system ssh did not bind port 9205 within 15s");
            }
        }
    }

    // Attempt MBA peer-to-peer link (5s timeout)
    let rt = tokio::runtime::Runtime::new()?;
    let mba = rt.block_on(async {
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            mba_client::connect_mba(
                &p.totebox_host,
                p.totebox_ssh_port,
                &p.username,
                &p.ssh_key_path,
            ),
        )
        .await
        .unwrap_or_else(|_| mba_client::MbaResult {
            active: false,
            fingerprint: "(connection timed out)".into(),
        })
    });
    drop(rt);

    let mut chassis = AppConsoleKeys::new(&p.username, &p.tenant);
    chassis.set_pair_base_url(p.pair_endpoint.clone());

    if mba.active {
        chassis.set_mba_active();
    } else {
        // MBA inactive — start zero-jargon pairing flow
        chassis.set_pairing_unpaired(mba.fingerprint.clone());

        let pub_key_line = load_pubkey_line(&p.ssh_key_path);
        let pair_rx = match pairing::post_pair_request(
            &p.pair_endpoint,
            &p.username,
            &p.tenant,
            &pub_key_line,
            &mba.fingerprint,
        ) {
            Ok((request_id, code)) => {
                chassis.set_pairing_awaiting(code, request_id.clone(), mba.fingerprint.clone());
                pairing::spawn_status_poll(p.pair_endpoint.clone(), request_id)
            }
            Err(_) => {
                // Tunnel not ready yet — retry in background; TUI shows "Connecting…"
                pairing::spawn_pair_init(
                    p.pair_endpoint.clone(),
                    p.username.clone(),
                    p.tenant.clone(),
                    pub_key_line,
                    mba.fingerprint.clone(),
                )
            }
        };
        chassis.set_pair_rx(pair_rx);

        // Reconnect watchdog — retries MBA link with exponential backoff.
        let (reconnect_tx, reconnect_rx) = std::sync::mpsc::channel::<bool>();
        let host = p.totebox_host.clone();
        let port = p.totebox_ssh_port;
        let username = p.username.clone();
        let key_path = p.ssh_key_path.clone();
        std::thread::spawn(move || {
            let mut delay = std::time::Duration::from_secs(2);
            let max_delay = std::time::Duration::from_secs(60);
            loop {
                std::thread::sleep(delay);
                let result = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map(|rt| {
                        rt.block_on(async {
                            tokio::time::timeout(
                                std::time::Duration::from_secs(5),
                                mba_client::connect_mba(&host, port, &username, &key_path),
                            )
                            .await
                            .unwrap_or_else(|_| {
                                mba_client::MbaResult {
                                    active: false,
                                    fingerprint: "(timeout)".into(),
                                }
                            })
                        })
                    });
                let active = result.map(|r| r.active).unwrap_or(false);
                if active {
                    let _ = reconnect_tx.send(true);
                    break;
                }
                delay = (delay * 2).min(max_delay);
            }
        });
        chassis.set_mba_reconnect_rx(reconnect_rx);
    }

    chassis.register(Box::new(PeopleCartridge::new_for(&p.people_endpoint)));
    chassis.register(Box::new(EmailCartridge::new_for(
        &p.email_endpoint,
        p.plain_mode,
    )));
    chassis.register(Box::new(ContentCartridge::new_for(
        &p.username,
        &p.tenant,
        &p.proof_endpoint,
        &p.slm_endpoint,
        &p.drafts_outbound_path,
        &p.content_endpoint,
        None,
        None,
        None,
    )));
    chassis.register(Box::new(InputCartridge::new_for(
        &p.username,
        &p.tenant,
        &p.ingest_endpoint,
    )));
    chassis.register(Box::new(SlmCartridge::new(&p.slm_endpoint, p.plain_mode)));
    chassis.register(Box::new(SystemCartridge::new(&p.pair_endpoint)));
    metrics::spawn_metrics_server(p.metrics_port);
    let _ = ctrlc::set_handler(|| {
        app_console_keys::request_shutdown();
    });
    let result = chassis.run_local();
    if let Some(mut child) = _ssh_child {
        let _ = child.kill();
    }
    result
}

/// Read the OpenSSH public key line from the .pub file alongside the private key.
fn load_pubkey_line(private_key_path: &str) -> String {
    let pub_path = format!("{}.pub", private_key_path);
    std::fs::read_to_string(&pub_path)
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}
