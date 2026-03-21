# tui4kas - Claude Code Instructions

## Build & Check

```bash
cargo build              # compile
cargo clippy -- -D warnings  # lint (treat warnings as errors)
cargo test               # no tests yet, but run to verify
```

## Architecture

Ratatui + Crossterm TUI for monitoring a Kaspa L1 node via wRPC.

### Module Layout

- `src/main.rs` — Entry point, terminal setup, event loop, key handling
- `src/app.rs` — Central state: `App` struct, `Tab` enum, `CommandLine`, `RpcExplorerState`
- `src/cli.rs` — Clap-derived CLI args (`--url`, `--network`, `--refresh-interval-ms`)
- `src/event.rs` — Crossterm event reader in a dedicated thread, sends `AppEvent` via mpsc
- `src/rpc/client.rs` — `RpcManager`: connects to Kaspa node, background polling loop, RPC execution
- `src/rpc/types.rs` — UI-friendly structs with `From` impls for kaspa RPC response types
- `src/ui/mod.rs` — Top-level `draw()` dispatcher
- `src/ui/common.rs` — Header/tab bar rendering
- `src/ui/dashboard.rs` — Dashboard tab (server info, DAG summary, supply, fees)
- `src/ui/mempool.rs` — Mempool tab (transaction table)
- `src/ui/blockdag.rs` — BlockDAG tab (tip hashes, DAG stats)
- `src/ui/rpc_explorer.rs` — RPC Explorer tab (interactive method selector)
- `src/ui/command.rs` — Vim-style command line overlay

### Key Patterns

- **Shared state**: `Arc<tokio::sync::Mutex<App>>` passed to RPC poller and UI
- **Background polling**: `RpcManager::start_polling()` spawns a tokio task that calls 5 RPC methods in parallel via `tokio::join!`, skips when `app.paused`
- **Event handling**: Crossterm runs in a std::thread, sends events to tokio via mpsc
- **Tab navigation**: `Tab/BackTab` keys cycle tabs; each tab has its own key handler

### Dependencies

- Kaspa crates (`kaspa-rpc-core`, `kaspa-wrpc-client`) pinned to git rev `10116df`
- Rust edition 2024
- Async runtime: tokio (full features)

## Conventions

- No test suite yet — verify changes with `cargo clippy` and manual testing
- Command line activated with `:` key (vim-style), supports history with Up/Down
- Press `p` to pause/unpause background polling
- All RPC types have UI-friendly wrapper structs in `rpc/types.rs` — don't use raw kaspa types in UI code
