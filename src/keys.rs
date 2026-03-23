use std::io::Write;
use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::Engine;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use tokio::sync::RwLock;

use crate::app::{App, CommandLine, DagFocus, MiningPanel, SettingsState, Tab};
use crate::config::AppConfig;
use crate::rpc::client::RpcManager;
use crate::rpc::types::sompi_to_kas;

/// Base URL for the Kaspa block explorer.
const EXPLORER_BASE: &str = "https://kaspa.stream";

/// Double-click detection threshold.
const DOUBLE_CLICK_MS: u64 = 400;

/// Copy text to terminal clipboard via OSC 52 escape sequence.
fn copy_to_clipboard(text: &str) {
    let encoded = base64::engine::general_purpose::STANDARD.encode(text);
    let _ = write!(std::io::stdout(), "\x1b]52;c;{}\x07", encoded);
    let _ = std::io::stdout().flush();
}

/// Get the text of the currently focused/selected element for clipboard copy.
fn get_focused_text(app: &App) -> Option<String> {
    match app.active_tab {
        Tab::Mempool => {
            let mempool = app.node.mempool_state.as_ref()?;
            let entry = mempool.entries.get(app.mempool_selected)?;
            Some(entry.transaction_id.clone())
        }
        Tab::BlockDag => get_selected_dag_hash(app),
        Tab::Mining => {
            let mining = app.node.mining_info.as_ref()?;
            let (data, selected) = match app.mining_tab.active_panel {
                MiningPanel::Miners => (&mining.all_miners, app.mining_tab.miners_selected),
                MiningPanel::Pools => (&mining.pools, app.mining_tab.pools_selected),
                MiningPanel::Versions => (&mining.node_versions, app.mining_tab.versions_selected),
            };
            data.get(selected).map(|(name, _)| name.clone())
        }
        Tab::RpcExplorer => app
            .rpc_explorer
            .available_methods
            .get(app.rpc_explorer.selected_method)
            .map(|m| m.to_string()),
        _ => None,
    }
}

/// Commands sent from key handlers to the main loop for settings/reconnection.
pub enum SettingsCommand {
    Reconnect(AppConfig),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsField {
    Url,
    Network,
    RefreshInterval,
    AnalysisStart,
}

impl SettingsField {
    pub const COUNT: usize = 4;

    pub fn from_index(i: usize) -> Option<Self> {
        [
            SettingsField::Url,
            SettingsField::Network,
            SettingsField::RefreshInterval,
            SettingsField::AnalysisStart,
        ]
        .get(i)
        .copied()
    }
}

/// Handle command-mode key input. Returns `Some(cmd)` if a command was submitted
/// (caller must drop the app guard and dispatch the command).
pub fn handle_command_mode_keys(app: &mut App, code: KeyCode) -> Option<String> {
    match code {
        KeyCode::Esc => {
            app.command_line.deactivate();
            app.command_line.show_output = false;
            None
        }
        KeyCode::Enter => app.command_line.submit(),
        KeyCode::Backspace => {
            app.command_line.backspace();
            None
        }
        KeyCode::Delete => {
            app.command_line.delete_char();
            None
        }
        KeyCode::Left => {
            app.command_line.move_left();
            None
        }
        KeyCode::Right => {
            app.command_line.move_right();
            None
        }
        KeyCode::Home => {
            app.command_line.move_home();
            None
        }
        KeyCode::End => {
            app.command_line.move_end();
            None
        }
        KeyCode::Up => {
            app.command_line.history_up();
            None
        }
        KeyCode::Down => {
            app.command_line.history_down();
            None
        }
        KeyCode::Char(c) => {
            app.command_line.insert_char(c);
            None
        }
        _ => None,
    }
}

/// Handle normal-mode key input (not command mode). Returns `true` if the event
/// was consumed by an overlay (help, command output) and the caller should `continue`.
pub fn handle_normal_keys(
    app: &mut App,
    key: KeyEvent,
    rpc: &Arc<RpcManager>,
    app_state: &Arc<RwLock<App>>,
    settings_tx: &tokio::sync::mpsc::Sender<SettingsCommand>,
) -> bool {
    // Reset quit confirmation on any key that isn't 'q'
    if key.code != KeyCode::Char('q') {
        app.quit_confirm = false;
    }

    // Help overlay takes priority
    if app.show_help {
        match key.code {
            KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') => {
                app.show_help = false;
            }
            _ => {}
        }
        return true;
    }

