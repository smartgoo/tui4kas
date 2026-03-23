use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use crate::app::{App, DagFocus};
use crate::rpc::types::format_number;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(19), // metrics + GHOSTDAG stats
            Constraint::Min(0),     // tips/parents
        ])
        .split(area);

    render_metrics(frame, chunks[0], app);
    render_tips(frame, chunks[1], app);

    if app.dag_selection.block_loading {
        render_loading_popup(frame, area);
    } else if let Some(ref detail) = app.dag_selection.block_detail {
        render_block_popup(frame, area, detail);
    }
}

fn render_metrics(frame: &mut Frame, area: Rect, app: &App) {
    let label = Style::default().fg(Color::DarkGray);
    let lines = if let Some(ref dag) = app.node.dag_info {
        let stats = &app.node.dag_stats;
        let mut lines = vec![
            Line::from(vec![
                Span::styled(" Network:          ", label),
                Span::raw(&dag.network),
            ]),
            Line::from(vec![
                Span::styled(" Block Count:      ", label),
                Span::raw(format_number(dag.block_count)),
            ]),
            Line::from(vec![
                Span::styled(" Header Count:     ", label),
                Span::raw(format_number(dag.header_count)),
            ]),
            Line::from(vec![
                Span::styled(" Difficulty:       ", label),
                Span::raw(format!("{:.4}", dag.difficulty)),
            ]),
            Line::from(vec![
                Span::styled(" DAA Score:        ", label),
                Span::raw(format_number(dag.virtual_daa_score)),
            ]),
            Line::from(vec![
                Span::styled(" Past Median Time: ", label),
                Span::raw(dag.past_median_time.to_string()),
            ]),
            Line::from(vec![
                Span::styled(" Pruning Point:    ", label),
                Span::raw(&dag.pruning_point_hash),
            ]),
            Line::from(vec![
                Span::styled(" Sink:             ", label),
                Span::raw(&dag.sink),
            ]),
            Line::from(vec![
                Span::styled(" Tips Count:       ", label),
                Span::raw(dag.tip_hashes.len().to_string()),
            ]),
            Line::from(vec![
                Span::styled(" Virtual Parents:  ", label),
                Span::raw(dag.virtual_parent_hashes.len().to_string()),
            ]),
        ];

        // GHOSTDAG separator and stats
        lines.push(Line::from(Span::styled(
            " ── GHOSTDAG ─────────────────────────",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(vec![
            Span::styled(" Blue Score:       ", label),
            Span::raw(
                stats
                    .sink_blue_score
                    .map(format_number)
                    .unwrap_or_else(|| "---".into()),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled(" Blue/Red Tips:    ", label),
            Span::raw(match stats.blue_red_ratio() {
                Some((blue, red)) => format!("{} blue / {} red", blue, red),
                None => "---".into(),
            }),
        ]));
        lines.push(Line::from(vec![
            Span::styled(" DAG Width:        ", label),
            Span::raw(match (stats.samples.back(), stats.avg_dag_width()) {
                (Some(s), Some(avg)) => format!("{} tips (avg: {:.1})", s.tip_count, avg),
                _ => "---".into(),
            }),
        ]));
        lines.push(Line::from(vec![
            Span::styled(" Block Interval:   ", label),
            Span::raw(match stats.block_interval_ms() {
                Some(ms) => format!("{:.0}ms", ms),
                None => "---".into(),
            }),
        ]));
        lines.push(Line::from(vec![
            Span::styled(" Blue Block Rate:  ", label),
            Span::raw(match stats.blue_block_rate() {
                Some(rate) => format!("{:.2} blocks/s", rate),
                None => "---".into(),
            }),
        ]));
        lines.push(Line::from(vec![
            Span::styled(" Unvalidated:      ", label),
            Span::raw(match stats.headers_blocks_delta() {
                Some(delta) => format!("{} headers ahead", format_number(delta)),
                None => "---".into(),
            }),
        ]));

        lines
    } else {
        vec![Line::from(Span::styled(
            " Waiting for DAG data...",
            Style::default().fg(Color::DarkGray),
        ))]
    };

    let block = Block::default().borders(Borders::ALL).title(Span::styled(
        " BlockDAG Metrics ",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ));

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_tips(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let tips_focused = app.dag_selection.focus == DagFocus::Tips;
    let parents_focused = app.dag_selection.focus == DagFocus::Parents;

    render_hash_list(
        frame,
        chunks[0],
        " Tip Hashes (←→ switch, ↑↓ select, Enter info, o open) ",
        app.node.dag_info.as_ref().map(|d| d.tip_hashes.as_slice()),
        tips_focused,
        app.dag_selection.tip_selected,
    );

    render_hash_list(
        frame,
        chunks[1],
        " Virtual Parent Hashes ",
        app.node
            .dag_info
            .as_ref()
            .map(|d| d.virtual_parent_hashes.as_slice()),
        parents_focused,
        app.dag_selection.parent_selected,
    );
}

fn render_hash_list(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    hashes: Option<&[String]>,
    is_focused: bool,
    selected: usize,
) {
    let border_color = if is_focused {
        Color::Cyan
    } else {
        Color::White
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let hashes = match hashes {
        Some(h) => h,
        None => return,
    };

    let visible_rows = inner.height as usize;
    let scroll = if selected >= visible_rows {
        selected - visible_rows + 1
    } else {
        0
    };

    let lines: Vec<Line> = hashes
        .iter()
        .enumerate()
        .skip(scroll)
        .take(visible_rows)
        .map(|(i, h)| {
            let style = if is_focused && i == selected {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };
            Line::from(Span::styled(h.as_str(), style))
        })
        .collect();

    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_loading_popup(frame: &mut Frame, area: Rect) {
    let popup_width = 30;
    let popup_height = 3;
    let x = (area.width.saturating_sub(popup_width)) / 2 + area.x;
    let y = (area.height.saturating_sub(popup_height)) / 2 + area.y;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    frame.render_widget(Clear, popup_area);
    let popup = Paragraph::new(" Loading block info...").block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow)),
    );
    frame.render_widget(popup, popup_area);
}

fn render_block_popup(frame: &mut Frame, area: Rect, detail: &str) {
    let content_lines = detail.lines().count() as u16;
    let popup_width = area.width.saturating_sub(10).clamp(40, 80);
    let popup_height = (content_lines + 4).clamp(8, area.height.saturating_sub(6));
    let x = (area.width.saturating_sub(popup_width)) / 2 + area.x;
    let y = (area.height.saturating_sub(popup_height)) / 2 + area.y;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    frame.render_widget(Clear, popup_area);

    let popup = Paragraph::new(detail.to_string())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(
                    " Block Info (Esc to close) ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ))
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(popup, popup_area);
}

