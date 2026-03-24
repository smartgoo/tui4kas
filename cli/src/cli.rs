use clap::{Args, Parser, Subcommand};
use log::LevelFilter;

#[derive(Args)]
pub struct GlobalArgs {
    /// Log level
    #[clap(long, global = true, default_value_t = LevelFilter::Info)]
    pub log_level: LevelFilter,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Get price from CoinGecko price endpoint
    GetPrice,
}

#[derive(Parser)]
pub struct Cli {
    #[clap(flatten)]
    pub global_args: GlobalArgs,

    #[clap(subcommand)]
    pub command: Commands,
}
