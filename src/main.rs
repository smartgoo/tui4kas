mod analytics;
mod analytics_streaming;
mod app;
mod cli;
mod config;
mod connection;
mod event;
mod keys;
mod rpc;
mod ui;

use std::io;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use crossterm::ExecutableCommand;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tokio::sync::RwLock;

use crate::app::App;
use crate::cli::CliArgs;
use crate::config::AppConfig;
use crate::connection::{PollingHandles, create_and_start_rpc, start_mining_polling};
use crate::event::{AppEvent, EventHandler};
use crate::keys::SettingsCommand;
use crate::rpc::client::RpcManager;
use crate::rpc::market;

#[tokio::main]
async fn main() -> Result<()> {
    let _args = CliArgs::parse();

    // Load app config
    let config = AppConfig::load().unwrap_or_default();

    // Set up panic hook to restore terminal
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = io::stdout().execute(crossterm::event::DisableMouseCapture);
        let _ = io::stdout().execute(LeaveAlternateScreen);
        original_hook(panic_info);
    }));

    // Initialize terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    stdout.execute(crossterm::event::EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create shared app state
    let app = Arc::new(RwLock::new(App::new(config.clone())));

    let refresh_interval_ms = config.refresh_interval_ms;
    let network = config.network.clone();

    // Settings/reconnection channel
    let (settings_tx, mut settings_rx) = tokio::sync::mpsc::channel::<SettingsCommand>(4);

    let mut polling_handles = PollingHandles::new();

    // Start market data polling (every 60 seconds) — independent of node
    market::start_market_polling(app.clone(), Duration::from_secs(60));

    // Connect to node (URL or PNN)
    let mut rpc_for_explorer: Arc<RpcManager> = create_and_start_rpc(
        config.url.clone(),
        &network,
        &app,
        refresh_interval_ms,
        false,
        &mut polling_handles,
    )
    .await?;

    if config.url.is_some() {
        start_mining_polling(&rpc_for_explorer, &app, &mut polling_handles);
        analytics_streaming::start_analytics_streaming(
            &rpc_for_explorer,
            &app,
            &mut polling_handles,
        );
    }

    // Event loop
    let mut events = EventHandler::new(Duration::from_millis(250));

    loop {
        // Check for settings/reconnection commands (non-blocking)
        if let Ok(cmd) = settings_rx.try_recv() {
            match cmd {
                SettingsCommand::Reconnect(new_config) => {
                    // Abort old polling
                    polling_handles.abort_all();
                    let _ = rpc_for_explorer.disconnect().await;

                    // Clear stale data
                    {
                        let mut app_guard = app.write().await;
                        app_guard.node.server_info = None;
                        app_guard.node.dag_info = None;
                        app_guard.node.mempool_state = None;
                        app_guard.node.coin_supply = None;
                        app_guard.node.fee_estimate = None;
                        app_guard.node.mining_info = None;
                        app_guard.analytics.engine = None;
                        app_guard.analytics.sync_progress = None;
                        app_guard.analytics.cached_views = None;
                        app_guard.node.node_url = None;
                        app_guard.node.node_uid = None;
                        app_guard.node.connection_status =
                            crate::app::ConnectionStatus::Disconnected;
                    }

                    // Reconnect with new config
                    match create_and_start_rpc(
                        new_config.url.clone(),
                        &new_config.network,
                        &app,
                        new_config.refresh_interval_ms,
                        false,
                        &mut polling_handles,
                    )
                    .await
                    {
                        Ok(new_rpc) => {
                            rpc_for_explorer = new_rpc;
                            if new_config.url.is_some() {
                                start_mining_polling(
                                    &rpc_for_explorer,
                                    &app,
                                    &mut polling_handles,
                                );
                                analytics_streaming::start_analytics_streaming(
                                    &rpc_for_explorer,
                                    &app,
                                    &mut polling_handles,
                                );
                            }
                        }
                        Err(_) => {
                            // Best effort — app continues without RPC
                            rpc_for_explorer = Arc::new(
                                RpcManager::new(None, &new_config.network, app.clone()).await?,
                            );
                        }
                    }
                }
            }
        }

        // Draw (skip if nothing changed)
        {
            let mut app_guard = app.write().await;
            if app_guard.dirty {
                app_guard.dirty = false;
                terminal.draw(|f| ui::draw(f, &app_guard))?;
            }
        }

        // Handle events
        let Some(event) = events.next().await else {
            break;
        };

        match event {
            AppEvent::Key(key) => {
                let mut app_guard = app.write().await;
                app_guard.dirty = true;

                if app_guard.command_line.active {
                    if let Some(cmd) = keys::handle_command_mode_keys(&mut app_guard, key.code) {
                        drop(app_guard);
                        keys::handle_command(&cmd, &app, &rpc_for_explorer).await;
                        continue;
                    }
                } else {
                    let consumed = keys::handle_normal_keys(
                        &mut app_guard,
                        key,
                        &rpc_for_explorer,
                        &app,
                        &settings_tx,
                    );
                    if consumed {
                        continue;
                    }
                }

                if app_guard.should_quit {
                    break;
                }
            }
            AppEvent::Mouse(mouse) => {
                let mut app_guard = app.write().await;
                app_guard.dirty = true;
                if keys::handle_mouse(&mut app_guard, mouse) {
                    continue;
                }
            }
            AppEvent::Tick => {
                // Clear clipboard flash after one tick
                let mut app_guard = app.write().await;
                if app_guard.clipboard_flash.is_some() {
                    app_guard.clipboard_flash = None;
                    app_guard.dirty = true;
                }
            }
            AppEvent::Resize(_, _) => {
                app.write().await.dirty = true;
            }
        }
    }

    // Clean shutdown
    polling_handles.abort_all();
    drop(rpc_for_explorer);

    // Persist analytics cache before exit
    {
        let app_guard = app.read().await;
        if let Some(ref engine) = app_guard.analytics.engine
            && let Ok(eng) = engine.try_read()
        {
            let cache_path = dirs::home_dir()
                .unwrap_or_default()
                .join(".tui4kas")
                .join("analytics_cache.bin");
            let _ = eng.save(&cache_path);
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    terminal
        .backend_mut()
        .execute(crossterm::event::DisableMouseCapture)?;
    terminal.backend_mut().execute(LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
