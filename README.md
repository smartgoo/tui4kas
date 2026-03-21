# tui4kas - Kaspa TUI

A terminal UI dashboard for monitoring Kaspa L1 blockchain nodes via wRPC.

Built with [Ratatui](https://ratatui.rs) and [rusty-kaspa](https://github.com/kaspanet/rusty-kaspa).

## Features

- **Dashboard** — Server info, DAG summary, coin supply, fee estimates
- **Mempool** — Live transaction table with fees and orphan status
- **BlockDAG** — Tip hashes, difficulty, DAA score, pruning point
- **RPC Explorer** — Interactive method selector with formatted responses
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
  main.rs          Event loop & terminal setup
  app.rs           App state, tabs, command line
  cli.rs           CLI argument parsing (clap)
  event.rs         Crossterm event handler
  rpc/
    client.rs      RpcManager (connect, poll, execute)
    types.rs       UI-friendly RPC type wrappers
  ui/
    mod.rs         Draw dispatcher
    common.rs      Header & tab bar
    dashboard.rs   Dashboard tab
    mempool.rs     Mempool tab
    blockdag.rs    BlockDAG tab
    rpc_explorer.rs RPC Explorer tab
    command.rs     Command line overlay
```

## License

MIT
