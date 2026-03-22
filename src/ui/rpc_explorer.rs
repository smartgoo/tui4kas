use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};

use crate::app::App;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    if super::common::render_syncing_guard(frame, area, app, "RPC Cmds") {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(30), Constraint::Min(0)])
        .split(area);

    render_method_list(frame, chunks[0], app);
    render_response(frame, chunks[1], app);
}

fn render_method_list(frame: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = app
        .rpc_explorer
        .available_methods
        .iter()
        .enumerate()
        .map(|(i, method)| {
            let style = if i == app.rpc_explorer.selected_method {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let prefix = if i == app.rpc_explorer.selected_method {
                "▸ "
            } else {
                "  "
            };

            ListItem::new(Line::from(Span::styled(
                format!("{}{}", prefix, method),
                style,
            )))
        })
        .collect();

    let list = List::new(items).block(
        Block::default().borders(Borders::ALL).title(Span::styled(
            " RPC Methods ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
    );

    frame.render_widget(list, area);
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

    let paragraph = Paragraph::new(content)
        .block(
            Block::default().borders(Borders::ALL).title(Span::styled(
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
