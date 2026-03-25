use clap::Parser;
use log::LevelFilter;
use tui4kas_core::config::ConfigOverrides;

#[derive(Parser, Debug, Clone)]
#[command(name = "tui4kas", version, about = "Terminal UI for Kaspa L1")]
pub struct CliArgs {
    /// Log level
    #[clap(long, default_value_t = LevelFilter::Info)]
    pub log_level: LevelFilter,

    /// Node wRPC URL (e.g. ws://127.0.0.1:17110)
    #[clap(long)]
    pub url: Option<String>,

    /// Network (mainnet, testnet-10, testnet-11)
    #[clap(long)]
    pub network: Option<String>,

    /// Polling refresh interval in milliseconds
    #[clap(long)]
    pub refresh_interval_ms: Option<u64>,

    /// Analyze from pruning point
    #[clap(long)]
    pub analyze_from_pruning_point: Option<bool>,
}

impl CliArgs {
    pub fn into_overrides(self) -> ConfigOverrides {
        ConfigOverrides {
            url: self.url,
            network: self.network,
            refresh_interval_ms: self.refresh_interval_ms,
            analyze_from_pruning_point: self.analyze_from_pruning_point,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_no_args() {
        let _args = CliArgs::parse_from(["tui4kas"]);
    }
}
