mod analytics;
mod analytics_streaming;
mod app;
mod cli;
mod config;
mod daemon;
mod daemon_lifecycle;
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

use crate::app::{App, DaemonStatus};
use crate::cli::CliArgs;
use crate::config::DaemonConfig;
use crate::daemon_lifecycle::{PollingHandles, create_and_start_rpc, start_mining_polling};
use crate::event::{AppEvent, EventHandler};
use crate::keys::DaemonCommand;
use crate::rpc::client::RpcManager;
use crate::rpc::market;

#[tokio::main]
async fn main() -> Result<()> {
    let args = CliArgs::parse();

    // Load daemon config (always, for tab state initialization)
    let daemon_config = DaemonConfig::load().unwrap_or_default();

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
    let app = Arc::new(RwLock::new(App::new(daemon_config.clone())));

    let original_url = args.url.clone();
    let refresh_interval_ms = args.refresh_interval_ms;
    let network = args.network.clone();

    // Daemon lifecycle management channel
    let (daemon_cmd_tx, mut daemon_cmd_rx) = tokio::sync::mpsc::channel::<DaemonCommand>(4);

    let mut daemon_handle: Option<daemon::DaemonHandle> = None;
    let mut log_tail_handle: Option<tokio::task::JoinHandle<()>> = None;
    let mut polling_handles = PollingHandles::new();

    // Start market data polling (every 60 seconds) — independent of node
    market::start_market_polling(app.clone(), Duration::from_secs(60));

    // Determine startup mode:
    // 1. --url provided: connect directly to that node
    // 2. auto_start_daemon (no --url): start integrated daemon, connect to it
    // 3. neither: start with no connection, user starts daemon from Embedded Node tab
    let mut rpc_for_explorer: Arc<RpcManager>;

    if original_url.is_some() {
        // Direct URL mode — connect to specified node
        app.write().await.has_direct_node = true;
        rpc_for_explorer =
            create_and_start_rpc(args.url, &network, &app, refresh_interval_ms, false).await?;
        start_mining_polling(&rpc_for_explorer, &app, &mut polling_handles);
        analytics_streaming::start_analytics_streaming(&rpc_for_explorer, &app, &mut polling_handles);
    } else if daemon_config.auto_start_daemon {
        // Auto-start integrated daemon
        {
            let mut app_guard = app.write().await;
            app_guard.integrated_node.status = DaemonStatus::Starting;
        }
        match daemon_lifecycle::start_daemon_and_connect(
            &daemon_config,
            &app,
            refresh_interval_ms,
            &mut polling_handles,
        )
        .await
        {
            Ok((handle, rpc, log_handle)) => {
                daemon_handle = Some(handle);
                rpc_for_explorer = rpc;
                log_tail_handle = Some(log_handle);
            }
            Err(e) => {
                let mut app_guard = app.write().await;
                app_guard.integrated_node.status = DaemonStatus::Error(e.to_string());
                drop(app_guard);
                rpc_for_explorer =
                    Arc::new(RpcManager::new(None, &network, app.clone()).await?);
            }
        }
    } else {
        // No URL, no auto-start: start disconnected, user starts daemon from tab
        app.write().await.has_direct_node = false;
        rpc_for_explorer =
            Arc::new(RpcManager::new(None, &network, app.clone()).await?);
    }

    // Event loop
    let mut events = EventHandler::new(Duration::from_millis(250));

    loop {
        // Check for daemon commands (non-blocking)
        if let Ok(cmd) = daemon_cmd_rx.try_recv() {
            match cmd {
                DaemonCommand::Start(config) => {
                    // Abort old polling
                    polling_handles.abort_all();
                    let _ = rpc_for_explorer.disconnect().await;

                    match daemon_lifecycle::start_daemon_and_connect(
                        &config,
                        &app,
                        refresh_interval_ms,
                        &mut polling_handles,
                    )
                    .await
                    {
                        Ok((handle, rpc, log_handle)) => {
                            daemon_handle = Some(handle);
                            rpc_for_explorer = rpc;
                            log_tail_handle = Some(log_handle);
                        }
                        Err(e) => {
                            // Shut down daemon if it was started but RPC failed
                            if let Some(mut h) = daemon_handle.take() {
                                h.shutdown();
                            }
                            let mut app_guard = app.write().await;
                            app_guard.integrated_node.status =
                                DaemonStatus::Error(e.to_string());
                        }
                    }
                }
                DaemonCommand::Stop => {
                    // Abort polling
                    polling_handles.abort_all();
                    if let Some(h) = log_tail_handle.take() {
                        h.abort();
                    }
                    let _ = rpc_for_explorer.disconnect().await;

                    // Shutdown daemon
                    if let Some(mut h) = daemon_handle.take() {
                        h.shutdown();
                    }

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
                        app_guard.integrated_node.status = DaemonStatus::Stopped;
                        app_guard.integrated_node.started_at = None;
                        app_guard.has_direct_node = original_url.is_some();
                        if original_url.is_none() {
                            app_guard.node.connection_status =
                                crate::app::ConnectionStatus::Disconnected;
                        }
                    }

                    if let Some(ref url) = original_url {
                        // Restore original direct URL connection
                        match create_and_start_rpc(
                            Some(url.clone()),
                            &network,
                            &app,
                            refresh_interval_ms,
                            false,
                        )
                        .await
                        {
                            Ok(new_rpc) => {
                                rpc_for_explorer = new_rpc;
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
                            Err(_) => {
                                // Best effort — app continues without RPC
                            }
                        }
                    } else {
                        // No original URL — stay disconnected, no PNN fallback
                        rpc_for_explorer = Arc::new(
                            RpcManager::new(None, &network, app.clone()).await?,
                        );
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
                        &daemon_cmd_tx,
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
                // Tick just triggers a draw check — dirty is set by data updates
            }
            AppEvent::Resize(_, _) => {
                app.write().await.dirty = true;
            }
        }
    }

    // Shut down embedded daemon before restoring terminal so user sees status
    if daemon_handle.is_some() {
        // Update status and render one final frame showing "Stopping..."
        {
            let mut app_guard = app.write().await;
            app_guard.integrated_node.status = DaemonStatus::Stopping;
            terminal.draw(|f| ui::draw(f, &app_guard))?;
        }

        // Stop polling and RPC first
        polling_handles.abort_all();
        if let Some(h) = log_tail_handle {
            h.abort();
        }
        drop(rpc_for_explorer);

        // Shut down the daemon — this blocks until the node finishes
        if let Some(mut h) = daemon_handle {
            h.shutdown();
        }
    } else {
        polling_handles.abort_all();
        drop(rpc_for_explorer);
    }

    // Persist analytics cache before exit (best-effort — analytics streaming also saves on exit)
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