    // Command output overlay: intercept scroll keys
    if app.command_line.show_output && !app.command_line.output.is_empty() {
        match key.code {
            KeyCode::Esc => {
                app.command_line.show_output = false;
            }
            KeyCode::Char('j') => {
                app.command_line.output_scroll = app.command_line.output_scroll.saturating_add(1);
            }
            KeyCode::Char('k') => {
                app.command_line.output_scroll = app.command_line.output_scroll.saturating_sub(1);
            }
            KeyCode::Char('J') => {
                app.command_line.output_scroll = app.command_line.output_scroll.saturating_add(10);
            }
            KeyCode::Char('K') => {
                app.command_line.output_scroll = app.command_line.output_scroll.saturating_sub(10);
            }
            KeyCode::Char('g') => {
                app.command_line.output_scroll = 0;
            }
            KeyCode::Char('G') => {
                app.command_line.output_scroll = usize::MAX;
            }
            _ => {}
        }
        return true;
    }

    // When editing a settings field, route all input there first
    if app.active_tab == Tab::Settings && app.settings.editing {
        handle_settings_keys(app, key.code, settings_tx);
        return false;
    }

    // Normal mode
    match (key.code, key.modifiers) {
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
            app.should_quit = true;
        }
        (KeyCode::Char('q'), _) => {
            if app.quit_confirm {
                app.should_quit = true;
            } else {
                app.quit_confirm = true;
            }
        }
        (KeyCode::Esc, _) => {
            app.command_line.show_output = false;
            // Dispatch Esc to tab-specific handlers for popup closing
            match app.active_tab {
                Tab::Mempool => handle_mempool_keys(app, key.code),
                Tab::BlockDag => handle_blockdag_keys(app, key.code, rpc, app_state),
                Tab::Analytics => handle_analytics_keys(app, key.code),
                _ => {}
            }
        }
        (KeyCode::Char('?'), _) => {
            app.show_help = true;
        }
        (KeyCode::Char(':'), _) => {
            app.command_line.activate();
        }
        (KeyCode::Char('p'), _) => {
            app.paused = !app.paused;
        }
        (KeyCode::Char('c'), _) => {
            if let Some(text) = get_focused_text(app) {
                copy_to_clipboard(&text);
                app.clipboard_flash = Some(format!("Copied: {}", text));
            }
        }
        (KeyCode::Tab, _) => {
            app.next_tab();
        }
        (KeyCode::BackTab, _) => {
            app.prev_tab();
        }
        (KeyCode::Char('1'), _) => {
            app.active_tab = Tab::Dashboard;
        }
        (KeyCode::Char('2'), _) => {
            app.active_tab = Tab::Mining;
        }
        (KeyCode::Char('3'), _) => {
            app.active_tab = Tab::Mempool;
        }
        (KeyCode::Char('4'), _) => {
            app.active_tab = Tab::BlockDag;
        }
        (KeyCode::Char('5'), _) => {
            app.active_tab = Tab::Analytics;
        }
        (KeyCode::Char('6'), _) => {
            app.active_tab = Tab::RpcExplorer;
        }
        (KeyCode::Char('7'), _) => {
            app.active_tab = Tab::Settings;
        }
        _ => match app.active_tab {
            Tab::Mining => handle_mining_keys(app, key.code),
            Tab::RpcExplorer => handle_rpc_explorer_keys(app, key.code, rpc, app_state),
            Tab::Mempool => handle_mempool_keys(app, key.code),
            Tab::BlockDag => handle_blockdag_keys(app, key.code, rpc, app_state),
            Tab::Analytics => handle_analytics_keys(app, key.code),
            Tab::Settings => handle_settings_keys(app, key.code, settings_tx),
            _ => {}
        },
    }

    false
}

