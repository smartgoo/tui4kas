# tui4kas - Copilot Instructions

Ratatui + Crossterm TUI for monitoring Kaspa L1 blockchain nodes via wRPC.

## Key Facts
- Rust edition 2024, async with tokio
- Kaspa crates from git (rev 10116df)
- Shared state: `Arc<tokio::sync::RwLock<App>>` in `src/app.rs`
- Background polling: `RpcManager::start_polling()` in `src/rpc/client.rs`
- Connection management: `PollingHandles` in `src/connection.rs`
- Key handling: `src/keys.rs` dispatches to tab-specific handlers
- Config: `AppConfig` in `src/config.rs` (TOML-based persistent config)
- Analytics: `src/analytics.rs` (engine) + `src/analytics_streaming.rs` (VSPC V2 streaming)
- UI tabs: Dashboard, Mining, Mempool, BlockDAG, Analytics, RPC Explorer, Settings (each in `src/ui/`)
- RPC types wrapped in `src/rpc/types.rs` — use these in UI, not raw kaspa types

## Build
```bash
cargo build
cargo clippy -- -D warnings
```
