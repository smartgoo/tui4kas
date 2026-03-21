mod app;
mod cli;
mod config;
mod daemon;
mod event;
mod rpc;
mod ui;

use std::io;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use crossterm::ExecutableCommand;
use crossterm::event::{KeyCode, KeyModifiers};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tokio::sync::Mutex;

use crate::app::{App, CommandLine, DagFocus, Tab};
use crate::cli::CliArgs;
use crate::config::DaemonConfig;
use crate::event::{AppEvent, EventHandler};
use crate::rpc::client::RpcManager;
use crate::rpc::market;
use crate::rpc::types::sompi_to_kas;

use crate::app::DaemonStatus;

/// Commands sent from key handlers to the main loop for daemon lifecycle management.
enum DaemonCommand {
    Start(DaemonConfig),
    Stop,
}

/// Tracks cancellable background polling tasks.
struct PollingHandles {
    mining: Option<tokio::task::JoinHandle<()>>,
    analytics: Option<tokio::task::JoinHandle<()>>,
}

impl PollingHandles {
    fn new() -> Self {
        Self {
            mining: None,
            analytics: None,
        }
    }

    fn abort_all(&mut self) {
        if let Some(h) = self.mining.take() {
            h.abort();
        }
        if let Some(h) = self.analytics.take() {
            h.abort();
        }
    }
}

fn start_mining_analytics_polling(
    rpc: &Arc<RpcManager>,
    app: &Arc<Mutex<App>>,
    handles: &mut PollingHandles,
) {
    let rpc_for_mining = rpc.clone();
    let app_for_mining = app.clone();
    handles.mining = Some(tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(5)).await;
        let mut ticker = tokio::time::interval(Duration::from_secs(30));
        loop {
            ticker.tick().await;
            let app_guard = app_for_mining.lock().await;
            let is_synced = app_guard.server_info.as_ref().is_some_and(|s| s.is_synced);
            let is_paused = app_guard.paused;
            drop(app_guard);
            if !is_paused
                && is_synced
                && let Ok(info) = rpc_for_mining.fetch_mining_info().await
            {
                app_for_mining.lock().await.mining_info = Some(info);
            }
        }
    }));

    let rpc_for_analytics = rpc.clone();
    let app_for_analytics = app.clone();
    handles.analytics = Some(tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(8)).await;
        let mut ticker = tokio::time::interval(Duration::from_secs(30));
        loop {
            ticker.tick().await;
            let app_guard = app_for_analytics.lock().await;
            let is_synced = app_guard.server_info.as_ref().is_some_and(|s| s.is_synced);
            let is_paused = app_guard.paused;
            drop(app_guard);
            if !is_paused
                && is_synced
                && let Ok(data) = rpc_for_analytics.fetch_analytics().await
            {
                app_for_analytics.lock().await.analytics = Some(data);
            }
        }
    }));
}

