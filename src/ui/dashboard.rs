use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::rpc::types::sompi_to_kas;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[0]);

    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[1]);

    render_node_info(frame, top[0], app);
    render_network_stats(frame, top[1], app);
    render_coin_supply(frame, bottom[0], app);
    render_mempool_summary(frame, bottom[1], app);
}

fn render_node_info(frame: &mut Frame, area: Rect, app: &App) {
    let lines = if let Some(ref info) = app.server_info {
        let synced_style = if info.is_synced {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::Red)
        };

        let mut lines = vec![
            Line::from(vec![
                Span::styled(" Version:     ", Style::default().fg(Color::Gray)),
                Span::raw(&info.server_version),
            ]),
            Line::from(vec![
                Span::styled(" Network:     ", Style::default().fg(Color::Gray)),
                Span::raw(&info.network_id),
            ]),
            Line::from(vec![
                Span::styled(" Synced:      ", Style::default().fg(Color::Gray)),
                Span::styled(
                    if info.is_synced { "Yes" } else { "No" },
                    synced_style,
                ),
            ]),
            Line::from(vec![
                Span::styled(" UTXO Index:  ", Style::default().fg(Color::Gray)),
                Span::raw(if info.has_utxo_index { "Yes" } else { "No" }),
            ]),
            Line::from(vec![
                Span::styled(" DAA Score:   ", Style::default().fg(Color::Gray)),
                Span::raw(info.virtual_daa_score.to_string()),
            ]),
        ];

        if let Some(ref url) = app.node_url {
            lines.push(Line::from(vec![
                Span::styled(" URL:         ", Style::default().fg(Color::Gray)),
                Span::raw(url),
            ]));
        }
        if let Some(ref uid) = app.node_uid {
            lines.push(Line::from(vec![
                Span::styled(" Node ID:     ", Style::default().fg(Color::Gray)),
                Span::raw(uid),
            ]));
        }

        lines
    } else {
        vec![Line::from(Span::styled(
            " Waiting for data...",
            Style::default().fg(Color::DarkGray),
        ))]
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " Node Info ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

fn render_network_stats(frame: &mut Frame, area: Rect, app: &App) {
    let lines = if let Some(ref dag) = app.dag_info {
        vec![
            Line::from(vec![
                Span::styled(" Block Count:  ", Style::default().fg(Color::Gray)),
                Span::raw(format_number(dag.block_count)),
            ]),
            Line::from(vec![
                Span::styled(" Header Count: ", Style::default().fg(Color::Gray)),
                Span::raw(format_number(dag.header_count)),
            ]),
            Line::from(vec![
                Span::styled(" Difficulty:   ", Style::default().fg(Color::Gray)),
                Span::raw(format!("{:.2}", dag.difficulty)),
            ]),
            Line::from(vec![
                Span::styled(" DAA Score:    ", Style::default().fg(Color::Gray)),
                Span::raw(format_number(dag.virtual_daa_score)),
            ]),
            Line::from(vec![
                Span::styled(" Tips:         ", Style::default().fg(Color::Gray)),
                Span::raw(dag.tip_hashes.len().to_string()),
            ]),
        ]
    } else {
        vec![Line::from(Span::styled(
            " Waiting for data...",
            Style::default().fg(Color::DarkGray),
        ))]
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " Network Stats ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_coin_supply(frame: &mut Frame, area: Rect, app: &App) {
    let lines = if let Some(ref supply) = app.coin_supply {
        let max_kas = sompi_to_kas(supply.max_sompi);
        let circ_kas = sompi_to_kas(supply.circulating_sompi);
        let pct = if max_kas > 0.0 {
            (circ_kas / max_kas) * 100.0
        } else {
            0.0
        };

        vec![
            Line::from(vec![
                Span::styled(" Max Supply:         ", Style::default().fg(Color::Gray)),
                Span::raw(format!("{:.0} KAS", max_kas)),
            ]),
            Line::from(vec![
                Span::styled(" Circulating:        ", Style::default().fg(Color::Gray)),
                Span::raw(format!("{:.0} KAS", circ_kas)),
            ]),
            Line::from(vec![
                Span::styled(" % Circulating:      ", Style::default().fg(Color::Gray)),
                Span::raw(format!("{:.2}%", pct)),
            ]),
        ]
    } else {
        vec![Line::from(Span::styled(
            " Waiting for data...",
            Style::default().fg(Color::DarkGray),
        ))]
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " Coin Supply ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_mempool_summary(frame: &mut Frame, area: Rect, app: &App) {
    let mut lines = Vec::new();

    if let Some(ref mempool) = app.mempool_state {
        lines.push(Line::from(vec![
            Span::styled(" Transactions: ", Style::default().fg(Color::Gray)),
            Span::raw(format_number(mempool.entry_count as u64)),
        ]));
        lines.push(Line::from(vec![
            Span::styled(" Total Fees:   ", Style::default().fg(Color::Gray)),
            Span::raw(format!("{:.8} KAS", sompi_to_kas(mempool.total_fees))),
        ]));
    } else {
        lines.push(Line::from(Span::styled(
            " Waiting for data...",
            Style::default().fg(Color::DarkGray),
        )));
    }

    if let Some(ref fee) = app.fee_estimate {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(" Priority Fee: ", Style::default().fg(Color::Gray)),
            Span::raw(&fee.priority_bucket),
        ]));
        if let Some(normal) = fee.normal_buckets.first() {
            lines.push(Line::from(vec![
                Span::styled(" Normal Fee:   ", Style::default().fg(Color::Gray)),
                Span::raw(normal),
            ]));
        }
        if let Some(low) = fee.low_buckets.first() {
            lines.push(Line::from(vec![
                Span::styled(" Low Fee:      ", Style::default().fg(Color::Gray)),
                Span::raw(low),
            ]));
        }
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " Mempool & Fees ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_number_zero() {
        assert_eq!(format_number(0), "0");
    }

    #[test]
    fn format_number_small() {
        assert_eq!(format_number(1), "1");
        assert_eq!(format_number(999), "999");
    }

    #[test]
    fn format_number_thousands() {
        assert_eq!(format_number(1_000), "1,000");
        assert_eq!(format_number(12_345), "12,345");
    }

    #[test]
    fn format_number_millions() {
        assert_eq!(format_number(1_000_000), "1,000,000");
        assert_eq!(format_number(123_456_789), "123,456,789");
    }

    #[test]
    fn format_number_large() {
        assert_eq!(format_number(1_000_000_000_000), "1,000,000,000,000");
    }
}
