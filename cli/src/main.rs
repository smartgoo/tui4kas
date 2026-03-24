mod cli;

use clap::Parser;
use cli::Commands;

#[tokio::main]
async fn main() {
    let cli = cli::Cli::parse();

    match cli.command {
        Commands::GetPrice => {
            let price = tui4kas_core::price::fetch_market_data(None).await.unwrap();
            println!("{:?}", price)
        }
    }

    ()
}
