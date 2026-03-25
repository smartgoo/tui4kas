mod cli;

use anyhow::Result;
use clap::Parser;
use cli::{Commands, RpcCommands};
use log::info;
use tui4kas_core::config::AppConfig;
use tui4kas_core::log::{LogTarget, init_logger};
use tui4kas_core::rpc::client::RpcClient;
use tui4kas_core::rpc::types::*;

async fn connect_rpc(config: &AppConfig) -> Result<RpcClient> {
    let rpc = RpcClient::new(config.url.clone(), &config.network)?;
    rpc.connect().await?;
    Ok(rpc)
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = cli::Cli::parse();

    let log_level = cli.global_args.log_level;
    let mut config = AppConfig::load().unwrap_or_default();
    config.apply_overrides(cli.global_args.into_overrides());

    init_logger(LogTarget::Cli, log_level).unwrap();
    info!(target: LogTarget::Cli.as_str(), "{:?} command starting...", cli.command);

    match cli.command {
        Commands::GetPrice => {
            let price = tui4kas_core::price::fetch_market_data(None).await?;
            println!("{}", serde_json::to_string_pretty(&price)?);
        }
        Commands::Rpc { command } => {
            let rpc = connect_rpc(&config).await?;
            match command {
                RpcCommands::GetServerInfo => {
                    let result = rpc.execute_rpc_call(RpcMethod::GetServerInfo).await?;
                    println!("{}", result);
                }
                RpcCommands::GetDagInfo => {
                    let result = rpc.execute_rpc_call(RpcMethod::GetBlockDagInfo).await?;
                    println!("{}", result);
                }
                RpcCommands::GetMempool => {
                    let result = rpc.execute_rpc_call(RpcMethod::GetMempoolEntries).await?;
                    println!("{}", result);
                }
                RpcCommands::GetMiningInfo { blocks } => {
                    let info = rpc.fetch_mining_info(blocks).await?;
                    println!("Network Hashrate: {}", format_hashrate(info.hashrate));
                    println!(
                        "Blocks Analyzed:  {}",
                        format_number(info.blocks_analyzed as u64)
                    );
                    println!("Unique Miners:    {}", info.unique_miners);
                    println!();
                    if !info.pools.is_empty() {
                        println!("Top Pools:");
                        for (pool, count) in &info.pools {
                            println!("  {}: {} blocks", pool, count);
                        }
                        println!();
                    }
                    if !info.all_miners.is_empty() {
                        println!("Top Miners:");
                        for (addr, count) in info.all_miners.iter().take(10) {
                            println!("  {}: {} blocks", addr, count);
                        }
                        println!();
                    }
                    if !info.node_versions.is_empty() {
                        println!("Node Versions:");
                        for (version, count) in &info.node_versions {
                            println!("  {}: {} blocks", version, count);
                        }
                    }
                }
                RpcCommands::GetCoinSupply => {
                    let result = rpc.execute_rpc_call(RpcMethod::GetCoinSupply).await?;
                    println!("{}", result);
                }
                RpcCommands::GetFeeEstimate => {
                    let result = rpc.execute_rpc_call(RpcMethod::GetFeeEstimate).await?;
                    println!("{}", result);
                }
            }
        }
    }

    Ok(())
}
