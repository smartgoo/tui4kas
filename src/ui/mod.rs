mod analytics;
mod blockdag;
mod command;
mod common;
mod dashboard;
mod mempool;
mod rpc_explorer;

use ratatui::Frame;

use crate::app::{App, Tab};

pub fn draw(frame: &mut Frame, app: &App) {
    use ratatui::layout::{Constraint, Direction, Layout};

    let show_output = app.command_line.show_output && !app.command_line.output.is_empty();

    let cmd_input_height: u16 = 3;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),              // header
            Constraint::Min(0),                 // main content
            Constraint::Length(cmd_input_height), // command input
        ])
        .split(frame.area());

    common::render_header(frame, chunks[0], app);

    if show_output {
        // Full-screen overlay: output fills the main content area
        command::render_output(frame, chunks[1], app);
    } else {
        match app.active_tab {
            Tab::Dashboard => dashboard::render(frame, chunks[1], app),
            Tab::Mempool => mempool::render(frame, chunks[1], app),
            Tab::BlockDag => blockdag::render(frame, chunks[1], app),
            Tab::Analytics => analytics::render(frame, chunks[1], app),
            Tab::RpcExplorer => rpc_explorer::render(frame, chunks[1], app),
        }
    }

    command::render_input(frame, chunks[2], app);
}
