use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Tabs};
use ratatui::Frame;

use crate::app::{App, ConnectionStatus, Tab};

pub fn render_header(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(40)])
        .split(area);

    let titles: Vec<Line> = Tab::ALL
        .iter()
        .map(|t| Line::from(t.title()))
        .collect();

    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" tui4kas "),
        )
        .select(app.tab_index())
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );

    frame.render_widget(tabs, chunks[0]);

    let (status_text, status_color) = match &app.connection_status {
        ConnectionStatus::Connected => ("Connected", Color::Green),
        ConnectionStatus::Connecting => ("Connecting...", Color::Yellow),
        ConnectionStatus::Disconnected => ("Disconnected", Color::Red),
        ConnectionStatus::Error(_) => ("Error", Color::Red),
    };

    let poll_text = if app.paused {
        String::from(" | Paused")
    } else {
        app.last_poll_duration_ms
            .map(|ms| format!(" | {:.0}ms", ms))
            .unwrap_or_default()
    };

    let poll_color = if app.paused { Color::Yellow } else { Color::DarkGray };

    let status = Paragraph::new(Line::from(vec![
        Span::raw(" "),
        Span::styled("●", Style::default().fg(status_color)),
        Span::raw(" "),
        Span::styled(status_text, Style::default().fg(status_color)),
        Span::styled(&poll_text, Style::default().fg(poll_color)),
    ]))
    .block(Block::default().borders(Borders::ALL));

    frame.render_widget(status, chunks[1]);
}

