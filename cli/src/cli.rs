use clap::{Args, Parser, Subcommand};
use log::LevelFilter;
use tui4kas_core::config::ConfigOverrides;

#[derive(Args)]
pub struct GlobalArgs {
    /// Log level
    #[clap(long, global = true, default_value_t = LevelFilter::Info)]
    pub log_level: LevelFilter,

    /// Node wRPC URL (e.g. ws://127.0.0.1:17110)
    #[clap(long, global = true)]
    pub url: Option<String>,

    /// Network (mainnet, testnet-10, testnet-11)
    #[clap(long, global = true)]
    pub network: Option<String>,

    /// Polling refresh interval in milliseconds
    #[clap(long, global = true)]
    pub refresh_interval_ms: Option<u64>,

    /// Analyze from pruning point
    #[clap(long, global = true)]
    pub analyze_from_pruning_point: Option<bool>,
}

impl GlobalArgs {
    pub fn into_overrides(self) -> ConfigOverrides {
        ConfigOverrides {
            url: self.url,
            network: self.network,
            refresh_interval_ms: self.refresh_interval_ms,
            analyze_from_pruning_point: self.analyze_from_pruning_point,
        }
    }
}

#[allow(clippy::enum_variant_names)]
#[derive(Debug, Subcommand)]
pub enum RpcCommands {
    /// Get server info from connected node
    GetServerInfo,
    /// Get block DAG info from connected node
    GetDagInfo,
    /// Get mempool entries from connected node
    GetMempool,
    /// Get mining info (hashrate, miners, pools)
    GetMiningInfo {
        /// Number of blocks to analyze
        #[clap(long, default_value_t = 500)]
        blocks: usize,
    },
    /// Get coin supply from connected node
    GetCoinSupply,
    /// Get fee estimate from connected node
    GetFeeEstimate,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Get price from CoinGecko price endpoint
    GetPrice,
    /// RPC commands for interacting with a Kaspa node
    Rpc {
        #[clap(subcommand)]
        command: RpcCommands,
    },
}

#[derive(Parser)]
#[command(name = "tui4kas-cli", version, about = "CLI for Kaspa L1")]
pub struct Cli {
    #[clap(flatten)]
    pub global_args: GlobalArgs,

    #[clap(subcommand)]
    pub command: Commands,
}