/// Handle mouse events.
pub fn handle_mouse(app: &mut App, mouse: MouseEvent) -> bool {
    // Dismiss help on any click
    if app.show_help {
        if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
            app.show_help = false;
        }
        return true;
    }

    match mouse.kind {
        MouseEventKind::ScrollDown => match app.active_tab {
            Tab::Mempool if app.mempool_detail.is_none() => {
                let max = app
                    .node
                    .mempool_state
                    .as_ref()
                    .map(|m| m.entries.len().saturating_sub(1))
                    .unwrap_or(0);
                if app.mempool_selected < max {
                    app.mempool_selected += 1;
                }
            }
            Tab::BlockDag if app.dag_selection.block_detail.is_none() => {
                if let Some(ref dag) = app.node.dag_info {
                    match app.dag_selection.focus {
                        DagFocus::Tips => {
                            if app.dag_selection.tip_selected
                                < dag.tip_hashes.len().saturating_sub(1)
                            {
                                app.dag_selection.tip_selected += 1;
                            }
                        }
                        DagFocus::Parents => {
                            if app.dag_selection.parent_selected
                                < dag.virtual_parent_hashes.len().saturating_sub(1)
                            {
                                app.dag_selection.parent_selected += 1;
                            }
                        }
                    }
                }
            }
            Tab::Mining => {
                let max = mining_panel_len(app).saturating_sub(1);
                let sel = app.mining_tab.selected_mut();
                *sel = (*sel + 3).min(max);
            }
            Tab::RpcExplorer => {
                app.rpc_explorer.scroll_offset = app.rpc_explorer.scroll_offset.saturating_add(3);
            }
            _ => {}
        },
        MouseEventKind::ScrollUp => match app.active_tab {
            Tab::Mempool if app.mempool_detail.is_none() => {
                app.mempool_selected = app.mempool_selected.saturating_sub(1);
            }
            Tab::BlockDag if app.dag_selection.block_detail.is_none() => {
                match app.dag_selection.focus {
                    DagFocus::Tips => {
                        app.dag_selection.tip_selected =
                            app.dag_selection.tip_selected.saturating_sub(1);
                    }
                    DagFocus::Parents => {
                        app.dag_selection.parent_selected =
                            app.dag_selection.parent_selected.saturating_sub(1);
                    }
                }
            }
            Tab::Mining => {
                let sel = app.mining_tab.selected_mut();
                *sel = sel.saturating_sub(3);
            }
            Tab::RpcExplorer => {
                app.rpc_explorer.scroll_offset = app.rpc_explorer.scroll_offset.saturating_sub(3);
            }
            _ => {}
        },
        MouseEventKind::Down(MouseButton::Left) => {
            let now = Instant::now();
            let is_double_click = app.last_click.is_some_and(|prev| {
                now.duration_since(prev) < Duration::from_millis(DOUBLE_CLICK_MS)
            });
            app.last_click = Some(now);

            if is_double_click {
                // Double-click: copy focused text to clipboard
                if let Some(text) = get_focused_text(app) {
                    copy_to_clipboard(&text);
                    app.clipboard_flash = Some(format!("Copied: {}", text));
                }
            } else if mouse.row < 3 {
                let tabs = Tab::all();
                let mut x_pos: u16 = 2;
                for tab in tabs {
                    let title_len = tab.title().len() as u16;
                    if mouse.column >= x_pos && mouse.column < x_pos + title_len {
                        app.active_tab = *tab;
                        break;
                    }
                    x_pos += title_len + 3;
                }
            }
        }
        _ => {}
    }

    false
}

pub fn handle_mining_keys(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Left | KeyCode::Char('h') => {
            app.mining_tab.active_panel = app.mining_tab.active_panel.prev();
        }
        KeyCode::Right | KeyCode::Char('l') => {
            app.mining_tab.active_panel = app.mining_tab.active_panel.next();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            let scroll = app.mining_tab.selected_mut();
            *scroll = scroll.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let max = mining_panel_len(app).saturating_sub(1);
            let sel = app.mining_tab.selected_mut();
            if *sel < max {
                *sel += 1;
            }
        }
        KeyCode::Home | KeyCode::Char('g') => {
            *app.mining_tab.selected_mut() = 0;
        }
        KeyCode::End | KeyCode::Char('G') => {
            *app.mining_tab.selected_mut() = mining_panel_len(app).saturating_sub(1);
        }
        KeyCode::Char('o') => {
            if let Some(ref mining) = app.node.mining_info {
                let (data, selected) = match app.mining_tab.active_panel {
                    MiningPanel::Miners => (&mining.all_miners, app.mining_tab.miners_selected),
                    MiningPanel::Pools => (&mining.pools, app.mining_tab.pools_selected),
                    MiningPanel::Versions => {
                        (&mining.node_versions, app.mining_tab.versions_selected)
                    }
                };
                if let Some((name, _)) = data.get(selected) {
                    // Only open addresses (kaspa:...) in explorer
                    if name.starts_with("kaspa:") {
                        let _ = open::that(format!("{}/address/{}", EXPLORER_BASE, name));
                    }
                }
            }
        }
        _ => {}
    }
}

