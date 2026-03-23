use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::{App, MiningPanel};
use crate::rpc::types::{MiningInfo, format_hashrate, format_number};

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    if !app.has_direct_url() {
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
            Constraint::Length(3),
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(area);

    // --- Summary bar ---
    let hashrate_str = format_hashrate(mining.hashrate);
    let summary_lines = vec![
        Line::from(vec![
            Span::styled(" Hashrate: ", Style::default().fg(Color::DarkGray)),
            Span::raw(&hashrate_str),
            Span::styled("    Unique Miners: ", Style::default().fg(Color::DarkGray)),
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
        app.mining_tab.miners_selected,
    );

    render_table_panel(
        frame,
        middle[1],
        "Mining Pools",
        &["Pool", "Blocks", "Share"],
        &mining.pools,
        mining.blocks_analyzed,
        app.mining_tab.active_panel == MiningPanel::Pools,
        app.mining_tab.pools_selected,
    );

    // --- Bottom: Node Versions (left) + Mass Info (right) ---
    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[2]);

    render_table_panel(
        frame,
        bottom[0],
        "Node Versions",
        &["Version", "Blocks", "Share"],
        &mining.node_versions,
        mining.blocks_analyzed,
        app.mining_tab.active_panel == MiningPanel::Versions,
        app.mining_tab.versions_selected,
    );

    render_fee_panel(frame, bottom[1], mining);
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
    selected: usize,
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
    let selected = selected.min(data.len().saturating_sub(1));

    // Compute scroll offset to keep selection visible
    let scroll = if selected >= visible_rows {
        selected - visible_rows + 1
    } else {
        0
    };

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

            let row_style = if is_active && i == selected {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };

            lines.push(Line::from(vec![
                Span::styled(
                    format!(" {:<width$}", name, width = name_width),
                    row_style,
                ),
                Span::styled(format!("{:>7}", count), row_style),
                Span::styled(format!("{:>7.1}%", pct), row_style),
            ]));
        }
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_fee_panel(frame: &mut Frame, area: Rect, mining: &MiningInfo) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            " Mass Info ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));

    let avg_mass = if mining.mass_count > 0 {
        mining.total_mass as f64 / mining.mass_count as f64
    } else {
        0.0
    };

    let lines = vec![
        Line::from(vec![
            Span::styled(" Avg Mass:           ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!("{:.2}", avg_mass)),
        ]),
        Line::from(vec![
            Span::styled(" Min Mass:           ", Style::default().fg(Color::DarkGray)),
            Span::raw(if mining.mass_count > 0 {
                mining.min_mass.to_string()
            } else {
                "N/A".to_string()
            }),
        ]),
        Line::from(vec![
            Span::styled(" Max Mass:           ", Style::default().fg(Color::DarkGray)),
            Span::raw(if mining.mass_count > 0 {
                mining.max_mass.to_string()
            } else {
                "N/A".to_string()
            }),
        ]),
        Line::from(vec![
            Span::styled(" Total Mass:         ", Style::default().fg(Color::DarkGray)),
            Span::raw(format_number(mining.total_mass)),
        ]),
        Line::from(vec![
            Span::styled(" Tx w/ Mass Data:    ", Style::default().fg(Color::DarkGray)),
            Span::raw(format_number(mining.mass_count as u64)),
        ]),
    ];

    frame.render_widget(Paragraph::new(lines).block(block), area);
}
