use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use tokio::sync::RwLock;

pub use tui4kas_core::rpc::polling::PollingHandles;

use crate::app::App;
use crate::rpc::client::RpcManager;

pub fn start_mining_polling(
    rpc: &Arc<RpcManager>,
    app: &Arc<RwLock<App>>,
    handles: &mut PollingHandles,
) {
    let rpc_for_mining = rpc.clone();
    let app_for_mining = app.clone();
    handles.mining = Some(tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(5)).await;
        let mut ticker = tokio::time::interval(Duration::from_secs(30));
        loop {
            ticker.tick().await;
            let app_guard = app_for_mining.read().await;
            let is_synced = app_guard
                .node
                .server_info
                .as_ref()
                .is_some_and(|s| s.is_synced);
            let is_paused = app_guard.paused;
            let block_count = app_guard.mining_tab.block_count;
            drop(app_guard);
            if !is_paused
                && is_synced
                && let Ok(info) = rpc_for_mining.fetch_mining_info(block_count).await
            {
                let mut app = app_for_mining.write().await;
                app.node.mining_info = Some(info);
                app.dirty = true;
            }
        }
    }));
}

/// Create an RPC manager, connect, and start polling.
/// The core polling handle is stored in `handles` so it can be aborted on reconnect.
pub async fn create_and_start_rpc(
    url: Option<String>,
    network: &str,
    app: &Arc<RwLock<App>>,
    refresh_interval_ms: u64,
    retry: bool,
    handles: &mut PollingHandles,
) -> Result<Arc<RpcManager>> {
    let rpc_manager = RpcManager::new(url, network, app.clone())?;
    let rpc = Arc::new(rpc_manager);

    let rpc_for_connect = rpc.clone();
    let interval = refresh_interval_ms;
    let app_clone = app.clone();
    handles.core = Some(tokio::spawn(async move {
        let max_attempts = if retry { 30 } else { 1 };
        let mut connected = false;
        for attempt in 0..max_attempts {
            match rpc_for_connect.connect().await {
                Ok(_) => {
                    connected = true;
                    break;
                }
                Err(_) if attempt < max_attempts - 1 => {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                Err(_) => break,
            }
        }
        if connected {
            rpc_for_connect
                .run_polling_loop(Duration::from_millis(interval), app_clone)
                .await;
        }
    }));

    Ok(rpc)
}
