# tui4kas - Claude Code Instructions

## Build & Check

```bash
cargo build              # compile
cargo clippy -- -D warnings  # lint (treat warnings as errors)
cargo test               # run test suite (74+ tests)
```

## Architecture

Ratatui + Crossterm TUI for monitoring a Kaspa L1 node via wRPC.

### Module Layout

- `src/main.rs` — Entry point, terminal setup, event loop
- `src/app.rs` — Central state: `App` struct, `Tab` enum (Dashboard, Mining, Mempool, BlockDag, Analytics, RpcExplorer, Settings), `CommandLine`, `RpcExplorerState`, `DagVisualizer`
- `src/cli.rs` — Clap-derived CLI args (`--url`, `--network`, `--refresh-interval-ms`)
- `src/config.rs` — Persistent app configuration (`AppConfig`, TOML-based)
- `src/connection.rs` — Connection manager and cancellable background polling handles
- `src/event.rs` — Crossterm event reader in a dedicated thread, sends `AppEvent` via mpsc
- `src/keys.rs` — Key event handling and dispatch to tab-specific handlers
- `src/analytics.rs` — Analytics engine: protocol detection, aggregation, time windows
- `src/analytics_streaming.rs` — VSPC V2 streaming analytics task
- `src/rpc/client.rs` — `RpcManager`: connects to Kaspa node, background polling loop, RPC execution, mining/analytics data fetching
- `src/rpc/market.rs` — CoinGecko API market data polling (price, market cap, volume)
- `src/rpc/types.rs` — UI-friendly structs with `From` impls for kaspa RPC response types
- `src/ui/mod.rs` — Top-level `draw()` dispatcher
- `src/ui/common.rs` — Header/tab bar rendering
- `src/ui/dashboard.rs` — Dashboard tab (node info, network stats + coin supply, markets, mempool & fees, mining info)
- `src/ui/mining.rs` — Mining tab (hashrate, block rewards, pool stats)
- `src/ui/mempool.rs` — Mempool tab (transaction table with selection, detail popup)
- `src/ui/blockdag.rs` — BlockDAG tab (DAG visualizer, metrics, tip/parent hash selection with block info popup)
- `src/ui/analytics.rs` — Analytics tab (fee analysis, transaction summary, top receivers)
- `src/ui/rpc_explorer.rs` — RPC Explorer tab (interactive method selector with scrollable results)
- `src/ui/settings.rs` — Settings tab (in-app configuration)
- `src/ui/help.rs` — Help overlay
- `src/ui/command.rs` — Vim-style command line overlay

### Key Patterns

- **Shared state**: `Arc<tokio::sync::RwLock<App>>` passed to RPC poller and UI
- **Background polling**: `RpcManager::start_polling()` spawns a tokio task that calls 5 RPC methods in parallel via `tokio::join!`, skips when `app.paused`
- **Market data polling**: Separate tokio task polls CoinGecko API every 60 seconds
- **Mining/Analytics polling**: Separate tokio tasks poll chain data every 30 seconds (delayed start)
- **Event handling**: Crossterm runs in a std::thread, sends events to tokio via mpsc
- **Tab navigation**: `Tab/BackTab` keys cycle 7 tabs; each tab has its own key handler in `keys.rs`
- **Popups**: Mempool detail and BlockDAG block info use overlay popups closed with Esc

### Dependencies

- Kaspa crates (`kaspa-rpc-core`, `kaspa-wrpc-client`) pinned to git rev `10116df`
- `reqwest` for CoinGecko HTTP API calls
- Rust edition 2024
- Async runtime: tokio (full features)

## Conventions

- Test suite covers app state, types, formatting helpers — run `cargo test` after changes
- Command line activated with `:` key (vim-style), supports history with Up/Down
- Press `p` to pause/unpause background polling
- All RPC types have UI-friendly wrapper structs in `rpc/types.rs` — don't use raw kaspa types in UI code
- Label colors use `Color::DarkGray` (not `Color::Gray`) for macOS terminal readability
- 18 RPC methods available in both explorer and command line
