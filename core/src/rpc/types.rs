use kaspa_rpc_core::{
    GetBlockDagInfoResponse, GetCoinSupplyResponse, GetServerInfoResponse, RpcFeeEstimate,
    RpcMempoolEntry,
};

#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub server_version: String,
    pub network_id: String,
    pub is_synced: bool,
    pub has_utxo_index: bool,
}

impl From<GetServerInfoResponse> for ServerInfo {
    fn from(r: GetServerInfoResponse) -> Self {
        Self {
            server_version: r.server_version,
            network_id: r.network_id.to_string(),
            is_synced: r.is_synced,
            has_utxo_index: r.has_utxo_index,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DagInfo {
    pub block_count: u64,
    pub header_count: u64,
    pub tip_hashes: Vec<String>,
    pub difficulty: f64,
    pub past_median_time: u64,
    pub pruning_point_hash: String,
    pub virtual_daa_score: u64,
    pub sink: String,
}

impl From<GetBlockDagInfoResponse> for DagInfo {
    fn from(r: GetBlockDagInfoResponse) -> Self {
        Self {
            block_count: r.block_count,
            header_count: r.header_count,
            tip_hashes: r.tip_hashes.iter().map(|h| h.to_string()).collect(),
            difficulty: r.difficulty,
            past_median_time: r.past_median_time,
            pruning_point_hash: r.pruning_point_hash.to_string(),
            virtual_daa_score: r.virtual_daa_score,
            sink: r.sink.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MempoolEntryInfo {
    pub transaction_id: String,
    pub fee: u64,
    pub is_orphan: bool,
}

impl From<RpcMempoolEntry> for MempoolEntryInfo {
    fn from(e: RpcMempoolEntry) -> Self {
        let transaction_id = e
            .transaction
            .verbose_data
            .as_ref()
            .map(|v| v.transaction_id.to_string())
            .unwrap_or_else(|| format!("mass:{}", e.transaction.mass));
        Self {
            transaction_id,
            fee: e.fee,
            is_orphan: e.is_orphan,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MempoolState {
    pub entry_count: usize,
    pub entries: Vec<MempoolEntryInfo>,
    pub total_fees: u64,
}

impl From<Vec<RpcMempoolEntry>> for MempoolState {
    fn from(r: Vec<RpcMempoolEntry>) -> Self {
        let entries: Vec<MempoolEntryInfo> = r.into_iter().map(|e| e.into()).collect();
        let total_fees = entries.iter().map(|e| e.fee).sum();
        let entry_count = entries.len();
        Self {
            entry_count,
            entries,
            total_fees,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CoinSupplyInfo {
    pub max_sompi: u64,
    pub circulating_sompi: u64,
}

impl From<GetCoinSupplyResponse> for CoinSupplyInfo {
    fn from(r: GetCoinSupplyResponse) -> Self {
        Self {
            max_sompi: r.max_sompi,
            circulating_sompi: r.circulating_sompi,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FeeEstimateInfo {
    pub priority_bucket: String,
    pub normal_buckets: Vec<String>,
    pub low_buckets: Vec<String>,
}

impl From<RpcFeeEstimate> for FeeEstimateInfo {
    fn from(r: RpcFeeEstimate) -> Self {
        Self {
            priority_bucket: format!("{:.8} KAS/gram", r.priority_bucket.feerate / 1e8),
            normal_buckets: r
                .normal_buckets
                .iter()
                .map(|b| format!("{:.8} KAS/gram", b.feerate / 1e8))
                .collect(),
            low_buckets: r
                .low_buckets
                .iter()
                .map(|b| format!("{:.8} KAS/gram", b.feerate / 1e8))
                .collect(),
        }
    }
}

pub fn sompi_to_kas(sompi: u64) -> f64 {
    sompi as f64 / 1e8
}

pub fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

#[derive(Debug, Clone, Default)]
pub struct MiningInfo {
    pub hashrate: f64,
    pub unique_miners: usize,
    pub all_miners: Vec<(String, usize)>,
    pub blocks_analyzed: usize,
    pub pools: Vec<(String, usize)>,
    pub node_versions: Vec<(String, usize)>,
}

#[derive(Debug, Clone)]
pub struct CoinbaseInfo {
    pub node_version: Option<String>,
    pub pool_name: Option<String>,
}

pub fn parse_coinbase_payload(payload: &[u8]) -> CoinbaseInfo {
    let none = CoinbaseInfo {
        node_version: None,
        pool_name: None,
    };

    if payload.len() < 19 {
        return none;
    }

    let script_len = payload[18] as usize;
    let data_start = 19 + script_len;

    if data_start >= payload.len() {
        return none;
    }

    // Check for address-style payload (0xaa first byte in script)
    if script_len > 0 && payload[19] == 0xaa {
        return none;
    }

    let payload_str = String::from_utf8_lossy(&payload[data_start..]);
    let payload_str = payload_str.trim_matches('\0').trim();

    if payload_str.is_empty() {
        return none;
    }

    let parts: Vec<&str> = payload_str.splitn(3, '/').collect();
    let node_version = parts
        .first()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let pool_name = parts
        .get(1)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    CoinbaseInfo {
        node_version,
        pool_name,
    }
}

pub fn format_hashrate(hps: f64) -> String {
    if hps >= 1e18 {
        format!("{:.2} EH/s", hps / 1e18)
    } else if hps >= 1e15 {
        format!("{:.2} PH/s", hps / 1e15)
    } else if hps >= 1e12 {
        format!("{:.2} TH/s", hps / 1e12)
    } else if hps >= 1e9 {
        format!("{:.2} GH/s", hps / 1e9)
    } else if hps >= 1e6 {
        format!("{:.2} MH/s", hps / 1e6)
    } else if hps >= 1e3 {
        format!("{:.2} KH/s", hps / 1e3)
    } else {
        format!("{:.2} H/s", hps)
    }
}

// --- DagStats ---

#[derive(Debug, Clone)]
pub struct DagSample {
    pub timestamp: std::time::Instant,
    pub blue_score: u64,
}

#[derive(Debug, Clone, Default)]
pub struct DagStats {
    pub samples: std::collections::VecDeque<DagSample>,
    pub sink_blue_score: Option<u64>,
}

impl DagStats {
    pub fn update(&mut self, blue_score: Option<u64>) {
        self.sink_blue_score = blue_score;
        if let Some(score) = blue_score {
            self.samples.push_back(DagSample {
                timestamp: std::time::Instant::now(),
                blue_score: score,
            });
            while self.samples.len() > 120 {
                self.samples.pop_front();
            }
        }
    }

    pub fn blue_block_rate(&self) -> Option<f64> {
        if self.samples.len() < 2 {
            return None;
        }
        let first = self.samples.front()?;
        let last = self.samples.back()?;
        let elapsed = last.timestamp.duration_since(first.timestamp).as_secs_f64();
        if elapsed < 0.1 {
            return None;
        }
        let delta = last.blue_score.saturating_sub(first.blue_score) as f64;
        Some(delta / elapsed)
    }

    pub fn block_interval_ms(&self) -> Option<f64> {
        if self.samples.len() < 2 {
            return None;
        }
        let first = self.samples.front()?;
        let last = self.samples.back()?;
        let delta = last.blue_score.saturating_sub(first.blue_score);
        if delta == 0 {
            return None;
        }
        let elapsed_ms = last.timestamp.duration_since(first.timestamp).as_secs_f64() * 1000.0;
        Some(elapsed_ms / delta as f64)
    }
}

/// All RPC methods available in the explorer and command line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RpcMethod {
    GetServerInfo,
    GetBlockDagInfo,
    GetBlockCount,
    GetMempoolEntries,
    GetCoinSupply,
    GetFeeEstimate,
    GetFeeEstimateExperimental,
    GetConnectedPeerInfo,
    GetPeerAddresses,
    GetCurrentNetwork,
    GetSink,
    GetSinkBlueScore,
    GetInfo,
    GetSyncStatus,
    GetVirtualChain,
    GetHeaders,
    EstimateNetworkHashesPerSecond,
    Ping,
}

impl RpcMethod {
    /// All variants in display order.
    pub const ALL: &[RpcMethod] = &[
        Self::GetServerInfo,
        Self::GetBlockDagInfo,
        Self::GetBlockCount,
        Self::GetMempoolEntries,
        Self::GetCoinSupply,
        Self::GetFeeEstimate,
        Self::GetFeeEstimateExperimental,
        Self::GetConnectedPeerInfo,
        Self::GetPeerAddresses,
        Self::GetCurrentNetwork,
        Self::GetSink,
        Self::GetSinkBlueScore,
        Self::GetInfo,
        Self::GetSyncStatus,
        Self::GetVirtualChain,
        Self::GetHeaders,
        Self::EstimateNetworkHashesPerSecond,
        Self::Ping,
    ];

    /// Snake_case name used for display and parsing.
    pub fn name(self) -> &'static str {
        match self {
            Self::GetServerInfo => "get_server_info",
            Self::GetBlockDagInfo => "get_block_dag_info",
            Self::GetBlockCount => "get_block_count",
            Self::GetMempoolEntries => "get_mempool_entries",
            Self::GetCoinSupply => "get_coin_supply",
            Self::GetFeeEstimate => "get_fee_estimate",
            Self::GetFeeEstimateExperimental => "get_fee_estimate_experimental",
            Self::GetConnectedPeerInfo => "get_connected_peer_info",
            Self::GetPeerAddresses => "get_peer_addresses",
            Self::GetCurrentNetwork => "get_current_network",
            Self::GetSink => "get_sink",
            Self::GetSinkBlueScore => "get_sink_blue_score",
            Self::GetInfo => "get_info",
            Self::GetSyncStatus => "get_sync_status",
            Self::GetVirtualChain => "get_virtual_chain",
            Self::GetHeaders => "get_headers",
            Self::EstimateNetworkHashesPerSecond => "estimate_network_hashes_per_second",
            Self::Ping => "ping",
        }
    }

    /// Human-readable description.
    pub fn description(self) -> &'static str {
        match self {
            Self::GetServerInfo => "Get server info",
            Self::GetBlockDagInfo => "Get block DAG info",
            Self::GetBlockCount => "Get block count",
            Self::GetMempoolEntries => "Get mempool entries",
            Self::GetCoinSupply => "Get coin supply",
            Self::GetFeeEstimate => "Get fee estimate",
            Self::GetFeeEstimateExperimental => "Get experimental fee estimate (verbose)",
            Self::GetConnectedPeerInfo => "Get connected peer info",
            Self::GetPeerAddresses => "Get known peer addresses",
            Self::GetCurrentNetwork => "Get current network type",
            Self::GetSink => "Get sink (virtual selected parent) hash",
            Self::GetSinkBlueScore => "Get sink blue score",
            Self::GetInfo => "Get general node info",
            Self::GetSyncStatus => "Get sync status",
            Self::GetVirtualChain => "Get virtual selected parent chain",
            Self::GetHeaders => "Get header count",
            Self::EstimateNetworkHashesPerSecond => "Estimate network hashrate",
            Self::Ping => "Ping the node",
        }
    }
}

impl std::fmt::Display for RpcMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::str::FromStr for RpcMethod {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .iter()
            .find(|m| m.name() == s)
            .copied()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Unknown RPC method: '{}'. Type 'help' for available commands.",
                    s
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaspa_rpc_core::{
        RpcFeerateBucket, RpcHash, RpcMempoolEntry, RpcNetworkId, RpcSubnetworkId, RpcTransaction,
        RpcTransactionVerboseData,
    };
    use std::str::FromStr;

    // --- sompi_to_kas ---

    #[test]
    fn sompi_to_kas_zero() {
        assert_eq!(sompi_to_kas(0), 0.0);
    }

    #[test]
    fn sompi_to_kas_one_kas() {
        assert_eq!(sompi_to_kas(100_000_000), 1.0);
    }

    #[test]
    fn sompi_to_kas_fractional() {
        let result = sompi_to_kas(50_000_000);
        assert!((result - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn sompi_to_kas_large_value() {
        let result = sompi_to_kas(2_900_000_000_000_000_000);
        assert!((result - 29_000_000_000.0).abs() < 1.0);
    }

    // --- From<GetServerInfoResponse> ---

    #[test]
    fn server_info_from_response() {
        let network_id = RpcNetworkId::from_str("mainnet").unwrap();
        let response = GetServerInfoResponse {
            rpc_api_version: 1,
            rpc_api_revision: 0,
            server_version: "0.14.0".to_string(),
            network_id,
            has_utxo_index: true,
            is_synced: true,
            virtual_daa_score: 12345,
        };
        let info: ServerInfo = response.into();
        assert_eq!(info.server_version, "0.14.0");
        assert!(info.is_synced);
        assert!(info.has_utxo_index);
        assert!(info.network_id.contains("mainnet"));
    }

    // --- From<GetBlockDagInfoResponse> ---

    #[test]
    fn dag_info_from_response() {
        let hash1 = RpcHash::from_bytes([1u8; 32]);
        let hash2 = RpcHash::from_bytes([2u8; 32]);
        let network_id = RpcNetworkId::from_str("mainnet").unwrap();
        let response = GetBlockDagInfoResponse::new(
            network_id,
            1000,
            2000,
            vec![hash1, hash2],
            1234.56,
            9999999,
            vec![hash1],
            hash2,
            5000,
            hash1,
        );
        let info: DagInfo = response.into();
        assert_eq!(info.block_count, 1000);
        assert_eq!(info.header_count, 2000);
        assert_eq!(info.tip_hashes.len(), 2);
        assert!((info.difficulty - 1234.56).abs() < f64::EPSILON);
        assert_eq!(info.past_median_time, 9999999);
    }

    // --- From<GetCoinSupplyResponse> ---

    #[test]
    fn coin_supply_from_response() {
        let response =
            GetCoinSupplyResponse::new(2_900_000_000_000_000_000, 1_000_000_000_000_000_000);
        let info: CoinSupplyInfo = response.into();
        assert_eq!(info.max_sompi, 2_900_000_000_000_000_000);
        assert_eq!(info.circulating_sompi, 1_000_000_000_000_000_000);
    }

    // --- From<RpcMempoolEntry> ---

    fn make_test_transaction(verbose_data: Option<RpcTransactionVerboseData>) -> RpcTransaction {
        RpcTransaction {
            version: 0,
            inputs: vec![],
            outputs: vec![],
            lock_time: 0,
            subnetwork_id: RpcSubnetworkId::from_byte(0),
            gas: 0,
            payload: vec![],
            mass: 42,
            verbose_data,
        }
    }

    #[test]
    fn mempool_entry_without_verbose_data() {
        let entry = RpcMempoolEntry::new(500, make_test_transaction(None), false);
        let info: MempoolEntryInfo = entry.into();
        assert_eq!(info.transaction_id, "mass:42");
        assert_eq!(info.fee, 500);
        assert!(!info.is_orphan);
    }

    #[test]
    fn mempool_entry_with_verbose_data() {
        let tx_id = RpcHash::from_bytes([0xAB; 32]);
        let verbose = RpcTransactionVerboseData {
            transaction_id: tx_id,
            hash: RpcHash::from_bytes([0; 32]),
            compute_mass: 100,
            block_hash: RpcHash::from_bytes([0; 32]),
            block_time: 0,
        };
        let entry = RpcMempoolEntry::new(1000, make_test_transaction(Some(verbose)), true);
        let info: MempoolEntryInfo = entry.into();
        assert!(info.transaction_id.contains("abab"));
        assert_eq!(info.fee, 1000);
        assert!(info.is_orphan);
    }

    // --- From<Vec<RpcMempoolEntry>> ---

    #[test]
    fn mempool_state_empty() {
        let entries: Vec<RpcMempoolEntry> = vec![];
        let state: MempoolState = entries.into();
        assert_eq!(state.entry_count, 0);
        assert_eq!(state.total_fees, 0);
        assert!(state.entries.is_empty());
    }

    #[test]
    fn mempool_state_fee_summation() {
        let entries = vec![
            RpcMempoolEntry::new(100, make_test_transaction(None), false),
            RpcMempoolEntry::new(250, make_test_transaction(None), false),
            RpcMempoolEntry::new(50, make_test_transaction(None), true),
        ];
        let state: MempoolState = entries.into();
        assert_eq!(state.entry_count, 3);
        assert_eq!(state.total_fees, 400);
    }

    // --- From<RpcFeeEstimate> ---

    #[test]
    fn fee_estimate_formatting() {
        let estimate = RpcFeeEstimate {
            priority_bucket: RpcFeerateBucket {
                feerate: 100_000_000.0,
                estimated_seconds: 0.5,
            },
            normal_buckets: vec![RpcFeerateBucket {
                feerate: 50_000_000.0,
                estimated_seconds: 30.0,
            }],
            low_buckets: vec![],
        };
        let info: FeeEstimateInfo = estimate.into();
        assert_eq!(info.priority_bucket, "1.00000000 KAS/gram");
        assert_eq!(info.normal_buckets.len(), 1);
        assert_eq!(info.normal_buckets[0], "0.50000000 KAS/gram");
        assert!(info.low_buckets.is_empty());
    }

    // --- format_number ---

    #[test]
    fn format_number_zero() {
        assert_eq!(format_number(0), "0");
    }

    #[test]
    fn format_number_small() {
        assert_eq!(format_number(999), "999");
    }

    #[test]
    fn format_number_thousands() {
        assert_eq!(format_number(1_000), "1,000");
        assert_eq!(format_number(12_345), "12,345");
    }

    #[test]
    fn format_number_millions() {
        assert_eq!(format_number(1_000_000), "1,000,000");
    }

    // --- parse_coinbase_payload ---

    #[test]
    fn parse_coinbase_payload_too_short() {
        let info = parse_coinbase_payload(&[0u8; 10]);
        assert!(info.node_version.is_none());
        assert!(info.pool_name.is_none());
    }

    #[test]
    fn parse_coinbase_payload_version_and_pool() {
        // 19 bytes of header + script_len=0 at byte 18 + "0.14.1/MyPool/"
        let mut payload = vec![0u8; 18];
        payload.push(0); // script_len = 0
        payload.extend_from_slice(b"0.14.1/MyPool/extra");
        let info = parse_coinbase_payload(&payload);
        assert_eq!(info.node_version.as_deref(), Some("0.14.1"));
        assert_eq!(info.pool_name.as_deref(), Some("MyPool"));
    }

    #[test]
    fn parse_coinbase_payload_version_only() {
        let mut payload = vec![0u8; 18];
        payload.push(0);
        payload.extend_from_slice(b"0.14.1");
        let info = parse_coinbase_payload(&payload);
        assert_eq!(info.node_version.as_deref(), Some("0.14.1"));
        assert!(info.pool_name.is_none());
    }

    #[test]
    fn parse_coinbase_payload_with_script() {
        let mut payload = vec![0u8; 18];
        payload.push(3); // script_len = 3
        payload.extend_from_slice(&[0x01, 0x02, 0x03]); // script bytes
        payload.extend_from_slice(b"0.13.2/PoolX");
        let info = parse_coinbase_payload(&payload);
        assert_eq!(info.node_version.as_deref(), Some("0.13.2"));
        assert_eq!(info.pool_name.as_deref(), Some("PoolX"));
    }

    #[test]
    fn parse_coinbase_payload_aa_script() {
        let mut payload = vec![0u8; 18];
        payload.push(2); // script_len = 2
        payload.extend_from_slice(&[0xaa, 0x01]); // 0xaa first byte
        payload.extend_from_slice(b"0.14.1/Pool");
        let info = parse_coinbase_payload(&payload);
        assert!(info.node_version.is_none());
        assert!(info.pool_name.is_none());
    }

    // --- format_hashrate ---

    #[test]
    fn format_hashrate_terahash() {
        assert_eq!(format_hashrate(1.5e12), "1.50 TH/s");
    }

    #[test]
    fn format_hashrate_petahash() {
        assert_eq!(format_hashrate(2.0e15), "2.00 PH/s");
    }
}
