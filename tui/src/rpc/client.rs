use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use kaspa_rpc_core::api::rpc::RpcApi;
use tokio::sync::RwLock;

pub use tui4kas_core::rpc::client::{RpcClient, extract_block_summaries};

use crate::app::{App, ConnectionStatus};
use crate::rpc::types::RpcMethod;

pub struct RpcManager {
    pub rpc: Arc<RpcClient>,
    app_state: Arc<RwLock<App>>,
    poll_handle: Option<tokio::task::JoinHandle<()>>,
}

impl RpcManager {
    pub fn new(url: Option<String>, network: &str, app_state: Arc<RwLock<App>>) -> Result<Self> {
        let rpc = RpcClient::new(url, network)?;
        Ok(Self {
            rpc: Arc::new(rpc),
            app_state,
            poll_handle: None,
        })
    }

    pub async fn connect(&self) -> Result<()> {
        {
            let mut app = self.app_state.write().await;
            app.node.connection_status = ConnectionStatus::Connecting;
        }

        match self.rpc.connect().await {
            Ok(_) => {
                let mut app = self.app_state.write().await;
                app.node.connection_status = ConnectionStatus::Connected;
                Ok(())
            }
            Err(e) => {
                let mut app = self.app_state.write().await;
                app.node.connection_status = ConnectionStatus::Error(e.to_string());
                Err(e)
            }
        }
    }

    pub async fn disconnect(&self) -> Result<()> {
        self.rpc.disconnect().await?;
        let mut app = self.app_state.write().await;
        app.node.connection_status = ConnectionStatus::Disconnected;
        Ok(())
    }

    /// Run the polling loop inline (caller is responsible for spawning).
    pub async fn run_polling_loop(
        self: &Arc<Self>,
        interval: Duration,
        app_state: Arc<RwLock<App>>,
    ) {
        let client = self.rpc.inner().clone();
        let mut ticker = tokio::time::interval(interval);
        loop {
            ticker.tick().await;
            if !app_state.read().await.paused {
                Self::poll_once(&client, &app_state).await;
            }
        }
    }

    async fn poll_once(
        client: &kaspa_wrpc_client::prelude::KaspaRpcClient,
        state: &Arc<RwLock<App>>,
    ) {
        let start = std::time::Instant::now();
        let mut errors: Vec<String> = Vec::new();

        let (server_info, dag_info, mempool, supply, fee_estimate, sink_blue_score) = tokio::join!(
            client.get_server_info(),
            client.get_block_dag_info(),
            client.get_mempool_entries(true, false),
            client.get_coin_supply(),
            client.get_fee_estimate(),
            client.get_sink_blue_score(),
        );

        let mut app = state.write().await;

        match server_info {
            Ok(v) => app.node.server_info = Some(v.into()),
            Err(e) => errors.push(format!("server_info: {}", e)),
        }
        match dag_info {
            Ok(v) => {
                let info: crate::rpc::types::DagInfo = v.into();
                app.node.dag_info = Some(info);
            }
            Err(e) => errors.push(format!("dag_info: {}", e)),
        }
        match mempool {
            Ok(v) => app.node.mempool_state = Some(v.into()),
            Err(e) => errors.push(format!("mempool: {}", e)),
        }
        match supply {
            Ok(v) => app.node.coin_supply = Some(v.into()),
            Err(e) => errors.push(format!("coin_supply: {}", e)),
        }
        match fee_estimate {
            Ok(v) => app.node.fee_estimate = Some(v.into()),
            Err(e) => errors.push(format!("fee_estimate: {}", e)),
        }
        match sink_blue_score {
            Ok(v) => app.node.sink_blue_score = Some(v),
            Err(e) => errors.push(format!("sink_blue_score: {}", e)),
        }

        if app.node.dag_info.is_some() {
            let blue = app.node.sink_blue_score;
            app.node.dag_stats.update(blue);
        }

        app.node.node_url = client.url();
        if let Some(desc) = client.node_descriptor() {
            app.node.node_uid = Some(desc.uid.clone());
        }

        let poll_duration_ms = start.elapsed().as_secs_f64() * 1000.0;
        app.node.last_poll_duration_ms = Some(poll_duration_ms);
        app.dirty = true;
    }

    /// Delegate to core's execute_rpc_call
    pub async fn execute_rpc_call(&self, method: RpcMethod) -> Result<String> {
        self.rpc.execute_rpc_call(method).await
    }

    /// Delegate to core's get_mempool_entry
    pub async fn get_mempool_entry(&self, tx_id_str: &str) -> Result<String> {
        self.rpc.get_mempool_entry(tx_id_str).await
    }

    /// Delegate to core's fetch_mining_info
    pub async fn fetch_mining_info(
        &self,
        block_count: usize,
    ) -> Result<crate::rpc::types::MiningInfo> {
        self.rpc.fetch_mining_info(block_count).await
    }

    /// Delegate to core's fetch_vspc_v2
    pub async fn fetch_vspc_v2(
        &self,
        start_hash: kaspa_rpc_core::RpcHash,
    ) -> Result<kaspa_rpc_core::GetVirtualChainFromBlockV2Response> {
        self.rpc.fetch_vspc_v2(start_hash).await
    }

    /// Delegate to core
    pub async fn get_pruning_point_hash(&self) -> Result<kaspa_rpc_core::RpcHash> {
        self.rpc.get_pruning_point_hash().await
    }

    /// Delegate to core
    pub async fn get_sink_hash(&self) -> Result<kaspa_rpc_core::RpcHash> {
        self.rpc.get_sink_hash().await
    }

    /// Delegate to core
    pub async fn get_daa_score(&self) -> Result<u64> {
        self.rpc.get_daa_score().await
    }
}

impl Drop for RpcManager {
    fn drop(&mut self) {
        if let Some(handle) = self.poll_handle.take() {
            handle.abort();
        }
    }
}
