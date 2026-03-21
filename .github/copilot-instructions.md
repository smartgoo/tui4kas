# tui4kas - Copilot Instructions

Ratatui + Crossterm TUI for monitoring Kaspa L1 blockchain nodes via wRPC.

## Key Facts
- Rust edition 2024, async with tokio
- Kaspa crates from git (rev 10116df)
- Shared state: `Arc<tokio::sync::Mutex<App>>` in `src/app.rs`
- Background polling: `RpcManager::start_polling()` in `src/rpc/client.rs`
- UI tabs: Dashboard, Mempool, BlockDAG, RPC Explorer (each in `src/ui/`)
- RPC types wrapped in `src/rpc/types.rs` — use these in UI, not raw kaspa types

## Build
```bash
cargo build
cargo clippy -- -D warnings
```
