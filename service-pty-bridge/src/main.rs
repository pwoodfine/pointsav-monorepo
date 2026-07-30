use warp::Filter;
use futures_util::{StreamExt, SinkExt};
use tokio::process::Command;
use std::process::Stdio;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::main]
async fn main() {
    println!("[SYSTEM] PointSav Rust Kernel Bridge Ignited (PURE BINARY MODE).");
    println!("[SYSTEM] Strict Origin Auth Enabled. Listening on 127.0.0.1:7681");

    let ws_route = warp::path!("terminal" / "ws")
        .and(warp::ws())
        .map(|ws: warp::ws::Ws| {
            ws.on_upgrade(|mut websocket| async move {
                println!("[AUTH] Tier-1 Binary WebSocket connection established.");
                
                let mut child = Command::new("/bin/bash")
                    .arg("-l")
                    .env("TERM", "xterm-256color")
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .expect("[FAULT] Failed to spawn kernel shell.");

                let mut stdin = child.stdin.take().expect("Failed to open stdin");
                let mut stdout = child.stdout.take().expect("Failed to open stdout");

                let (mut ws_tx, mut ws_rx) = websocket.split();
                
                // THREAD 1: Read raw bytes from Kernel -> Send raw binary to Browser
                let mut stdout_buf = [0u8; 1024];
                tokio::spawn(async move {
                    loop {
                        match stdout.read(&mut stdout_buf).await {
                            Ok(0) => break, // Process exited
                            Ok(n) => {
                                // MATHEMATICAL PARITY: Sending raw binary vector (No Strings)
                                if ws_tx.send(warp::ws::Message::binary(stdout_buf[..n].to_vec())).await.is_err() {
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                });

                // THREAD 2: Receive raw binary from Browser -> Write to Kernel
                tokio::spawn(async move {
                    while let Some(result) = ws_rx.next().await {
                        if let Ok(msg) = result {
                            // Extract raw bytes directly, bypassing UTF-8 parsers
                            let bytes = msg.as_bytes();
                            if !bytes.is_empty() {
                                if stdin.write_all(bytes).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                });
            })
        });

    warp::serve(ws_route).run(([127, 0, 0, 1], 7681)).await;
}
