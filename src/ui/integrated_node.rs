use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::app::{App, DaemonStatus, IntegratedNodeState};

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    match &app.integrated_node.status {
        DaemonStatus::Stopped | DaemonStatus::Error(_) => render_settings(frame, area, app),
        _ => render_running(frame, area, app),
    }
}

fn render_settings(frame: &mut Frame, area: Rect, app: &App) {
    let state = &app.integrated_node;

    let mut lines = build_field_lines(
        state,
        &[
            (0, "Network", &state.config.network),
            (
                1,
                "UTXO Index",
                if state.config.utxo_index { "Yes" } else { "No" },
            ),
            (2, "RAM Scale", &format!("{:.1}", state.config.ram_scale)),
            (3, "App Dir", &state.config.app_dir),
            (4, "Log Level", &state.config.log_level),
            (5, "Async Threads", &state.config.async_threads.to_string()),
            (
                6,
                "Auto Start",
                if state.config.auto_start_daemon {
                    "Yes"
                } else {
                    "No"
                },
            ),
        ],
    );

    lines.push(Line::from(""));

    // Start action
    let start_style = if state.selected_field == 7 {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Green)
    };
    let prefix = if state.selected_field == 7 {
        " > "
    } else {
        "   "
    };
    lines.push(Line::from(Span::styled(
        format!("{}[ Start Daemon ]", prefix),
        start_style,
    )));

    lines.push(Line::from(""));

    if let DaemonStatus::Error(ref msg) = state.status {
        lines.push(Line::from(Span::styled(
            format!(" Error: {}", msg),
            Style::default().fg(Color::Red),
        )));
        lines.push(Line::from(""));
    }

    if let Some((ref msg, is_error)) = state.status_message {
        let color = if is_error { Color::Red } else { Color::Green };
        lines.push(Line::from(Span::styled(
            format!(" {}", msg),
            Style::default().fg(color),
        )));
        lines.push(Line::from(""));
    }

    lines.push(Line::from(vec![
        Span::styled(" [s]", Style::default().fg(Color::Cyan)),
        Span::styled(" Save  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[r]", Style::default().fg(Color::Cyan)),
        Span::styled(" Reload  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[←/→]", Style::default().fg(Color::Cyan)),
        Span::styled(" Cycle  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[Enter]", Style::default().fg(Color::Cyan)),
        Span::styled(" Edit/Toggle", Style::default().fg(Color::DarkGray)),
    ]));

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Node Settings ");
    let para = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    frame.render_widget(para, area);
}

fn build_field_lines(
    state: &IntegratedNodeState,
    fields: &[(usize, &str, &str)],
) -> Vec<Line<'static>> {
    fields
        .iter()
        .map(|(idx, label, value)| {
            let selected = state.selected_field == *idx;
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
                value.to_string()
            };

            Line::from(vec![
                Span::raw(prefix.to_string()),
                Span::styled(format!("{:<16}", label), label_style),
                Span::styled(display_value, value_style),
            ])
        })
        .collect()
}

fn render_running(frame: &mut Frame, area: Rect, app: &App) {
    let state = &app.integrated_node;

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(0)])
        .split(area);

    // Status panel
    let status_text = match &state.status {
        DaemonStatus::Starting => ("Starting...", Color::Yellow),
        DaemonStatus::Running => ("Running", Color::Green),
        DaemonStatus::Stopping => ("Stopping...", Color::Yellow),
        _ => ("Unknown", Color::DarkGray),
    };

    let sync_status = if let Some(ref info) = app.server_info {
        if info.is_synced {
            ("Synced".to_string(), Color::Green)
        } else {
            ("Syncing...".to_string(), Color::Yellow)
        }
    } else {
        ("Waiting...".to_string(), Color::DarkGray)
    };

    let uptime = state
        .started_at
        .map(|t| {
            let secs = t.elapsed().as_secs();
            if secs >= 3600 {
                format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
            } else if secs >= 60 {
                format!("{}m {}s", secs / 60, secs % 60)
            } else {
                format!("{}s", secs)
            }
        })
        .unwrap_or_else(|| "—".to_string());

    let port = crate::daemon::DaemonHandle::wrpc_borsh_url(&state.config.network);

    let mut status_lines = vec![
        Line::from(vec![
            Span::styled(" Status:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(status_text.0, Style::default().fg(status_text.1)),
            Span::raw("  "),
            Span::styled(sync_status.0, Style::default().fg(sync_status.1)),
        ]),
        Line::from(vec![
            Span::styled(" Network:  ", Style::default().fg(Color::DarkGray)),
            Span::raw(&state.config.network),
        ]),
        Line::from(vec![
            Span::styled(" wRPC:     ", Style::default().fg(Color::DarkGray)),
            Span::raw(&port),
        ]),
        Line::from(vec![
            Span::styled(" Uptime:   ", Style::default().fg(Color::DarkGray)),
            Span::raw(uptime),
        ]),
    ];

    if matches!(state.status, DaemonStatus::Running) {
        status_lines.push(Line::from(vec![
            Span::styled(" Press ", Style::default().fg(Color::DarkGray)),
            Span::styled("[Enter]", Style::default().fg(Color::Cyan)),
            Span::styled(" to stop daemon", Style::default().fg(Color::DarkGray)),
        ]));
    }

    let status_block = Block::default()
        .borders(Borders::ALL)
        .title(" Daemon Status ");
    let status_para = Paragraph::new(status_lines).block(status_block);
    frame.render_widget(status_para, rows[0]);

    // Log panel
    let log_block = Block::default()
        .borders(Borders::ALL)
        .title(" Daemon Logs (j/k scroll) ");

    let inner_height = rows[1].height.saturating_sub(2) as usize;
    let total_lines = state.log_lines.len();
    let max_scroll = total_lines.saturating_sub(inner_height);
    let scroll = state.log_scroll.min(max_scroll);

    let visible_lines: Vec<Line> = state
        .log_lines
        .iter()
        .skip(scroll)
        .take(inner_height)
        .map(|line| {
            let color = if line.contains("ERROR") {
                Color::Red
            } else if line.contains("WARN") {
                Color::Yellow
            } else if line.contains("INFO") {
                Color::Green
            } else {
                Color::DarkGray
            };
            Line::from(Span::styled(
                format!(" {}", line),
                Style::default().fg(color),
            ))
        })
        .collect();

    let log_content = if visible_lines.is_empty() {
        Paragraph::new(Line::from(Span::styled(
            " Waiting for log output...",
            Style::default().fg(Color::DarkGray),
        )))
    } else {
        Paragraph::new(visible_lines)
    };

    frame.render_widget(log_content.block(log_block), rows[1]);
}