fn mining_panel_len(app: &App) -> usize {
    app.node
        .mining_info
        .as_ref()
        .map(|m| match app.mining_tab.active_panel {
            MiningPanel::Miners => m.all_miners.len(),
            MiningPanel::Pools => m.pools.len(),
            MiningPanel::Versions => m.node_versions.len(),
        })
        .unwrap_or(0)
}

pub fn handle_rpc_explorer_keys(
    app: &mut App,
    key: KeyCode,
    rpc: &Arc<RpcManager>,
    app_state: &Arc<RwLock<App>>,
) {
    if app.rpc_explorer.available_methods.is_empty() {
        return;
    }

    match key {
        KeyCode::Up if app.rpc_explorer.selected_method > 0 => {
            app.rpc_explorer.selected_method -= 1;
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
                let mut app_guard = state.write().await;
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
        KeyCode::Home | KeyCode::Char('g') => {
            app.rpc_explorer.scroll_offset = 0;
        }
        KeyCode::End | KeyCode::Char('G') => {
            app.rpc_explorer.scroll_offset = usize::MAX;
        }
        _ => {}
    }
}

pub fn handle_mempool_keys(app: &mut App, key: KeyCode) {
    if app.mempool_detail.is_some() {
        if key == KeyCode::Esc {
            app.mempool_detail = None;
        }
        return;
    }

    let entry_count = app
        .node
        .mempool_state
        .as_ref()
        .map(|m| m.entries.len())
        .unwrap_or(0);

    match key {
        KeyCode::Up | KeyCode::Char('k') => {
            app.mempool_selected = app.mempool_selected.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j')
            if entry_count > 0 && app.mempool_selected < entry_count.saturating_sub(1) =>
        {
            app.mempool_selected += 1;
        }
        KeyCode::Home | KeyCode::Char('g') => {
            app.mempool_selected = 0;
        }
        KeyCode::End | KeyCode::Char('G') if entry_count > 0 => {
            app.mempool_selected = entry_count - 1;
        }
        KeyCode::Char('o') => {
            if let Some(ref mempool) = app.node.mempool_state
                && app.mempool_selected < mempool.entries.len()
            {
                let txid = &mempool.entries[app.mempool_selected].transaction_id;
                let _ = open::that(format!("{}/tx/{}", EXPLORER_BASE, txid));
            }
        }
        KeyCode::Enter => {
            if let Some(ref mempool) = app.node.mempool_state
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

pub fn handle_blockdag_keys(
    app: &mut App,
    key: KeyCode,
    rpc: &Arc<RpcManager>,
    app_state: &Arc<RwLock<App>>,
) {
    if app.dag_selection.block_detail.is_some() {
        if key == KeyCode::Esc {
            app.dag_selection.block_detail = None;
        }
        return;
    }

    match key {
        KeyCode::Left | KeyCode::Right | KeyCode::Char('h') | KeyCode::Char('l') => {
            app.dag_selection.focus = match app.dag_selection.focus {
                DagFocus::Tips => DagFocus::Parents,
                DagFocus::Parents => DagFocus::Tips,
            };
        }
        KeyCode::Up | KeyCode::Char('k') => match app.dag_selection.focus {
            DagFocus::Tips => {
                app.dag_selection.tip_selected = app.dag_selection.tip_selected.saturating_sub(1);
            }
            DagFocus::Parents => {
                app.dag_selection.parent_selected =
                    app.dag_selection.parent_selected.saturating_sub(1);
            }
        },
        KeyCode::Down | KeyCode::Char('j') => {
            if let Some(ref dag) = app.node.dag_info {
                match app.dag_selection.focus {
                    DagFocus::Tips => {
                        if app.dag_selection.tip_selected < dag.tip_hashes.len().saturating_sub(1) {
                            app.dag_selection.tip_selected += 1;
                        }
                    }
                    DagFocus::Parents => {
                        if app.dag_selection.parent_selected
                            < dag.virtual_parent_hashes.len().saturating_sub(1)
                        {
                            app.dag_selection.parent_selected += 1;
                        }
                    }
                }
            }
        }
        KeyCode::Home | KeyCode::Char('g') => match app.dag_selection.focus {
            DagFocus::Tips => {
                app.dag_selection.tip_selected = 0;
            }
            DagFocus::Parents => {
                app.dag_selection.parent_selected = 0;
            }
        },
        KeyCode::End | KeyCode::Char('G') => {
            if let Some(ref dag) = app.node.dag_info {
                match app.dag_selection.focus {
                    DagFocus::Tips => {
                        app.dag_selection.tip_selected = dag.tip_hashes.len().saturating_sub(1);
                    }
                    DagFocus::Parents => {
                        app.dag_selection.parent_selected =
                            dag.virtual_parent_hashes.len().saturating_sub(1);
                    }
                }
            }
        }
        KeyCode::Char('o') => {
            if let Some(hash) = get_selected_dag_hash(app) {
                let _ = open::that(format!("{}/block/{}", EXPLORER_BASE, hash));
            }
        }
        KeyCode::Enter => {
            let hash = get_selected_dag_hash(app);

            if let Some(hash) = hash {
                app.dag_selection.block_loading = true;
                let rpc = rpc.clone();
                let state = app_state.clone();
                tokio::spawn(async move {
                    let result = match rpc.get_block_by_hash(&hash).await {
                        Ok(info) => info,
                        Err(e) => format!("Error: {}", e),
                    };
                    let mut app_guard = state.write().await;
                    app_guard.dag_selection.block_detail = Some(result);
                    app_guard.dag_selection.block_loading = false;
                });
            }
        }
        _ => {}
    }
}

fn get_selected_dag_hash(app: &App) -> Option<String> {
    let dag = app.node.dag_info.as_ref()?;
    match app.dag_selection.focus {
        DagFocus::Tips => dag.tip_hashes.get(app.dag_selection.tip_selected).cloned(),
        DagFocus::Parents => dag
            .virtual_parent_hashes
            .get(app.dag_selection.parent_selected)
            .cloned(),
    }
}

pub fn handle_analytics_keys(app: &mut App, key: KeyCode) {
    // Dismiss reorg notification on Esc
    if app.analytics.reorg_notification.is_some() && key == KeyCode::Esc {
        app.analytics.reorg_notification = None;
        return;
    }

    match key {
        // Panel navigation
        // Grid:  0  1  2
        //        3     4
        //        5  5  5
        KeyCode::Left | KeyCode::Char('h') => match app.analytics.focus {
            1 => app.analytics.focus = 0,
            2 => app.analytics.focus = 1,
            4 => app.analytics.focus = 3,
            _ => {}
        },
        KeyCode::Right | KeyCode::Char('l') => match app.analytics.focus {
            0 => app.analytics.focus = 1,
            1 => app.analytics.focus = 2,
            3 => app.analytics.focus = 4,
            _ => {}
        },
        KeyCode::Up | KeyCode::Char('k') => match app.analytics.focus {
            3 => app.analytics.focus = 0,
            4 => app.analytics.focus = 2,
            5 => app.analytics.focus = 3,
            _ => {}
        },
        KeyCode::Down | KeyCode::Char('j') => match app.analytics.focus {
            0 | 1 => app.analytics.focus = 3,
            2 => app.analytics.focus = 4,
            3 | 4 => app.analytics.focus = 5,
            _ => {}
        },
        // Toggle view mode for focused panel
        KeyCode::Char('v') => {
            let focus = app.analytics.focus;
            if let Some(mode) = app.analytics.view_modes.get_mut(focus) {
                mode.toggle();
            }
        }
        // Cycle time window for focused panel
        KeyCode::Char('t') => {
            let focus = app.analytics.focus;
            if let Some(tw) = app.analytics.time_windows.get_mut(focus) {
                tw.cycle();
                // Refresh the cached view for this panel immediately
                if let Some(ref engine) = app.analytics.engine
                    && let Ok(eng) = engine.try_read()
                {
                    let new_view = eng.get_view(app.analytics.time_windows[focus]);
                    if let Some(ref mut views) = app.analytics.cached_views {
                        views[focus] = new_view;
                    }
                }
            }
        }
        _ => {}
    }
}

pub fn handle_settings_keys(
    app: &mut App,
    key: KeyCode,
    settings_tx: &tokio::sync::mpsc::Sender<SettingsCommand>,
) {
    let state = &mut app.settings;

    if state.editing {
        match key {
            KeyCode::Esc => {
                state.editing = false;
                state.edit_buffer.clear();
            }
            KeyCode::Enter => {
                let val = state.edit_buffer.clone();
                state.editing = false;
                state.edit_buffer.clear();
                if let Some(field) = SettingsField::from_index(state.selected_field)
                    && apply_field_edit(state, field, &val)
                {
                    auto_save_and_reconnect(state, settings_tx);
                }
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
        state.status_message = None;
        match key {
            KeyCode::Up => {
                state.selected_field = state.selected_field.saturating_sub(1);
            }
            KeyCode::Down => {
                let max = SettingsField::COUNT - 1;
                if state.selected_field < max {
                    state.selected_field += 1;
                }
            }
            KeyCode::Enter => {
                if let Some(field) = SettingsField::from_index(state.selected_field) {
                    match field {
                        SettingsField::Network => {
                            state.config.cycle_network();
                            auto_save_and_reconnect(state, settings_tx);
                        }
                        SettingsField::AnalysisStart => {
                            state.config.analyze_from_pruning_point =
                                !state.config.analyze_from_pruning_point;
                            auto_save_and_reconnect(state, settings_tx);
                        }
                        SettingsField::Url | SettingsField::RefreshInterval => {
                            state.editing = true;
                            state.edit_buffer = get_field_value(&state.config, field);
                        }
                    }
                }
            }
            KeyCode::Left | KeyCode::Right => {
                if let Some(field) = SettingsField::from_index(state.selected_field) {
                    match field {
                        SettingsField::Network => {
                            state.config.cycle_network();
                            auto_save_and_reconnect(state, settings_tx);
                        }
                        SettingsField::AnalysisStart => {
                            state.config.analyze_from_pruning_point =
                                !state.config.analyze_from_pruning_point;
                            auto_save_and_reconnect(state, settings_tx);
                        }
                        _ => {}
                    }
                }
            }
            KeyCode::Char('r') => match AppConfig::load() {
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

fn auto_save_and_reconnect(
    state: &mut SettingsState,
    settings_tx: &tokio::sync::mpsc::Sender<SettingsCommand>,
) {
    if let Err(e) = state.config.save() {
        state.status_message = Some((format!("Save failed: {}", e), true));
        return;
    }
    match settings_tx.try_send(SettingsCommand::Reconnect(state.config.clone())) {
        Ok(()) => {
            state.status_message = Some(("Saved & reconnecting...".to_string(), false));
        }
        Err(_) => {
            state.status_message = Some(("Saved (reconnect pending)".to_string(), false));
        }
    }
}

fn get_field_value(config: &AppConfig, field: SettingsField) -> String {
    match field {
        SettingsField::Url => config.url.clone().unwrap_or_default(),
        SettingsField::Network => config.network.clone(),
        SettingsField::RefreshInterval => config.refresh_interval_ms.to_string(),
        SettingsField::AnalysisStart => {
            if config.analyze_from_pruning_point {
                "Pruning Point".to_string()
            } else {
                "Current".to_string()
            }
        }
    }
}

/// Apply an edited field value. Returns `true` if the edit was valid and applied.
fn apply_field_edit(state: &mut SettingsState, field: SettingsField, val: &str) -> bool {
    let trimmed = val.trim();
    match field {
        SettingsField::Url => {
            if trimmed.is_empty() {
                state.config.url = None;
                true
            } else if trimmed.starts_with("ws://") || trimmed.starts_with("wss://") {
                state.config.url = Some(trimmed.to_string());
                true
            } else {
                state.status_message =
                    Some(("URL must start with ws:// or wss://".to_string(), true));
                false
            }
        }
        SettingsField::Network => false, // cycled, not typed
        SettingsField::RefreshInterval => {
            if let Ok(v) = trimmed.parse::<u64>()
                && v >= 100
            {
                state.config.refresh_interval_ms = v;
                true
            } else {
                state.status_message = Some(("Interval must be >= 100ms".to_string(), true));
                false
            }
        }
        SettingsField::AnalysisStart => false, // cycled, not typed
    }
}

pub async fn handle_command(cmd: &str, app: &Arc<RwLock<App>>, rpc: &Arc<RpcManager>) {
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
            let mut app_guard = app.write().await;
            app_guard
                .command_line
                .push_output(cmd.to_string(), help_text, false);
        }
        "clear" => {
            let mut app_guard = app.write().await;
            app_guard.command_line.output.clear();
            app_guard.command_line.show_output = false;
        }
        _ => {
            // Try as RPC call
            match rpc.execute_rpc_call(command).await {
                Ok(response) => {
                    let mut app_guard = app.write().await;
                    app_guard
                        .command_line
                        .push_output(cmd.to_string(), response, false);
                }
                Err(e) => {
                    let mut app_guard = app.write().await;
                    app_guard
                        .command_line
                        .push_output(cmd.to_string(), e.to_string(), true);
                }
            }
        }
    }
}
