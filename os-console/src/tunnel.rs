// Embedded SSH port-forward tunnel.
//
// Spawns a background tokio runtime that opens an SSH connection to the GCE VM
// and forwards the ports os-console needs. The binary manages its own tunnel;
// no external `ssh -N` command or second terminal required.
//
// Called from main() when `gce_host` is set in config.toml. Non-fatal — if the
// tunnel can't connect, os-console continues and service errors appear in the UI.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use russh::{
    client,
    keys::{load_secret_key, PrivateKeyWithHashAlg},
};
use tokio::io::copy_bidirectional;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio::task::JoinSet;

pub struct TunnelConfig {
    pub gce_host: String,
    pub gce_port: u16,
    pub username: String,
    pub key_path: String,
    /// (local_port, remote_port) pairs to forward
    pub forwards: Vec<(u16, u16)>,
}

/// Spawn the SSH tunnel in a background thread. Returns immediately.
/// Reconnects automatically with exponential backoff on disconnection.
pub fn spawn_tunnel(config: TunnelConfig) {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(2)
            .thread_name("os-console-tunnel")
            .build()
            .expect("tunnel runtime");
        rt.block_on(tunnel_loop(config));
    });
}

async fn tunnel_loop(config: TunnelConfig) {
    let mut delay = Duration::from_secs(2);
    let max_delay = Duration::from_secs(60);
    loop {
        run_tunnel(&config).await;
        eprintln!(
            "os-console: tunnel: disconnected; retrying in {:.0}s",
            delay.as_secs_f32()
        );
        tokio::time::sleep(delay).await;
        delay = (delay * 2).min(max_delay);
    }
}

async fn run_tunnel(config: &TunnelConfig) {
    let key = match load_secret_key(&config.key_path, None) {
        Ok(k) => k,
        Err(e) => {
            eprintln!(
                "os-console: tunnel: key load failed ({}): {e}",
                config.key_path
            );
            tokio::time::sleep(Duration::from_secs(5)).await;
            return;
        }
    };

    let ssh_config = Arc::new(client::Config::default());
    let mut handle = match client::connect(
        ssh_config,
        (config.gce_host.as_str(), config.gce_port),
        TunnelHandler,
    )
    .await
    {
        Ok(h) => h,
        Err(e) => {
            eprintln!(
                "os-console: tunnel: connect to {}:{} failed: {e}",
                config.gce_host, config.gce_port
            );
            return;
        }
    };

    let key_with_alg = PrivateKeyWithHashAlg::new(Arc::new(key), None);
    match handle
        .authenticate_publickey(&config.username, key_with_alg)
        .await
    {
        Ok(r) if r.success() => {}
        Ok(_) => {
            eprintln!(
                "os-console: tunnel: SSH auth rejected for {}",
                config.username
            );
            return;
        }
        Err(e) => {
            eprintln!("os-console: tunnel: SSH auth failed: {e}");
            return;
        }
    }

    eprintln!(
        "os-console: tunnel: up — {}@{}",
        config.username, config.gce_host
    );

    let handle = Arc::new(Mutex::new(handle));
    let mut join_set = JoinSet::new();

    for &(local_port, remote_port) in &config.forwards {
        let addr = SocketAddr::from(([127, 0, 0, 1], local_port));
        let listener = match TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(e) => {
                // Port already in use is non-fatal — skip it
                eprintln!("os-console: tunnel: bind :{local_port} failed: {e}");
                continue;
            }
        };
        let handle = handle.clone();
        join_set.spawn(async move {
            accept_loop(listener, handle, remote_port).await;
        });
    }

    if join_set.is_empty() {
        eprintln!("os-console: tunnel: no ports bound — nothing to forward");
        return;
    }

    // Wait until first accept loop exits (SSH connection dropped → channels fail)
    let _ = join_set.join_next().await;
}

async fn accept_loop(
    listener: TcpListener,
    handle: Arc<Mutex<client::Handle<TunnelHandler>>>,
    remote_port: u16,
) {
    loop {
        let (mut tcp_stream, _) = match listener.accept().await {
            Ok(c) => c,
            Err(_) => break,
        };
        let handle = handle.clone();
        tokio::spawn(async move {
            let channel = {
                let h = handle.lock().await;
                h.channel_open_direct_tcpip("localhost", remote_port as u32, "127.0.0.1", 0)
                    .await
            };
            match channel {
                Ok(ch) => {
                    let mut ch_stream = ch.into_stream();
                    let _ = copy_bidirectional(&mut tcp_stream, &mut ch_stream).await;
                }
                Err(e) => {
                    eprintln!("os-console: tunnel: channel :{remote_port} failed: {e}");
                }
            }
        });
    }
}

pub struct TunnelHandler;

impl client::Handler for TunnelHandler {
    type Error = anyhow::Error;

    async fn check_server_key(
        &mut self,
        _key: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}
