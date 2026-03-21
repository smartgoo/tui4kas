use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_wrpc_client::prelude::*;
use std::str::FromStr;
use tokio::sync::Mutex;

use crate::app::{App, ConnectionStatus};

pub struct RpcManager {
    client: Arc<KaspaRpcClient>,
    app_state: Arc<Mutex<App>>,
    poll_handle: Option<tokio::task::JoinHandle<()>>,
}

impl RpcManager {
    pub async fn new(
        url: Option<String>,
        network: &str,
        app_state: Arc<Mutex<App>>,
    ) -> Result<Self> {
        let network_id = NetworkId::from_str(network)?;

        let client = if let Some(ref url) = url {
            KaspaRpcClient::new(
                WrpcEncoding::Borsh,
                Some(url.as_str()),
                None,
                Some(network_id),
                None,
            )?
        } else {
            let resolver = Resolver::default();
            KaspaRpcClient::new(
                WrpcEncoding::Borsh,
                None,
                Some(resolver),
                Some(network_id),
                None,
            )?
        };

        Ok(Self {
            client: Arc::new(client),
            app_state,
            poll_handle: None,
        })
    }

    pub async fn connect(&self) -> Result<()> {
        {
            let mut app = self.app_state.lock().await;
            app.connection_status = ConnectionStatus::Connecting;
        }

        match self.client.connect(None).await {
            Ok(_) => {
                let mut app = self.app_state.lock().await;
                app.connection_status = ConnectionStatus::Connected;
                Ok(())
            }
            Err(e) => {
                let mut app = self.app_state.lock().await;
                app.connection_status = ConnectionStatus::Error(e.to_string());
                Err(e.into())
            }
        }
    }

    #[allow(dead_code)]
    pub async fn disconnect(&self) -> Result<()> {
        self.client.disconnect().await?;
        let mut app = self.app_state.lock().await;
        app.connection_status = ConnectionStatus::Disconnected;
        Ok(())
    }

    pub fn start_polling(&mut self, interval: Duration) {
        if let Some(handle) = self.poll_handle.take() {
            handle.abort();
        }

        let client = self.client.clone();
        let state = self.app_state.clone();

        self.poll_handle = Some(tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;
                if !state.lock().await.paused {
                    Self::poll_once(&client, &state).await;
                }
            }
        }));
    }

    async fn poll_once(client: &KaspaRpcClient, state: &Arc<Mutex<App>>) {
        let start = std::time::Instant::now();

        let (server_info, dag_info, mempool, supply, fee_estimate) = tokio::join!(
            client.get_server_info(),
            client.get_block_dag_info(),
            client.get_mempool_entries(true, false),
            client.get_coin_supply(),
            client.get_fee_estimate(),
        );

        let poll_duration_ms = start.elapsed().as_secs_f64() * 1000.0;

        let mut app = state.lock().await;
        let mut errors: Vec<String> = Vec::new();

        match server_info {
            Ok(v) => app.server_info = Some(v.into()),
            Err(e) => errors.push(format!("server_info: {}", e)),
        }
        match dag_info {
            Ok(v) => app.dag_info = Some(v.into()),
            Err(e) => errors.push(format!("dag_info: {}", e)),
        }
        match mempool {
            Ok(v) => app.mempool_state = Some(v.into()),
            Err(e) => errors.push(format!("mempool: {}", e)),
        }
        match supply {
            Ok(v) => app.coin_supply = Some(v.into()),
            Err(e) => errors.push(format!("coin_supply: {}", e)),
        }
        match fee_estimate {
            Ok(v) => app.fee_estimate = Some(v.into()),
            Err(e) => errors.push(format!("fee_estimate: {}", e)),
        }

        if app.node_url.is_none() {
            app.node_url = client.url();
        }
        if app.node_uid.is_none()
            && let Some(desc) = client.node_descriptor()
        {
            app.node_uid = Some(desc.uid.clone());
        }

        app.last_refresh = Some(std::time::Instant::now());
        app.last_poll_duration_ms = Some(poll_duration_ms);
        app.last_error = if errors.is_empty() {
            None
        } else {
            Some(errors.join("; "))
        };
    }

    pub async fn execute_rpc_call(&self, method: &str) -> Result<String> {
        match method {
            "get_server_info" => {
                let r = self.client.get_server_info().await?;
                Ok(format!("{:#?}", r))
            }
            "get_block_dag_info" => {
                let r = self.client.get_block_dag_info().await?;
                Ok(format!("{:#?}", r))
            }
            "get_mempool_entries" => {
                let r = self.client.get_mempool_entries(true, false).await?;
                Ok(format!("{:#?}", r))
            }
            "get_coin_supply" => {
                let r = self.client.get_coin_supply().await?;
                Ok(format!("{:#?}", r))
            }
            "get_fee_estimate" => {
                let r = self.client.get_fee_estimate().await?;
                Ok(format!("{:#?}", r))
            }
            "get_connected_peer_info" => {
                let r = self.client.get_connected_peer_info().await?;
                Ok(format!("{:#?}", r))
            }
            "get_block_count" => {
                let r = self.client.get_block_count().await?;
                Ok(format!("{:#?}", r))
            }
            "get_headers" => {
                let r = self.client.get_block_count().await?;
                Ok(format!("Header count: {}", r.header_count))
            }
            "get_sync_status" => {
                let r = self.client.get_server_info().await?;
                Ok(format!(
                    "Synced: {}\nVirtual DAA Score: {}\nServer Version: {}",
                    r.is_synced, r.virtual_daa_score, r.server_version
                ))
            }
            "get_virtual_chain" => {
                let dag = self.client.get_block_dag_info().await?;
                let r = self.client.get_virtual_chain_from_block(
                    dag.pruning_point_hash,
                    false,
                    None,
                ).await?;
                Ok(format!(
                    "Removed chain blocks: {}\nAdded chain blocks: {}\nAccepted transaction IDs: {}",
                    r.removed_chain_block_hashes.len(),
                    r.added_chain_block_hashes.len(),
                    r.accepted_transaction_ids.len(),
                ))
            }
            "ping" => {
                let start = std::time::Instant::now();
                self.client.ping().await?;
                let elapsed = start.elapsed();
                Ok(format!("Pong! ({:.2}ms)", elapsed.as_secs_f64() * 1000.0))
            }
            _ => Err(anyhow::anyhow!("Unknown command: '{}'. Type 'help' for available commands.", method)),
        }
    }

}

impl Drop for RpcManager {
    fn drop(&mut self) {
        if let Some(handle) = self.poll_handle.take() {
            handle.abort();
        }
    }
}
