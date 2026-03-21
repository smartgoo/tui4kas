mod app;
mod cli;
mod event;
mod rpc;
mod ui;

use std::io;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use crossterm::event::{KeyCode, KeyModifiers};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::Mutex;

use crate::app::{App, CommandLine, Tab};
use crate::cli::CliArgs;
use crate::event::{AppEvent, EventHandler};
use crate::rpc::client::RpcManager;

#[tokio::main]
async fn main() -> Result<()> {
    let args = CliArgs::parse();

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
    let app = Arc::new(Mutex::new(App::new()));

    // Create RPC manager and start polling
    let mut rpc_manager = RpcManager::new(
        args.url.clone(),
        &args.network,
        app.clone(),
    )
    .await?;

    rpc_manager.start_polling(Duration::from_millis(args.refresh_interval_ms));

    let rpc_for_explorer = Arc::new(rpc_manager);

    // Connect in background to not block TUI startup
    let rpc_for_connect = rpc_for_explorer.clone();
    tokio::spawn(async move {
        let _ = rpc_for_connect.connect().await;
    });

    // Event loop
    let mut events = EventHandler::new(Duration::from_millis(250));

    loop {
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
                        _ => {
                            match app_guard.active_tab {
                                Tab::RpcExplorer => {
                                    handle_rpc_explorer_keys(&mut app_guard, key.code, &rpc_for_explorer, &app);
                                }
                                Tab::Mempool => {
                                    handle_mempool_keys(&mut app_guard, key.code);
                                }
                                Tab::BlockDag => {
                                    handle_blockdag_keys(&mut app_guard, key.code);
                                }
                                _ => {}
                            }
                        }
                    }
                }

                if app_guard.should_quit {
                    break;
                }
            }
            AppEvent::Tick | AppEvent::Resize(_, _) => {}
        }
    }

    // Cleanup
    disable_raw_mode()?;
    terminal.backend_mut().execute(LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
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
            help_text.push_str("\nPress ':' to open command line, Esc to close, Up/Down for history");
            let mut app_guard = app.lock().await;
            app_guard.command_line.push_output(cmd.to_string(), help_text, false);
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
                    app_guard.command_line.push_output(cmd.to_string(), response, false);
                }
                Err(e) => {
                    let mut app_guard = app.lock().await;
                    app_guard.command_line.push_output(cmd.to_string(), e.to_string(), true);
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
        KeyCode::Char('j') => {
            app.rpc_explorer.scroll_offset = app.rpc_explorer.scroll_offset.saturating_add(1);
        }
        KeyCode::Char('k') => {
            app.rpc_explorer.scroll_offset = app.rpc_explorer.scroll_offset.saturating_sub(1);
        }
        _ => {}
    }
}

fn handle_mempool_keys(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Up => {
            app.mempool_scroll = app.mempool_scroll.saturating_sub(1);
        }
        KeyCode::Down => {
            if let Some(ref mempool) = app.mempool_state
                && app.mempool_scroll < mempool.entries.len().saturating_sub(1) {
                    app.mempool_scroll += 1;
            }
        }
        _ => {}
    }
}

fn handle_blockdag_keys(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Up => {
            app.dag_scroll = app.dag_scroll.saturating_sub(1);
        }
        KeyCode::Down => {
            if let Some(ref dag) = app.dag_info
                && app.dag_scroll < dag.tip_hashes.len().saturating_sub(1) {
                    app.dag_scroll += 1;
            }
        }
        _ => {}
    }
}
