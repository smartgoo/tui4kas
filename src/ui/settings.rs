use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::app::{App, ConnectionStatus, SettingsState};

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let state = &app.settings;
    let cfg = &state.config;

    let mut lines: Vec<Line<'static>> = Vec::new();

    // Connection status
    let (status_text, status_color) = match &app.node.connection_status {
        ConnectionStatus::Connected => ("Connected", Color::Green),
        ConnectionStatus::Connecting => ("Connecting...", Color::Yellow),
        ConnectionStatus::Disconnected => ("Disconnected", Color::Red),
        ConnectionStatus::Error(_) => ("Error", Color::Red),
    };

    lines.push(Line::from(vec![
        Span::styled("  Status:  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{} {}", "●", status_text),
            Style::default().fg(status_color),
        ),
    ]));
    lines.push(Line::from(""));

    // Connection section
    lines.push(section_header("Connection"));

    // URL field
    let url_display = cfg.url.clone().unwrap_or_else(|| "(PNN — Public Node Network)".to_string());
    append_field(state, &mut lines, 0, "wRPC URL", url_display);

    // Network field
    append_field(state, &mut lines, 1, "Network", cfg.network.clone());

    // Refresh interval
    append_field(
        state,
        &mut lines,
        2,
        "Refresh (ms)",
        cfg.refresh_interval_ms.to_string(),
    );

    lines.push(Line::from(""));

    // Analytics section
    lines.push(section_header("Analytics"));

    let analysis_start = if cfg.analyze_from_pruning_point {
        "Pruning Point"
    } else {
        "Current"
    };
    append_field(state, &mut lines, 3, "Analyze From", analysis_start.to_string());

    lines.push(Line::from(""));

    if let Some((ref msg, is_error)) = state.status_message {
        let color = if is_error { Color::Red } else { Color::Green };
        lines.push(Line::from(Span::styled(
            format!("  {}", msg),
            Style::default().fg(color),
        )));
        lines.push(Line::from(""));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Leave URL empty to use Public Node Network (PNN)",
        Style::default().fg(Color::DarkGray),
    )));

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  [r]", Style::default().fg(Color::Cyan)),
        Span::styled(" Reload  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[←/→]", Style::default().fg(Color::Cyan)),
        Span::styled(" Cycle  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[Enter]", Style::default().fg(Color::Cyan)),
        Span::styled(" Edit/Cycle  ", Style::default().fg(Color::DarkGray)),
    ]));
    lines.push(Line::from(Span::styled(
        "  Changes auto-save and reconnect",
        Style::default().fg(Color::DarkGray),
    )));

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Settings ");
    let para = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    frame.render_widget(para, area);
}

fn section_header(title: &str) -> Line<'static> {
    Line::from(Span::styled(
        format!("  ── {} ──", title),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ))
}

fn append_field(
    state: &SettingsState,
    lines: &mut Vec<Line<'static>>,
    idx: usize,
    label: &str,
    value: String,
) {
    let selected = state.selected_field == idx;
    let prefix = if selected { " > " } else { "   " };
    let label_style = Style::default().fg(Color::DarkGray);
    let value_style = if selected && state.editing {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else if selected {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let display_value = if selected && state.editing {
        format!("{}_", state.edit_buffer)
    } else {
        value
    };

    lines.push(Line::from(vec![
        Span::raw(prefix.to_string()),
        Span::styled(format!("{:<18}", label), label_style),
        Span::styled(display_value, value_style),
    ]));
}
