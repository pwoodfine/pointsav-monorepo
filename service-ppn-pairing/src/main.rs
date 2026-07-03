// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

mod db;
mod http;

fn main() {
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "0.0.0.0:9202".to_string());
    if let Err(e) = http::run_server(&addr) {
        eprintln!("ppn-pairing: fatal: {e}");
        std::process::exit(1);
    }
}
