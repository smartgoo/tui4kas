use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use futures::stream::{self, StreamExt};
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_rpc_core::{GetVirtualChainFromBlockV2Response, RpcDataVerbosityLevel, RpcHash};
use kaspa_wrpc_client::prelude::*;
use std::str::FromStr;
use tokio::sync::RwLock;

use crate::analytics::{BlockSummary, detect_protocol};
use crate::app::{App, ConnectionStatus};

pub struct RpcManager {
    client: Arc<KaspaRpcClient>,
    app_state: Arc<RwLock<App>>,
    poll_handle: Option<tokio::task::JoinHandle<()>>,
}

impl RpcManager {
    pub async fn new(
        url: Option<String>,
        network: &str,
        app_state: Arc<RwLock<App>>,
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
            let mut app = self.app_state.write().await;
            app.node.connection_status = ConnectionStatus::Connecting;
        }

        match self.client.connect(None).await {
            Ok(_) => {
                let mut app = self.app_state.write().await;
                app.node.connection_status = ConnectionStatus::Connected;
                Ok(())
            }
            Err(e) => {
                let mut app = self.app_state.write().await;
                app.node.connection_status = ConnectionStatus::Error(e.to_string());
                Err(e.into())
            }
        }
    }

    pub async fn disconnect(&self) -> Result<()> {
        self.client.disconnect().await?;
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
        let client = self.client.clone();
        let mut ticker = tokio::time::interval(interval);
        loop {
            ticker.tick().await;
            if !app_state.read().await.paused {
                Self::poll_once(&client, &app_state).await;
            }
        }
    }

    async fn poll_once(client: &KaspaRpcClient, state: &Arc<RwLock<App>>) {
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
                Ok(format!(
                    "estimate_network_hashes_per_second(\n  window_size: 1000,\n  start_hash: {:?}\n)\n\n{:#?}",
                    dag.sink, r
                ))
            }
            "get_headers" => {
                let r = self.client.get_block_count().await?;
                Ok(format!("{:#?}", r))
            }
            "get_sync_status" => {
                let r = self.client.get_server_info().await?;
                Ok(format!("{:#?}", r))
            }
            "get_virtual_chain" => {
                let dag = self.client.get_block_dag_info().await?;
                let r = self
                    .client
                    .get_virtual_chain_from_block(dag.pruning_point_hash, false, None)
                    .await?;
                Ok(format!("{:#?}", r))
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

    pub async fn fetch_mining_info(
        &self,
        block_count: usize,
    ) -> Result<crate::rpc::types::MiningInfo> {
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
        let sample_size = block_count.min(vspc.added_chain_block_hashes.len());
        let start = vspc
            .added_chain_block_hashes
            .len()
            .saturating_sub(sample_size);
        let sample_hashes = &vspc.added_chain_block_hashes[start..];

        let mut miner_counts: HashMap<String, usize> = HashMap::new();
        let mut pool_counts: HashMap<String, usize> = HashMap::new();
        let mut version_counts: HashMap<String, usize> = HashMap::new();

        // Fetch blocks in parallel (10 concurrent)
        let hashes: Vec<_> = sample_hashes.to_vec();
        let client = self.client.clone();
        let results: Vec<_> = stream::iter(hashes)
            .map(|hash| {
                let client = client.clone();
                async move { client.get_block(hash, true).await }
            })
            .buffer_unordered(10)
            .collect()
            .await;

        for block in results.into_iter().flatten() {
            // The first transaction in a block is the coinbase
            if let Some(coinbase) = block.transactions.first() {
                // Miner address is typically in the first output
                if let Some(output) = coinbase.outputs.first()
                    && let Some(ref verbose) = output.verbose_data
                {
                    let addr = verbose.script_public_key_address.to_string();
                    *miner_counts.entry(addr).or_insert(0) += 1;
                }

                // Parse coinbase payload for pool and version info
                let payload = coinbase.payload.as_slice();
                let cb_info = crate::rpc::types::parse_coinbase_payload(payload);
                if let Some(pool) = cb_info.pool_name {
                    *pool_counts.entry(pool).or_insert(0) += 1;
                }
                if let Some(version) = cb_info.node_version {
                    *version_counts.entry(version).or_insert(0) += 1;
                }
            }
        }

        let unique_miners = miner_counts.len();
        let mut all_miners: Vec<(String, usize)> = miner_counts.into_iter().collect();
        all_miners.sort_by_key(|b| std::cmp::Reverse(b.1));

        let mut pools: Vec<(String, usize)> = pool_counts.into_iter().collect();
        pools.sort_by_key(|b| std::cmp::Reverse(b.1));

        let mut node_versions: Vec<(String, usize)> = version_counts.into_iter().collect();
        node_versions.sort_by_key(|b| std::cmp::Reverse(b.1));

        Ok(crate::rpc::types::MiningInfo {
            hashrate,
            unique_miners,
            all_miners,
            blocks_analyzed: sample_size,
            pools,
            node_versions,
        })
    }

    /// Fetch the virtual selected parent chain v2 from a given start hash.
    /// Uses High verbosity for fee/address/payload data, and min_confirmation_count=10.
    pub async fn fetch_vspc_v2(
        &self,
        start_hash: RpcHash,
    ) -> Result<GetVirtualChainFromBlockV2Response> {
        let response = self
            .client
            .get_virtual_chain_from_block_v2(
                start_hash,
                Some(RpcDataVerbosityLevel::High),
                Some(10),
            )
            .await?;
        Ok(response)
    }

    /// Get the pruning point hash from block DAG info.
    pub async fn get_pruning_point_hash(&self) -> Result<RpcHash> {
        let dag = self.client.get_block_dag_info().await?;
        Ok(dag.pruning_point_hash)
    }

    /// Get the sink (tip) hash from block DAG info.
    pub async fn get_sink_hash(&self) -> Result<RpcHash> {
        let dag = self.client.get_block_dag_info().await?;
        Ok(dag.sink)
    }

    /// Get the current DAA score from DAG info.
    pub async fn get_daa_score(&self) -> Result<u64> {
        let dag = self.client.get_block_dag_info().await?;
        Ok(dag.virtual_daa_score)
    }

    /// Extract BlockSummary entries and removed hashes from a VSPC V2 response.
    /// Deduplicates transactions across blocks and excludes coinbase transactions.
    pub fn extract_block_summaries(
        response: &GetVirtualChainFromBlockV2Response,
    ) -> (Vec<BlockSummary>, Vec<String>) {
        let removed: Vec<String> = response
            .removed_chain_block_hashes
            .iter()
            .map(|h| h.to_string())
            .collect();

        let mut summaries = Vec::new();

        // Track seen transaction IDs across all blocks for deduplication
        let mut seen_tx_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

        for chain_block in response.chain_block_accepted_transactions.iter() {
            let header = &chain_block.chain_block_header;
            let hash = header.hash.map(|h| h.to_string()).unwrap_or_default();
            let timestamp_ms = header.timestamp.unwrap_or(0);
            let daa_score = header.daa_score.unwrap_or(0);
            let _ = daa_score; // available for sync progress tracking

            let mut tx_count: usize = 0;
            let mut total_mass: u64 = 0;
            let mut mass_count: usize = 0;
            let mut total_fees: u64 = 0;
            let mut fee_count: usize = 0;
            let mut sender_counts: HashMap<String, usize> = HashMap::new();
            let mut receiver_counts: HashMap<String, usize> = HashMap::new();
            let mut protocol_counts: HashMap<crate::analytics::TransactionProtocol, usize> =
                HashMap::new();
            let mut protocol_mass: HashMap<crate::analytics::TransactionProtocol, u64> =
                HashMap::new();
            let mut protocol_fees: HashMap<crate::analytics::TransactionProtocol, u64> =
                HashMap::new();

            for tx in chain_block.accepted_transactions.iter() {
                // Exclude coinbase transactions via subnetwork_id
                if tx.subnetwork_id.as_ref().is_some_and(|id| {
                    id == &kaspa_rpc_core::RpcSubnetworkId::from_bytes([
                        1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    ])
                }) {
                    continue;
                }

                // Deduplicate: skip if we've already processed this transaction
                let tx_id = tx
                    .verbose_data
                    .as_ref()
                    .and_then(|vd| vd.transaction_id)
                    .map(|id| id.to_string())
                    .unwrap_or_default();
                if !tx_id.is_empty() && !seen_tx_ids.insert(tx_id) {
                    continue;
                }

                tx_count += 1;

                // Extract compute_mass (High verbosity)
                let tx_mass = if let Some(ref verbose) = tx.verbose_data
                    && let Some(mass) = verbose.compute_mass
                    && mass > 0
                {
                    total_mass += mass;
                    mass_count += 1;
                    mass
                } else {
                    0
                };

                // Calculate fee: sum(input amounts) - sum(output amounts)
                let input_sum: u64 = tx
                    .inputs
                    .iter()
                    .filter_map(|inp| {
                        inp.verbose_data
                            .as_ref()
                            .and_then(|vd| vd.utxo_entry.as_ref())
                            .map(|utxo| utxo.amount.unwrap_or(0))
                    })
                    .sum();
                let output_sum: u64 = tx.outputs.iter().map(|o| o.value.unwrap_or(0)).sum();
                let tx_fee = input_sum.saturating_sub(output_sum);
                if input_sum > 0 {
                    total_fees += tx_fee;
                    fee_count += 1;
                }

                // Track sender addresses per-transaction (not per-UTXO)
                let mut tx_senders: std::collections::HashSet<String> =
                    std::collections::HashSet::new();
                for input in &tx.inputs {
                    if let Some(ref vd) = input.verbose_data
                        && let Some(ref utxo) = vd.utxo_entry
                        && let Some(ref uvd) = utxo.verbose_data
                        && let Some(ref addr) = uvd.script_public_key_address
                    {
                        tx_senders.insert(addr.to_string());
                    }
                }
                for addr in tx_senders {
                    *sender_counts.entry(addr).or_insert(0) += 1;
                }

                // Track receiver addresses per-transaction (not per-UTXO)
                let mut tx_receivers: std::collections::HashSet<String> =
                    std::collections::HashSet::new();
                for output in &tx.outputs {
                    if let Some(ref vd) = output.verbose_data
                        && let Some(ref addr) = vd.script_public_key_address
                    {
                        tx_receivers.insert(addr.to_string());
                    }
                }
                for addr in tx_receivers {
                    *receiver_counts.entry(addr).or_insert(0) += 1;
                }

                // Protocol detection from payload and input scripts
                let payload = tx.payload.as_deref().unwrap_or(&[]);
                let input_scripts: Vec<&[u8]> = tx
                    .inputs
                    .iter()
                    .filter_map(|inp| inp.signature_script.as_deref())
                    .collect();
                if let Some(proto) = detect_protocol(payload, &input_scripts) {
                    *protocol_counts.entry(proto).or_insert(0) += 1;
                    if tx_mass > 0 {
                        *protocol_mass.entry(proto).or_insert(0) += tx_mass;
                    }
                    if input_sum > 0 {
                        *protocol_fees.entry(proto).or_insert(0) += tx_fee;
                    }
                }
            }

            summaries.push(BlockSummary {
                hash,
                timestamp_ms,
                tx_count,
                total_mass,
                mass_count,
                total_fees,
                fee_count,
                sender_counts,
                receiver_counts,
                protocol_counts,
                protocol_mass,
                protocol_fees,
            });
        }

        (summaries, removed)
    }
}

