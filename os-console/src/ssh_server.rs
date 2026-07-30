use std::{
    io::Write,
    net::SocketAddr,
    sync::{mpsc, Arc},
    time::Duration,
};

use anyhow::Result;
use app_console_content::cartridge::ContentCartridge;
use app_console_input::InputCartridge;
use app_console_keys::{AppConsoleKeys, ConsoleConfig};
use russh::{
    server::{Auth, Config, Handler, Msg, Server, Session},
    Channel, ChannelId,
};
use system_gateway_mba::{
    auth::compute_fingerprint,
    db::{find_user, open_db},
    user::User,
};

// ---------------------------------------------------------------------------
// TerminalHandle — bridges ratatui's Write output to the SSH channel
// ---------------------------------------------------------------------------

struct TerminalHandle {
    sender: tokio::sync::mpsc::UnboundedSender<Vec<u8>>,
    sink: Vec<u8>,
}

impl TerminalHandle {
    fn new(handle: russh::server::Handle, channel_id: ChannelId) -> Self {
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
        tokio::spawn(async move {
            while let Some(data) = receiver.recv().await {
                if handle.data(channel_id, data).await.is_err() {
                    break;
                }
            }
        });
        Self {
            sender,
            sink: Vec::new(),
        }
    }
}

impl Write for TerminalHandle {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.sink.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let data = std::mem::take(&mut self.sink);
        self.sender
            .send(data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::BrokenPipe, e))
    }
}

// ---------------------------------------------------------------------------
// AppSession — handles one SSH client connection
// ---------------------------------------------------------------------------

struct AppSession {
    handle: Option<russh::server::Handle>,
    channel_id: Option<ChannelId>,
    term_cols: u32,
    term_rows: u32,
    input_tx: Option<mpsc::SyncSender<u8>>,
    user: Option<User>,
}

impl AppSession {
    fn new() -> Self {
        Self {
            handle: None,
            channel_id: None,
            term_cols: 220,
            term_rows: 50,
            input_tx: None,
            user: None,
        }
    }
}

impl Handler for AppSession {
    type Error = anyhow::Error;

    async fn auth_publickey(
        &mut self,
        _ssh_user: &str,
        key: &russh::keys::PublicKey,
    ) -> Result<Auth, Self::Error> {
        let fingerprint = compute_fingerprint(key);
        match open_db().and_then(|conn| find_user(&conn, &fingerprint)) {
            Ok(Some(user)) => {
                eprintln!(
                    "os-console: auth accepted for {}@{}",
                    user.username,
                    user.tenant.as_str()
                );
                self.user = Some(user);
                Ok(Auth::Accept)
            }
            Ok(None) => {
                eprintln!("os-console: auth rejected — fingerprint not registered: {fingerprint}");
                Ok(Auth::Reject {
                    proceed_with_methods: None,
                    partial_success: false,
                })
            }
            Err(e) => {
                eprintln!("os-console: auth db error: {e}");
                Ok(Auth::Reject {
                    proceed_with_methods: None,
                    partial_success: false,
                })
            }
        }
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        session: &mut Session,
    ) -> Result<bool, Self::Error> {
        self.handle = Some(session.handle());
        self.channel_id = Some(channel.id());
        Ok(true)
    }

    async fn pty_request(
        &mut self,
        channel: ChannelId,
        _term: &str,
        col_width: u32,
        row_height: u32,
        _pix_width: u32,
        _pix_height: u32,
        _modes: &[(russh::Pty, u32)],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.term_cols = col_width;
        self.term_rows = row_height;
        session.channel_success(channel)?;
        Ok(())
    }

    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        let cols = self.term_cols as u16;
        let rows = self.term_rows as u16;

        let user = match self.user.take() {
            Some(u) => u,
            None => {
                eprintln!("os-console: shell_request without authenticated user — rejecting");
                return Ok(());
            }
        };

        let handle = self
            .handle
            .take()
            .expect("handle set in channel_open_session");
        let ch_id = self.channel_id.unwrap_or(channel);

        let terminal_handle = TerminalHandle::new(handle, ch_id);
        let backend = ratatui::backend::CrosstermBackend::new(terminal_handle);
        let mut terminal = ratatui::Terminal::new(backend)?;
        terminal.resize(ratatui::layout::Rect {
            x: 0,
            y: 0,
            width: cols,
            height: rows,
        })?;

        let (input_tx, input_rx) = mpsc::sync_channel::<u8>(256);
        self.input_tx = Some(input_tx);

        session.channel_success(channel)?;

        let username = user.username.clone();
        let tenant = user.tenant.as_str().to_string();

        tokio::task::spawn_blocking(move || {
            let cfg = ConsoleConfig::load();
            let content = ContentCartridge::new_for(
                &username,
                &tenant,
                &cfg.profile.proof_endpoint,
                &cfg.profile.slm_endpoint,
                &cfg.profile.drafts_outbound_path,
                &cfg.profile.content_endpoint,
            );
            let input = InputCartridge::new_for(&username, &tenant, &cfg.profile.ingest_endpoint);
            let mut chassis = AppConsoleKeys::new(username, tenant);
            chassis.set_mba_active();
            chassis.register(Box::new(content));
            chassis.register(Box::new(input));
            chassis.run_with_bytes(terminal, input_rx);
        });

        Ok(())
    }

    async fn data(
        &mut self,
        _channel: ChannelId,
        data: &[u8],
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        if let Some(tx) = &self.input_tx {
            for &byte in data {
                let _ = tx.send(byte);
            }
        }
        Ok(())
    }

    async fn window_change_request(
        &mut self,
        _channel: ChannelId,
        col_width: u32,
        row_height: u32,
        _pix_width: u32,
        _pix_height: u32,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.term_cols = col_width;
        self.term_rows = row_height;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// AppServer — creates a new AppSession per incoming connection
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct AppServer;

impl Server for AppServer {
    type Handler = AppSession;
    fn new_client(&mut self, _peer_addr: Option<SocketAddr>) -> AppSession {
        AppSession::new()
    }
}

// ---------------------------------------------------------------------------
// run — called from main when ssh-server feature is enabled
// ---------------------------------------------------------------------------

pub async fn run() -> Result<()> {
    let key = russh::keys::PrivateKey::random(&mut rand::rng(), russh::keys::Algorithm::Ed25519)
        .expect("ed25519 key generation failed");

    let config = Arc::new(Config {
        inactivity_timeout: Some(Duration::from_secs(3600)),
        auth_rejection_time: Duration::from_secs(0),
        auth_rejection_time_initial: Some(Duration::from_secs(0)),
        keys: vec![key],
        ..Default::default()
    });

    let addr: SocketAddr = "0.0.0.0:2222".parse()?;
    eprintln!("os-console: listening on {addr}  (ssh -p 2222 proof@localhost)");

    let mut server = AppServer;
    server.run_on_address(config, addr).await?;
    Ok(())
}
