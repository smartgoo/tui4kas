# tui4kas - Kaspa TUI

> [!CAUTION]
> **This is an EXPERIMENTAL & VIBE CODED project. Analytics processes have not been audited yet. Data likely has inaccuracies.**
>
> Please consider this alpha status.
>

A terminal UI dashboard for monitoring Kaspa L1 blockchain nodes via wRPC.

Built with [Ratatui](https://ratatui.rs) and [rusty-kaspa](https://github.com/kaspanet/rusty-kaspa).

## Features

- **Dashboard** — Server info, network stats, coin supply, markets, mempool & fees, mining info
- **Mining** — Mining dashboard with hashrate, block rewards, and pool stats
- **Mempool** — Live transaction table with fees and orphan status
- **BlockDAG** — DAG visualizer, metrics, tip/parent hash selection with block info popup
- **Analytics** — Fee analysis, transaction summary, top receivers with time windows
- **RPC Explorer** — Interactive method selector with scrollable formatted responses
- **Settings** — In-app configuration
- **Command Line** — Vim-style `:` command interface with history

## Prerequisites

- Rust (edition 2024 / nightly toolchain)
- Access to a Kaspa node (or use the public resolver)

## Build & Run

```bash
cargo build --release
./target/release/tui4kas
```

### CLI Options

| Flag | Description | Default |
|------|-------------|---------|
| `-u, --url <URL>` | wRPC endpoint (e.g., `ws://127.0.0.1:17110`) | Public resolver |
| `-n, --network <NET>` | Network: `mainnet`, `testnet-10`, `testnet-11` | `mainnet` |
| `-r, --refresh-interval-ms <MS>` | Polling interval in milliseconds | `1000` |

### Examples

```bash
# Connect via public resolver (default)
tui4kas

# Connect to a local node
tui4kas --url ws://127.0.0.1:17110

# Connect to testnet with 2s refresh
tui4kas --network testnet-10 --refresh-interval-ms 2000
```

## Keybindings

| Key | Action |
|-----|--------|
| `Tab` / `Shift+Tab` | Next / previous tab |
| `q` / `Ctrl+C` | Quit |
| `p` | Pause / unpause polling |
| `:` | Open command line |
| `Esc` | Close command line |
| `Up` / `Down` | Scroll lists / command history |
| `j` / `k` | Scroll RPC Explorer response |
| `Enter` | Execute RPC method (RPC Explorer tab) |

## Architecture

```
src/
  main.rs              Event loop & terminal setup
  app.rs               App state, tabs, command line
  cli.rs               CLI argument parsing (clap)
  config.rs            Persistent app configuration (TOML)
  connection.rs        Connection manager & polling handles
  event.rs             Crossterm event handler
  keys.rs              Key event handling & dispatch
  analytics.rs         Analytics engine (protocol detection, aggregation)
  analytics_streaming.rs  VSPC V2 streaming analytics task
  rpc/
    mod.rs             RPC module re-exports
    client.rs          RpcManager (connect, poll, execute)
    market.rs          CoinGecko market data polling
    types.rs           UI-friendly RPC type wrappers
  ui/
    mod.rs             Draw dispatcher
    common.rs          Header & tab bar
    dashboard.rs       Dashboard tab
    mining.rs          Mining tab
    mempool.rs         Mempool tab
    blockdag.rs        BlockDAG tab
    analytics.rs       Analytics tab
    rpc_explorer.rs    RPC Explorer tab
    settings.rs        Settings tab
    help.rs            Help overlay
    command.rs         Command line overlay
```

## License

MIT
