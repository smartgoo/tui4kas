mod analytics;
pub(crate) mod common;
mod contextbar;
mod dashboard;
mod help;
mod mempool;
mod mining;
mod rpc_explorer;
mod settings;

use ratatui::Frame;

use crate::app::{App, Tab};

pub fn draw(frame: &mut Frame, app: &App) {
    use ratatui::layout::{Constraint, Direction, Layout};

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Min(0),    // main content
            Constraint::Length(3), // status bar
        ])
        .split(frame.area());

    common::render_header(frame, chunks[0], app);

    match app.active_tab {
        Tab::Dashboard => dashboard::render(frame, chunks[1], app),
        Tab::Mining => mining::render(frame, chunks[1], app),
        Tab::Mempool => mempool::render(frame, chunks[1], app),
        Tab::Analytics => analytics::render(frame, chunks[1], app),
        Tab::RpcExplorer => rpc_explorer::render(frame, chunks[1], app),
        Tab::Settings => settings::render(frame, chunks[1], app),
    }

    contextbar::render(frame, chunks[2], app);

    // Help overlay on top of everything
    if app.show_help {
        help::render_help(frame, frame.area(), app);
    }
}