impl Drop for RpcManager {
    fn drop(&mut self) {
        if let Some(handle) = self.poll_handle.take() {
            handle.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rpc::types::RPC_METHODS;
    use std::collections::HashSet;

    #[test]
    fn all_rpc_methods_have_handler() {
        // All methods listed in RPC_METHODS must have a match arm in execute_rpc_call.
        // This list must be kept in sync with the match arms in execute_rpc_call.
        let handled: HashSet<&str> = [
            "get_server_info",
            "get_block_dag_info",
            "get_mempool_entries",
            "get_coin_supply",
            "get_fee_estimate",
            "get_fee_estimate_experimental",
            "get_connected_peer_info",
            "get_peer_addresses",
            "get_current_network",
            "get_sink",
            "get_sink_blue_score",
            "get_info",
            "get_block_count",
            "estimate_network_hashes_per_second",
            "get_headers",
            "get_sync_status",
            "get_virtual_chain",
            "ping",
        ]
        .into();

        for (method, _) in RPC_METHODS {
            assert!(
                handled.contains(method),
                "RPC method '{}' listed in RPC_METHODS but not handled in execute_rpc_call",
                method
            );
        }

        for method in &handled {
            assert!(
                RPC_METHODS.iter().any(|(name, _)| name == method),
                "Handler '{}' exists in execute_rpc_call but not listed in RPC_METHODS",
                method
            );
        }
    }
}