/// Create an RPC manager, connect, and start polling.
/// Connection is attempted first; polling starts only after the connection
/// task is spawned so the first poll has a chance to succeed.
async fn create_and_start_rpc(
    url: Option<String>,
    network: &str,
    app: &Arc<Mutex<App>>,
    refresh_interval_ms: u64,
    retry: bool,
) -> Result<Arc<RpcManager>> {
    let rpc_manager = RpcManager::new(url, network, app.clone()).await?;
    let rpc = Arc::new(rpc_manager);

    let rpc_for_connect = rpc.clone();
    let interval = refresh_interval_ms;
    let app_clone = app.clone();
    tokio::spawn(async move {
        let max_attempts = if retry { 30 } else { 1 };
        for attempt in 0..max_attempts {
            match rpc_for_connect.connect().await {
                Ok(_) => break,
                Err(_) if attempt < max_attempts - 1 => {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                Err(_) => break,
            }
        }
        // Start polling after connection is established (or all attempts exhausted)
        rpc_for_connect.start_polling_shared(Duration::from_millis(interval), app_clone);
    });

    Ok(rpc)
}

fn start_log_tailing(config: &DaemonConfig, app: Arc<Mutex<App>>) -> tokio::task::JoinHandle<()> {
    let log_dir = daemon::log_dir(config);
    tokio::spawn(async move {
        // Brief wait for log file to be created
        tokio::time::sleep(Duration::from_secs(1)).await;
        let mut last_pos: u64 = 0;
        let mut ticker = tokio::time::interval(Duration::from_secs(1));
        loop {
            ticker.tick().await;
            // Find the most recent .log file
            let log_file = match std::fs::read_dir(&log_dir) {
                Ok(entries) => entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().extension().is_some_and(|ext| ext == "log"))
                    .max_by_key(|e| e.metadata().and_then(|m| m.modified()).ok()),
                Err(_) => continue,
            };
            let Some(log_entry) = log_file else {
                continue;
            };
            let path = log_entry.path();
            let Ok(metadata) = std::fs::metadata(&path) else {
                continue;
            };
            let file_len = metadata.len();
            if file_len <= last_pos {
                continue;
            }
            // Read new content
            use std::io::{Read, Seek, SeekFrom};
            let Ok(mut file) = std::fs::File::open(&path) else {
                continue;
            };
            let _ = file.seek(SeekFrom::Start(last_pos));
            let mut buf = String::new();
            let Ok(bytes_read) = file.read_to_string(&mut buf) else {
                continue;
            };
            last_pos += bytes_read as u64;

            if !buf.is_empty() {
                let mut app_guard = app.lock().await;
                for line in buf.lines() {
                    if !line.trim().is_empty() {
                        app_guard.integrated_node.log_lines.push(line.to_string());
                    }
                }
                // Cap at 1000 lines
                let len = app_guard.integrated_node.log_lines.len();
                if len > 1000 {
                    app_guard.integrated_node.log_lines.drain(0..len - 1000);
                }
                // Auto-scroll to bottom if enabled
                if app_guard.integrated_node.log_auto_scroll {
                    let total = app_guard.integrated_node.log_lines.len();
                    app_guard.integrated_node.log_scroll = total;
                }
            }
        }
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = CliArgs::parse();

    // Load daemon config (always, for tab state initialization)
    let daemon_config = DaemonConfig::load().unwrap_or_default();

    // Set up panic hook to restore terminal
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = io::stdout().execute(LeaveAlternateScreen);
        original_hook(panic_info);
    }));

    // Initialize terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create shared app state
    let app = Arc::new(Mutex::new(App::new(daemon_config.clone())));

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
        app.lock().await.has_direct_node = true;
        rpc_for_explorer =
            create_and_start_rpc(args.url, &network, &app, refresh_interval_ms, false).await?;
        start_mining_analytics_polling(&rpc_for_explorer, &app, &mut polling_handles);
    } else if daemon_config.auto_start_daemon {
        // Auto-start integrated daemon
        {
            let mut app_guard = app.lock().await;
            app_guard.integrated_node.status = DaemonStatus::Starting;
        }
        match daemon::start_daemon(&daemon_config) {
            Ok(handle) => {
                daemon_handle = Some(handle);
                tokio::time::sleep(Duration::from_secs(2)).await;

                let daemon_url =
                    daemon::DaemonHandle::wrpc_borsh_url(&daemon_config.network);
                match create_and_start_rpc(
                    Some(daemon_url),
                    &daemon_config.network,
                    &app,
                    refresh_interval_ms,
                    true,
                )
                .await
                {
                    Ok(new_rpc) => {
                        rpc_for_explorer = new_rpc;
                        start_mining_analytics_polling(
                            &rpc_for_explorer,
                            &app,
                            &mut polling_handles,
                        );
                        log_tail_handle =
                            Some(start_log_tailing(&daemon_config, app.clone()));
                        let mut app_guard = app.lock().await;
                        app_guard.integrated_node.status = DaemonStatus::Running;
                        app_guard.integrated_node.started_at =
                            Some(std::time::Instant::now());
                        app_guard.has_direct_node = true;
                    }
                    Err(e) => {
                        if let Some(mut h) = daemon_handle.take() {
                            h.shutdown();
                        }
                        let mut app_guard = app.lock().await;
                        app_guard.integrated_node.status =
                            DaemonStatus::Error(format!("RPC failed: {}", e));
                        // Create a dummy RPC (disconnected) so we have something
                        drop(app_guard);
                        rpc_for_explorer = Arc::new(
                            RpcManager::new(None, &network, app.clone()).await?,
                        );
                    }
                }
            }
            Err(e) => {
                let mut app_guard = app.lock().await;
                app_guard.integrated_node.status = DaemonStatus::Error(e.to_string());
                drop(app_guard);
                // Create a dummy RPC (disconnected) so we have something
                rpc_for_explorer =
                    Arc::new(RpcManager::new(None, &network, app.clone()).await?);
            }
        }
    } else {
        // No URL, no auto-start: start disconnected, user starts daemon from tab
        app.lock().await.has_direct_node = false;
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
                    match daemon::start_daemon(&config) {
                        Ok(handle) => {
                            daemon_handle = Some(handle);

                            // Wait for wRPC server readiness
                            tokio::time::sleep(Duration::from_secs(2)).await;

                            // Abort old polling
                            polling_handles.abort_all();
                            let _ = rpc_for_explorer.disconnect().await;

                            // Create new RPC pointing to daemon
                            let daemon_url = daemon::DaemonHandle::wrpc_borsh_url(&config.network);
                            match create_and_start_rpc(
                                Some(daemon_url),
                                &config.network,
                                &app,
                                refresh_interval_ms,
                                true,
                            )
                            .await
                            {
                                Ok(new_rpc) => {
                                    rpc_for_explorer = new_rpc;
                                    // Start mining/analytics for daemon (direct node)
                                    start_mining_analytics_polling(
                                        &rpc_for_explorer,
                                        &app,
                                        &mut polling_handles,
                                    );
                                    // Start log tailing
                                    log_tail_handle = Some(start_log_tailing(&config, app.clone()));
                                    let mut app_guard = app.lock().await;
                                    app_guard.integrated_node.status = DaemonStatus::Running;
                                    app_guard.integrated_node.started_at =
                                        Some(std::time::Instant::now());
                                    app_guard.has_direct_node = true;
                                }
                                Err(e) => {
                                    // RPC creation failed — shut down daemon
                                    if let Some(mut h) = daemon_handle.take() {
                                        h.shutdown();
                                    }
                                    let mut app_guard = app.lock().await;
                                    app_guard.integrated_node.status =
                                        DaemonStatus::Error(format!("RPC failed: {}", e));
                                }
                            }
                        }
                        Err(e) => {
                            let mut app_guard = app.lock().await;
                            app_guard.integrated_node.status = DaemonStatus::Error(e.to_string());
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
                        let mut app_guard = app.lock().await;
                        app_guard.server_info = None;
                        app_guard.dag_info = None;
                        app_guard.mempool_state = None;
                        app_guard.coin_supply = None;
                        app_guard.fee_estimate = None;
                        app_guard.mining_info = None;
                        app_guard.analytics = None;
                        app_guard.node_url = None;
                        app_guard.node_uid = None;
                        app_guard.integrated_node.status = DaemonStatus::Stopped;
                        app_guard.integrated_node.started_at = None;
                        app_guard.has_direct_node = original_url.is_some();
                        if original_url.is_none() {
                            app_guard.connection_status =
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
                                start_mining_analytics_polling(
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
                            RpcManager::new(None, &network, app.clone())
                                .await
                                .unwrap_or_else(|_| unreachable!()),
                        );
                    }
                }
            }
        }

        // Draw
        {
            let app_guard = app.lock().await;
            terminal.draw(|f| ui::draw(f, &app_guard))?;
        }

        // Handle events
        let Some(event) = events.next().await else {
            break;
        };

        match event {
            AppEvent::Key(key) => {
                let mut app_guard = app.lock().await;

                if app_guard.command_line.active {
                    // Command mode: all keys go to command input
                    match key.code {
                        KeyCode::Esc => {
                            app_guard.command_line.deactivate();
                            app_guard.command_line.show_output = false;
                        }
                        KeyCode::Enter => {
                            if let Some(cmd) = app_guard.command_line.submit() {
                                drop(app_guard);
                                handle_command(&cmd, &app, &rpc_for_explorer).await;
                                continue;
                            }
                        }
                        KeyCode::Backspace => {
                            app_guard.command_line.backspace();
                        }
                        KeyCode::Delete => {
                            app_guard.command_line.delete_char();
                        }
                        KeyCode::Left => {
                            app_guard.command_line.move_left();
                        }
                        KeyCode::Right => {
                            app_guard.command_line.move_right();
                        }
                        KeyCode::Home => {
                            app_guard.command_line.move_home();
                        }
                        KeyCode::End => {
                            app_guard.command_line.move_end();
                        }
                        KeyCode::Up => {
                            app_guard.command_line.history_up();
                        }
                        KeyCode::Down => {
                            app_guard.command_line.history_down();
                        }
                        KeyCode::Char(c) => {
                            app_guard.command_line.insert_char(c);
                        }
                        _ => {}
                    }
                } else {
                    // Normal mode
                    match (key.code, key.modifiers) {
                        (KeyCode::Char('q'), _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                            app_guard.should_quit = true;
                        }
                        (KeyCode::Esc, _) => {
                            app_guard.command_line.show_output = false;
                        }
                        (KeyCode::Char(':'), _) => {
                            app_guard.command_line.activate();
                        }
                        (KeyCode::Char('p'), _) => {
                            app_guard.paused = !app_guard.paused;
                        }
                        (KeyCode::Tab, _) => {
                            app_guard.next_tab();
                        }
                        (KeyCode::BackTab, _) => {
                            app_guard.prev_tab();
                        }
                        _ => match app_guard.active_tab {
                            Tab::RpcExplorer => {
                                handle_rpc_explorer_keys(
                                    &mut app_guard,
                                    key.code,
                                    &rpc_for_explorer,
                                    &app,
                                );
                            }
                            Tab::Mempool => {
                                handle_mempool_keys(&mut app_guard, key.code);
                            }
                            Tab::BlockDag => {
                                handle_blockdag_keys(
                                    &mut app_guard,
                                    key.code,
                                    &rpc_for_explorer,
                                    &app,
                                );
                            }
                            Tab::IntegratedNode => {
                                handle_integrated_node_keys(
                                    &mut app_guard,
                                    key.code,
                                    &daemon_cmd_tx,
                                );
                            }
                            _ => {}
                        },
                    }
                }

                if app_guard.should_quit {
                    break;
                }
            }
            AppEvent::Tick | AppEvent::Resize(_, _) => {}
        }
    }

    // Shut down embedded daemon before restoring terminal so user sees status
    if daemon_handle.is_some() {
        // Update status and render one final frame showing "Stopping..."
        {
            let mut app_guard = app.lock().await;
            app_guard.integrated_node.status = DaemonStatus::Stopping;
        }
        {
            let app_guard = app.lock().await;
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

    // Restore terminal
    disable_raw_mode()?;
    terminal.backend_mut().execute(LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}

fn handle_integrated_node_keys(
    app: &mut App,
    key: KeyCode,
    daemon_tx: &tokio::sync::mpsc::Sender<DaemonCommand>,
) {
    let state = &mut app.integrated_node;

    if state.is_running() {
        // Running mode
        match key {
            KeyCode::Enter => {
                if matches!(state.status, DaemonStatus::Running)
                    && daemon_tx.try_send(DaemonCommand::Stop).is_ok()
                {
                    state.status = DaemonStatus::Stopping;
                }
            }
            KeyCode::Char('j') => {
                let max = state.log_lines.len();
                state.log_scroll = state.log_scroll.saturating_add(1).min(max);
                state.log_auto_scroll = state.log_scroll >= max;
            }
            KeyCode::Char('k') => {
                state.log_scroll = state.log_scroll.saturating_sub(1);
                state.log_auto_scroll = false;
            }
            KeyCode::Char('J') => {
                let max = state.log_lines.len();
                state.log_scroll = state.log_scroll.saturating_add(10).min(max);
                state.log_auto_scroll = state.log_scroll >= max;
            }
            KeyCode::Char('K') => {
                state.log_scroll = state.log_scroll.saturating_sub(10);
                state.log_auto_scroll = false;
            }
            KeyCode::Home => {
                state.log_scroll = 0;
                state.log_auto_scroll = false;
            }
            KeyCode::End => {
                state.log_scroll = state.log_lines.len();
                state.log_auto_scroll = true;
            }
            _ => {}
        }
    } else if state.editing {
        // Field editing mode
        match key {
            KeyCode::Esc => {
                state.editing = false;
                state.edit_buffer.clear();
            }
            KeyCode::Enter => {
                let val = state.edit_buffer.clone();
                state.editing = false;
                state.edit_buffer.clear();
                apply_field_edit(&mut state.config, state.selected_field, &val);
                auto_save_config(state);
            }
            KeyCode::Backspace => {
                state.edit_buffer.pop();
            }
            KeyCode::Char(c) => {
                state.edit_buffer.push(c);
            }
            _ => {}
        }
    } else {
        // Settings navigation mode
        state.status_message = None; // Clear transient messages on any navigation
        match key {
            KeyCode::Up => {
                state.selected_field = state.selected_field.saturating_sub(1);
            }
            KeyCode::Down => {
                let max = app::IntegratedNodeState::field_count() - 1;
                if state.selected_field < max {
                    state.selected_field += 1;
                }
            }
            KeyCode::Enter => {
                match state.selected_field {
                    // Toggle bool fields
                    1 => {
                        state.config.utxo_index = !state.config.utxo_index;
                        auto_save_config(state);
                    }
                    6 => {
                        state.config.auto_start_daemon = !state.config.auto_start_daemon;
                        auto_save_config(state);
                    }
                    // Cycle enum fields
                    0 => {
                        state.config.cycle_network();
                        auto_save_config(state);
                    }
                    4 => {
                        state.config.cycle_log_level();
                        auto_save_config(state);
                    }
                    // Start daemon action
                    7 => {
                        if matches!(state.status, DaemonStatus::Stopped | DaemonStatus::Error(_)) {
                            state.log_lines.clear();
                            state.log_scroll = 0;
                            state.log_auto_scroll = true;
                            state.status_message = None;
                            match daemon_tx.try_send(DaemonCommand::Start(state.config.clone())) {
                                Ok(()) => {
                                    state.status = DaemonStatus::Starting;
                                }
                                Err(_) => {
                                    state.status_message =
                                        Some(("Command channel full, try again".to_string(), true));
                                }
                            }
                        }
                    }
                    // Editable fields
                    _ => {
                        state.editing = true;
                        state.edit_buffer = match state.selected_field {
                            2 => format!("{:.1}", state.config.ram_scale),
                            3 => state.config.app_dir.clone(),
                            5 => state.config.async_threads.to_string(),
                            _ => String::new(),
                        };
                    }
                }
            }
            KeyCode::Left | KeyCode::Right => match state.selected_field {
                0 => {
                    state.config.cycle_network();
                    auto_save_config(state);
                }
                4 => {
                    state.config.cycle_log_level();
                    auto_save_config(state);
                }
                _ => {}
            },
            KeyCode::Char('r') => match DaemonConfig::load() {
                Ok(c) => {
                    state.config = c;
                    state.status_message = Some(("Config reloaded".to_string(), false));
                }
                Err(e) => {
                    state.status_message = Some((format!("Load failed: {}", e), true));
                }
            },
            _ => {}
        }
    }
}

fn auto_save_config(state: &mut app::IntegratedNodeState) {
    if let Err(e) = state.config.save() {
        state.status_message = Some((format!("Auto-save failed: {}", e), true));
    }
}

fn apply_field_edit(config: &mut DaemonConfig, field: usize, val: &str) {
    match field {
        2 => {
            if let Ok(v) = val.parse::<f64>()
                && v > 0.0
            {
                config.ram_scale = v;
            }
        }
        3 => {
            let trimmed = val.trim();
            if !trimmed.is_empty() {
                config.app_dir = trimmed.to_string();
            }
        }
        5 => {
            if let Ok(v) = val.parse::<usize>()
                && v > 0
            {
                config.async_threads = v;
            }
        }
        _ => {}
    }
}

async fn handle_command(cmd: &str, app: &Arc<Mutex<App>>, rpc: &Arc<RpcManager>) {
    let parts: Vec<&str> = cmd.trim().splitn(2, ' ').collect();
    let command = parts[0];

    match command {
        "help" => {
            let mut help_text = String::from("Available commands:\n\n");
            for (name, desc) in CommandLine::available_commands() {
                help_text.push_str(&format!("  {:<28} {}\n", name, desc));
            }
            help_text
                .push_str("\nPress ':' to open command line, Esc to close, Up/Down for history");
            let mut app_guard = app.lock().await;
            app_guard
                .command_line
                .push_output(cmd.to_string(), help_text, false);
        }
        "clear" => {
            let mut app_guard = app.lock().await;
            app_guard.command_line.output.clear();
            app_guard.command_line.show_output = false;
        }
        _ => {
            // Try as RPC call
            match rpc.execute_rpc_call(command).await {
                Ok(response) => {
                    let mut app_guard = app.lock().await;
                    app_guard
                        .command_line
                        .push_output(cmd.to_string(), response, false);
                }
                Err(e) => {
                    let mut app_guard = app.lock().await;
                    app_guard
                        .command_line
                        .push_output(cmd.to_string(), e.to_string(), true);
                }
            }
        }
    }
}

fn handle_rpc_explorer_keys(
    app: &mut App,
    key: KeyCode,
    rpc: &Arc<RpcManager>,
    app_state: &Arc<Mutex<App>>,
) {
    match key {
        KeyCode::Up => {
            if app.rpc_explorer.selected_method > 0 {
                app.rpc_explorer.selected_method -= 1;
            }
        }
        KeyCode::Down => {
            let len = app.rpc_explorer.available_methods.len();
            if len > 0 && app.rpc_explorer.selected_method < len - 1 {
                app.rpc_explorer.selected_method += 1;
            }
        }
        KeyCode::Enter => {
            let method = app.rpc_explorer.available_methods[app.rpc_explorer.selected_method];
            app.rpc_explorer.is_loading = true;
            app.rpc_explorer.scroll_offset = 0;

            let method = method.to_string();
            let rpc = rpc.clone();
            let state = app_state.clone();
            tokio::spawn(async move {
                let result = match rpc.execute_rpc_call(&method).await {
                    Ok(response) => response,
                    Err(e) => format!("Error: {}", e),
                };
                let mut app_guard = state.lock().await;
                app_guard.rpc_explorer.last_response = Some(result);
                app_guard.rpc_explorer.is_loading = false;
            });
        }
        KeyCode::Char('j') | KeyCode::Char('J') => {
            let step = if key == KeyCode::Char('J') { 10 } else { 1 };
            app.rpc_explorer.scroll_offset = app.rpc_explorer.scroll_offset.saturating_add(step);
        }
        KeyCode::Char('k') | KeyCode::Char('K') => {
            let step = if key == KeyCode::Char('K') { 10 } else { 1 };
            app.rpc_explorer.scroll_offset = app.rpc_explorer.scroll_offset.saturating_sub(step);
        }
        KeyCode::PageDown => {
            app.rpc_explorer.scroll_offset = app.rpc_explorer.scroll_offset.saturating_add(20);
        }
        KeyCode::PageUp => {
            app.rpc_explorer.scroll_offset = app.rpc_explorer.scroll_offset.saturating_sub(20);
        }
        KeyCode::Home => {
            app.rpc_explorer.scroll_offset = 0;
        }
        _ => {}
    }
}

fn handle_mempool_keys(app: &mut App, key: KeyCode) {
    if app.mempool_detail.is_some() {
        if key == KeyCode::Esc {
            app.mempool_detail = None;
        }
        return;
    }

    match key {
        KeyCode::Up => {
            app.mempool_selected = app.mempool_selected.saturating_sub(1);
        }
        KeyCode::Down => {
            if let Some(ref mempool) = app.mempool_state
                && app.mempool_selected < mempool.entries.len().saturating_sub(1)
            {
                app.mempool_selected += 1;
            }
        }
        KeyCode::Enter => {
            if let Some(ref mempool) = app.mempool_state
                && app.mempool_selected < mempool.entries.len()
            {
                let entry = &mempool.entries[app.mempool_selected];
                let detail = format!(
                    "Transaction ID: {}\nFee: {:.8} KAS ({} sompi)\nOrphan: {}",
                    entry.transaction_id,
                    sompi_to_kas(entry.fee),
                    entry.fee,
                    if entry.is_orphan { "Yes" } else { "No" },
                );
                app.mempool_detail = Some(detail);
            }
        }
        _ => {}
    }
}

fn handle_blockdag_keys(
    app: &mut App,
    key: KeyCode,
    rpc: &Arc<RpcManager>,
    app_state: &Arc<Mutex<App>>,
) {
    if app.dag_block_detail.is_some() {
        if key == KeyCode::Esc {
            app.dag_block_detail = None;
        }
        return;
    }

    match key {
        KeyCode::Left | KeyCode::Right => {
            app.dag_focus = match app.dag_focus {
                DagFocus::Tips => DagFocus::Parents,
                DagFocus::Parents => DagFocus::Tips,
            };
        }
        KeyCode::Up => match app.dag_focus {
            DagFocus::Tips => {
                app.dag_tip_selected = app.dag_tip_selected.saturating_sub(1);
            }
            DagFocus::Parents => {
                app.dag_parent_selected = app.dag_parent_selected.saturating_sub(1);
            }
        },
        KeyCode::Down => {
            if let Some(ref dag) = app.dag_info {
                match app.dag_focus {
                    DagFocus::Tips => {
                        if app.dag_tip_selected < dag.tip_hashes.len().saturating_sub(1) {
                            app.dag_tip_selected += 1;
                        }
                    }
                    DagFocus::Parents => {
                        if app.dag_parent_selected
                            < dag.virtual_parent_hashes.len().saturating_sub(1)
                        {
                            app.dag_parent_selected += 1;
                        }
                    }
                }
            }
        }
        KeyCode::Enter => {
            let hash = if let Some(ref dag) = app.dag_info {
                match app.dag_focus {
                    DagFocus::Tips => dag.tip_hashes.get(app.dag_tip_selected).cloned(),
                    DagFocus::Parents => dag
                        .virtual_parent_hashes
                        .get(app.dag_parent_selected)
                        .cloned(),
                }
            } else {
                None
            };

            if let Some(hash) = hash {
                app.dag_block_loading = true;
                let rpc = rpc.clone();
                let state = app_state.clone();
                tokio::spawn(async move {
                    let result = match rpc.get_block_by_hash(&hash).await {
                        Ok(info) => info,
                        Err(e) => format!("Error: {}", e),
                    };
                    let mut app_guard = state.lock().await;
                    app_guard.dag_block_detail = Some(result);
                    app_guard.dag_block_loading = false;
                });
            }
        }
        _ => {}
    }
}
