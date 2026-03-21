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

    #[allow(dead_code)]
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

    /// Start polling from an Arc reference (used when polling is deferred until after connection).
    pub fn start_polling_shared(self: &Arc<Self>, interval: Duration, app_state: Arc<Mutex<App>>) {
        let client = self.client.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;
                if !app_state.lock().await.paused {
                    Self::poll_once(&client, &app_state).await;
                }
            }
        });
    }

    async fn poll_once(client: &KaspaRpcClient, state: &Arc<Mutex<App>>) {
        let start = std::time::Instant::now();

        // Check if daemon is active and not yet synced — only poll server_info
        let is_daemon_syncing = state.lock().await.is_node_syncing();

        let server_info = client.get_server_info().await;

        let mut app = state.lock().await;
        let mut errors: Vec<String> = Vec::new();

        match server_info {
            Ok(v) => app.server_info = Some(v.into()),
            Err(e) => errors.push(format!("server_info: {}", e)),
        }

        // Release lock before making remaining RPC calls
        drop(app);

        if !is_daemon_syncing {
            let (dag_info, mempool, supply, fee_estimate) = tokio::join!(
                client.get_block_dag_info(),
                client.get_mempool_entries(true, false),
                client.get_coin_supply(),
                client.get_fee_estimate(),
            );

            let mut app = state.lock().await;

            match dag_info {
                Ok(v) => {
                    let info: crate::rpc::types::DagInfo = v.into();
                    app.dag_visualizer
                        .update(&info.tip_hashes, &info.virtual_parent_hashes);
                    app.dag_info = Some(info);
                }
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

            app.node_url = client.url();
            if let Some(desc) = client.node_descriptor() {
                app.node_uid = Some(desc.uid.clone());
            }

            let poll_duration_ms = start.elapsed().as_secs_f64() * 1000.0;
            app.last_refresh = Some(std::time::Instant::now());
            app.last_poll_duration_ms = Some(poll_duration_ms);
            app.last_error = if errors.is_empty() {
                None
            } else {
                Some(errors.join("; "))
            };
        } else {
            let mut app = state.lock().await;
            let poll_duration_ms = start.elapsed().as_secs_f64() * 1000.0;
            app.last_refresh = Some(std::time::Instant::now());
            app.last_poll_duration_ms = Some(poll_duration_ms);
            app.last_error = if errors.is_empty() {
                None
            } else {
                Some(errors.join("; "))
            };
        }
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
            "get_fee_estimate_experimental" => {
                let r = self.client.get_fee_estimate_experimental(true).await?;
                Ok(format!("{:#?}", r))
            }
            "get_connected_peer_info" => {
                let r = self.client.get_connected_peer_info().await?;
                Ok(format!("{:#?}", r))
            }
            "get_peer_addresses" => {
                let r = self.client.get_peer_addresses().await?;
                Ok(format!("{:#?}", r))
            }
            "get_current_network" => {
                let r = self.client.get_current_network().await?;
                Ok(format!("{:#?}", r))
            }
            "get_sink" => {
                let r = self.client.get_sink().await?;
                Ok(format!("{:#?}", r))
            }
            "get_sink_blue_score" => {
                let r = self.client.get_sink_blue_score().await?;
                Ok(format!("{:#?}", r))
            }
            "get_info" => {
                let r = self.client.get_info().await?;
                Ok(format!("{:#?}", r))
            }
            "get_block_count" => {
                let r = self.client.get_block_count().await?;
                Ok(format!("{:#?}", r))
            }
            "estimate_network_hashes_per_second" => {
                let dag = self.client.get_block_dag_info().await?;
                let r = self
                    .client
                    .estimate_network_hashes_per_second(1000, Some(dag.sink))
                    .await?;
                Ok(format!("Estimated network hash rate: {} hashes/second", r))
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
                let r = self
                    .client
                    .get_virtual_chain_from_block(dag.pruning_point_hash, false, None)
                    .await?;
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
            _ => Err(anyhow::anyhow!(
                "Unknown command: '{}'. Type 'help' for available commands.",
                method
            )),
        }
    }

    pub async fn fetch_mining_info(&self) -> Result<crate::rpc::types::MiningInfo> {
        use std::collections::HashMap;

        let dag = self.client.get_block_dag_info().await?;

        // Estimate hashrate
        let hashrate = self
            .client
            .estimate_network_hashes_per_second(1000, Some(dag.sink))
            .await
            .unwrap_or(0) as f64;

        // Get virtual chain to find recent blocks
        let vspc = self
            .client
            .get_virtual_chain_from_block(dag.pruning_point_hash, false, None)
            .await?;

        // Sample the last N blocks from the chain
        let sample_size = 100.min(vspc.added_chain_block_hashes.len());
        let start = vspc
            .added_chain_block_hashes
            .len()
            .saturating_sub(sample_size);
        let sample_hashes = &vspc.added_chain_block_hashes[start..];

        let mut miner_counts: HashMap<String, usize> = HashMap::new();

        for hash in sample_hashes {
            if let Ok(block) = self.client.get_block(*hash, true).await {
                // The first transaction in a block is the coinbase
                if let Some(coinbase) = block.transactions.first() {
                    // Miner address is typically in the first output
                    if let Some(output) = coinbase.outputs.first()
                        && let Some(ref verbose) = output.verbose_data
                    {
                        let addr = verbose.script_public_key_address.to_string();
                        let short_addr = crate::rpc::types::shorten_address(&addr, 10, 6);
                        *miner_counts.entry(short_addr).or_insert(0) += 1;
                    }
                }
            }
        }

        let unique_miners = miner_counts.len();
        let mut top_miners: Vec<(String, usize)> = miner_counts.into_iter().collect();
        top_miners.sort_by(|a, b| b.1.cmp(&a.1));
        top_miners.truncate(5);

        Ok(crate::rpc::types::MiningInfo {
            hashrate,
            unique_miners,
            top_miners,
            blocks_analyzed: sample_size,
        })
    }

    pub async fn fetch_analytics(&self) -> Result<crate::rpc::types::AnalyticsData> {
        use std::collections::HashMap;

        let dag = self.client.get_block_dag_info().await?;

        // Get virtual chain
        let vspc = self
            .client
            .get_virtual_chain_from_block(dag.pruning_point_hash, true, None)
            .await?;

        // Sample recent blocks
        let sample_size = 50.min(vspc.added_chain_block_hashes.len());
        let start = vspc
            .added_chain_block_hashes
            .len()
            .saturating_sub(sample_size);
        let sample_hashes = &vspc.added_chain_block_hashes[start..];

        let mut total_fees: u64 = 0;
        let mut fee_count: usize = 0;
        let mut min_fee = u64::MAX;
        let mut max_fee = 0u64;
        let mut receiver_counts: HashMap<String, usize> = HashMap::new();
        let mut total_transactions: usize = 0;

        for hash in sample_hashes {
            if let Ok(block) = self.client.get_block(*hash, true).await {
                for (i, tx) in block.transactions.iter().enumerate() {
                    if i == 0 {
                        continue;
                    } // skip coinbase
                    total_transactions += 1;

                    // Extract fee from mass via verbose data
                    if let Some(ref verbose) = tx.verbose_data {
                        let mass = verbose.compute_mass;
                        if mass > 0 {
                            total_fees += mass;
                            fee_count += 1;
                            min_fee = min_fee.min(mass);
                            max_fee = max_fee.max(mass);
                        }
                    }

                    // Track receiver addresses (from outputs)
                    for output in &tx.outputs {
                        if let Some(ref verbose) = output.verbose_data {
                            let addr = verbose.script_public_key_address.to_string();
                            let short = crate::rpc::types::shorten_address(&addr, 10, 6);
                            *receiver_counts.entry(short).or_insert(0) += 1;
                        }
                    }
                }
            }
        }

        let avg_fee = if fee_count > 0 {
            total_fees as f64 / fee_count as f64
        } else {
            0.0
        };
        if min_fee == u64::MAX {
            min_fee = 0;
        }

        let mut top_receivers: Vec<_> = receiver_counts
            .into_iter()
            .map(|(a, c)| crate::rpc::types::AddressActivity {
                address: a,
                tx_count: c,
            })
            .collect();
        top_receivers.sort_by(|a, b| b.tx_count.cmp(&a.tx_count));
        top_receivers.truncate(10);

        Ok(crate::rpc::types::AnalyticsData {
            fee_stats: crate::rpc::types::FeeStats {
                avg_fee_sompi: avg_fee,
                total_fees_sompi: total_fees,
                tx_count: fee_count,
                min_fee_sompi: min_fee,
                max_fee_sompi: max_fee,
            },
            top_senders: vec![], // Input verbose data doesn't include sender addresses in this RPC version
            top_receivers,
            blocks_analyzed: sample_size,
            total_transactions,
        })
    }

    pub async fn get_block_by_hash(&self, hash_str: &str) -> Result<String> {
        let hash = kaspa_rpc_core::RpcHash::from_str(hash_str)
            .map_err(|e| anyhow::anyhow!("Invalid hash: {}", e))?;
        let block = self.client.get_block(hash, true).await?;
        Ok(format!("{:#?}", block))
    }
}

impl Drop for RpcManager {
    fn drop(&mut self) {
        if let Some(handle) = self.poll_handle.take() {
            handle.abort();
        }
    }
}
