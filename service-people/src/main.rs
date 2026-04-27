// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::env;
use std::sync::Arc;
use tracing::info;
use service_people::{FsClient, AppState, PeopleStore};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let module_id = env::var("PEOPLE_MODULE_ID")
        .expect("PEOPLE_MODULE_ID environment variable must be set");

    let fs_url = env::var("PEOPLE_FS_URL")
        .expect("PEOPLE_FS_URL environment variable must be set");

    let bind_addr = env::var("PEOPLE_BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:9300".to_string());

    info!("Starting service-people on {}", bind_addr);
    info!("Module ID: {}", module_id);
    info!("FS URL: {}", fs_url);

    let fs_client = FsClient::new(fs_url, module_id.clone());
    let people_store = Arc::new(PeopleStore::new());

    let app_state = AppState {
        module_id: module_id.clone(),
        fs_client,
        people_store,
    };

    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    info!("service-people listening on {}", bind_addr);

    let router = service_people::router(app_state);
    axum::serve(listener, router).await?;

    Ok(())
}
