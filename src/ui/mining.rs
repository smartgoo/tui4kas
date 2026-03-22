use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::{App, MiningPanel};
use crate::rpc::types::format_hashrate;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    if super::common::render_syncing_guard(frame, area, app, "Mining") {
        return;
    }

    if !app.has_direct_node {
        let block = Block::default().borders(Borders::ALL).title(Span::styled(
            " Mining ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        let msg = Paragraph::new(Line::from(Span::styled(
            " Disabled when using Kaspa PNN via Resolver",
            Style::default().fg(Color::DarkGray),
        )))
        .block(block);
        frame.render_widget(msg, area);
        return;
    }

    let mining = match app.node.mining_info {
        Some(ref m) => m,
        None => {
            let block = Block::default().borders(Borders::ALL).title(Span::styled(
                " Mining ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
            let msg = Paragraph::new(Line::from(Span::styled(
                " Collecting mining data...",
                Style::default().fg(Color::DarkGray),
            )))
            .block(block);
            frame.render_widget(msg, area);
            return;
        }
    };

    // Layout: summary (4 lines) | middle (miners + pools) | bottom (versions)
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(area);

    // --- Summary bar ---
    let hashrate_str = format_hashrate(mining.hashrate);
    let summary_lines = vec![
        Line::from(vec![
            Span::styled(" Hashrate:         ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                &hashrate_str,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled(" Unique Miners:    ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!("{}", mining.unique_miners)),
            Span::styled("    Blocks Analyzed: ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!("{}", mining.blocks_analyzed)),
        ]),
    ];
    let summary_block = Block::default().borders(Borders::ALL).title(Span::styled(
        " Mining Summary ",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ));
    frame.render_widget(Paragraph::new(summary_lines).block(summary_block), rows[0]);

    // --- Middle: Miners (left) + Pools (right) ---
    let middle = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[1]);

    render_table_panel(
        frame,
        middle[0],
        "Miners",
        &["Address", "Blocks", "Share"],
        &mining.all_miners,
        mining.blocks_analyzed,
        app.mining_tab.active_panel == MiningPanel::Miners,
        app.mining_tab.miners_scroll,
    );

    render_table_panel(
        frame,
        middle[1],
        "Mining Pools",
        &["Pool", "Blocks", "Share"],
        &mining.pools,
        mining.blocks_analyzed,
        app.mining_tab.active_panel == MiningPanel::Pools,
        app.mining_tab.pools_scroll,
    );

    // --- Bottom: Node Versions ---
    render_table_panel(
        frame,
        rows[2],
        "Node Versions",
        &["Version", "Blocks", "Share"],
        &mining.node_versions,
        mining.blocks_analyzed,
        app.mining_tab.active_panel == MiningPanel::Versions,
        app.mining_tab.versions_scroll,
    );
}

#[allow(clippy::too_many_arguments)]
fn render_table_panel(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    headers: &[&str; 3],
    data: &[(String, usize)],
    total_blocks: usize,
    is_active: bool,
    scroll: usize,
) {
    let border_style = if is_active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(Span::styled(
            format!(" {} ", title),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 2 || inner.width < 10 {
        return;
    }

    // Header line
    let header_line = Line::from(vec![
        Span::styled(
            format!(" {:<width$}", headers[0], width = (inner.width as usize).saturating_sub(20).max(8)),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{:>7}", headers[1]),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{:>8} ", headers[2]),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    ]);

    let visible_rows = (inner.height as usize).saturating_sub(1); // minus header
    let max_scroll = data.len().saturating_sub(visible_rows);
    let scroll = scroll.min(max_scroll);

    let mut lines = vec![header_line];

    if data.is_empty() {
        lines.push(Line::from(Span::styled(
            " No data",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        let name_width = (inner.width as usize).saturating_sub(20).max(8);
        for (i, (name, count)) in data.iter().enumerate().skip(scroll).take(visible_rows) {
            let pct = if total_blocks > 0 {
                *count as f64 / total_blocks as f64 * 100.0
            } else {
                0.0
            };

            let row_style = if is_active && i == scroll {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            };

            let display_name: String = if name.chars().count() > name_width {
                name.chars().take(name_width.saturating_sub(1)).collect::<String>() + "~"
            } else {
                name.clone()
            };

            lines.push(Line::from(vec![
                Span::styled(
                    format!(" {:<width$}", display_name, width = name_width),
                    row_style.fg(Color::Cyan),
                ),
                Span::styled(format!("{:>7}", count), row_style.fg(Color::Gray)),
                Span::styled(format!("{:>7.1}%", pct), row_style.fg(Color::Gray)),
            ]));
        }
    }

    frame.render_widget(Paragraph::new(lines), inner);
}
