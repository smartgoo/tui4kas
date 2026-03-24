use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::app::{App, RpcExplorerPanel};

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(30), Constraint::Min(0)])
        .split(area);

    render_method_list(frame, chunks[0], app);
    render_response(frame, chunks[1], app);
}

fn render_method_list(frame: &mut Frame, area: Rect, app: &App) {
    let is_active = app.rpc_explorer_panel == RpcExplorerPanel::Methods;
    let border_style = if is_active {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(Span::styled(
            " RPC Methods ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let selected = app.rpc_explorer.selected_method;
    let visible_rows = inner.height as usize;
    let scroll = if selected >= visible_rows {
        selected - visible_rows + 1
    } else {
        0
    };

    let lines: Vec<Line> = app
        .rpc_explorer
        .available_methods
        .iter()
        .enumerate()
        .skip(scroll)
        .take(visible_rows)
        .map(|(i, method)| {
            let style = if i == selected {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };
            let prefix = if i == selected { "▸ " } else { "  " };
            Line::from(Span::styled(format!("{}{}", prefix, method), style))
        })
        .collect();

    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_response(frame: &mut Frame, area: Rect, app: &App) {
    let content: &str = if app.rpc_explorer.is_loading {
        "Loading..."
    } else if let Some(ref response) = app.rpc_explorer.last_response {
        response.as_str()
    } else {
        "Press Enter to execute the selected RPC method."
    };

    let scroll_hint = if app.rpc_explorer.scroll_offset > 0 {
        format!(
            " Response (line {} | j/k scroll, J/K fast, PgUp/PgDn, Home) ",
            app.rpc_explorer.scroll_offset
        )
    } else {
        " Response (j/k to scroll) ".to_string()
    };

    // Clamp scroll offset to content bounds
    let content_lines = content.lines().count();
    let visible_height = area.height.saturating_sub(2) as usize;
    let max_scroll = content_lines.saturating_sub(visible_height);
    let clamped_scroll = app.rpc_explorer.scroll_offset.min(max_scroll);

    let is_active = app.rpc_explorer_panel == RpcExplorerPanel::Response;
    let border_style = if is_active {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let paragraph = Paragraph::new(content)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(Span::styled(
                    scroll_hint,
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )),
        )
        .wrap(Wrap { trim: false })
        .scroll((clamped_scroll as u16, 0));

    frame.render_widget(paragraph, area);
}
