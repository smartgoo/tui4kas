use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::App;
use crate::rpc::types::{format_number, sompi_to_kas};

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(35),
            Constraint::Percentage(35),
            Constraint::Percentage(30),
        ])
        .split(area);

    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[0]);

    let middle = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[1]);

    render_node_info(frame, top[0], app);
    render_network_stats(frame, top[1], app);
    render_markets(frame, middle[0], app);
    render_mempool_summary(frame, middle[1], app);
    render_mining_info(frame, rows[2], app);
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
                Span::styled(" Version:     ", Style::default().fg(Color::DarkGray)),
                Span::raw(&info.server_version),
            ]),
            Line::from(vec![
                Span::styled(" Network:     ", Style::default().fg(Color::DarkGray)),
                Span::raw(&info.network_id),
            ]),
            Line::from(vec![
                Span::styled(" Synced:      ", Style::default().fg(Color::DarkGray)),
                Span::styled(if info.is_synced { "Yes" } else { "No" }, synced_style),
            ]),
            Line::from(vec![
                Span::styled(" UTXO Index:  ", Style::default().fg(Color::DarkGray)),
                Span::raw(if info.has_utxo_index { "Yes" } else { "No" }),
            ]),
        ];

        if app.is_daemon_active() {
            lines.push(Line::from(vec![
                Span::styled(" Mode:        ", Style::default().fg(Color::DarkGray)),
                Span::styled("Embedded Node", Style::default().fg(Color::Green)),
            ]));
        }

        if let Some(ref url) = app.node_url {
            lines.push(Line::from(vec![
                Span::styled(" URL:         ", Style::default().fg(Color::DarkGray)),
                Span::raw(url),
            ]));
        }
        if let Some(ref uid) = app.node_uid {
            lines.push(Line::from(vec![
                Span::styled(" Node ID:     ", Style::default().fg(Color::DarkGray)),
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

    let block = Block::default().borders(Borders::ALL).title(Span::styled(
        " Node Info ",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ));

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

fn render_network_stats(frame: &mut Frame, area: Rect, app: &App) {
    if app.is_node_syncing() {
        let daa = app
            .server_info
            .as_ref()
            .map(|s| format_number(s.virtual_daa_score))
            .unwrap_or_default();
        let lines = vec![
            Line::from(Span::styled(
                " Node is syncing...",
                Style::default().fg(Color::Yellow),
            )),
            Line::from(vec![
                Span::styled(" DAA Score:      ", Style::default().fg(Color::DarkGray)),
                Span::raw(daa),
            ]),
        ];
        let block = Block::default().borders(Borders::ALL).title(Span::styled(
            " Network Stats ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        frame.render_widget(Paragraph::new(lines).block(block), area);
        return;
    }

    let mut lines = if let Some(ref dag) = app.dag_info {
        vec![
            Line::from(vec![
                Span::styled(" Block Count:    ", Style::default().fg(Color::DarkGray)),
                Span::raw(format_number(dag.block_count)),
            ]),
            Line::from(vec![
                Span::styled(" Header Count:   ", Style::default().fg(Color::DarkGray)),
                Span::raw(format_number(dag.header_count)),
            ]),
            Line::from(vec![
                Span::styled(" Difficulty:     ", Style::default().fg(Color::DarkGray)),
                Span::raw(format!("{:.0}", dag.difficulty)),
            ]),
            Line::from(vec![
                Span::styled(" DAA Score:      ", Style::default().fg(Color::DarkGray)),
                Span::raw(format_number(dag.virtual_daa_score)),
            ]),
            Line::from(vec![
                Span::styled(" Tips:           ", Style::default().fg(Color::DarkGray)),
                Span::raw(dag.tip_hashes.len().to_string()),
            ]),
        ]
    } else {
        vec![Line::from(Span::styled(
            " Waiting for data...",
            Style::default().fg(Color::DarkGray),
        ))]
    };

    if let Some(ref supply) = app.coin_supply {
        let max_kas = sompi_to_kas(supply.max_sompi);
        let circ_kas = sompi_to_kas(supply.circulating_sompi);
        let pct = if max_kas > 0.0 {
            (circ_kas / max_kas) * 100.0
        } else {
            0.0
        };

        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(" Max Supply:     ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!("{} KAS", format_number(max_kas as u64))),
        ]));
        lines.push(Line::from(vec![
            Span::styled(" Circulating:    ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!("{} KAS", format_number(circ_kas as u64))),
        ]));
        lines.push(Line::from(vec![
            Span::styled(" % Circulating:  ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!("{:.2}%", pct)),
        ]));
    }

    let block = Block::default().borders(Borders::ALL).title(Span::styled(
        " Network Stats ",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ));

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_markets(frame: &mut Frame, area: Rect, app: &App) {
    let lines = if let Some(ref market) = app.market_data {
        let change_color = if market.price_change_24h_pct >= 0.0 {
            Color::Green
        } else {
            Color::Red
        };
        let change_prefix = if market.price_change_24h_pct >= 0.0 {
            "+"
        } else {
            ""
        };

        vec![
            Line::from(vec![
                Span::styled(" Price (USD):   ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("${:.6}", market.price_usd),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(
                    format!("{}{:.2}%", change_prefix, market.price_change_24h_pct),
                    Style::default().fg(change_color),
                ),
            ]),
            Line::from(vec![
                Span::styled(" Price (BTC):   ", Style::default().fg(Color::DarkGray)),
                Span::raw(format!("{:.10}", market.price_btc)),
            ]),
            Line::from(vec![
                Span::styled(" Market Cap:    ", Style::default().fg(Color::DarkGray)),
                Span::raw(format_usd(market.market_cap)),
            ]),
            Line::from(vec![
                Span::styled(" 24h Volume:    ", Style::default().fg(Color::DarkGray)),
                Span::raw(format_usd(market.volume_24h)),
            ]),
        ]
    } else {
        vec![Line::from(Span::styled(
            " Fetching market data...",
            Style::default().fg(Color::DarkGray),
        ))]
    };

    let block = Block::default().borders(Borders::ALL).title(Span::styled(
        " Markets ",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ));

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_mining_info(frame: &mut Frame, area: Rect, app: &App) {
    if app.is_node_syncing() {
        let block = Block::default().borders(Borders::ALL).title(Span::styled(
            " Mining Info ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        let msg = Paragraph::new(Line::from(Span::styled(
            " Node is syncing...",
            Style::default().fg(Color::Yellow),
        )))
        .block(block);
        frame.render_widget(msg, area);
        return;
    }

    let lines = if let Some(ref mining) = app.mining_info {
        let hashrate_str = format_hashrate(mining.hashrate);
        let mut lines = vec![
            Line::from(vec![
                Span::styled(" Hashrate:        ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    hashrate_str,
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled(" Unique Miners:   ", Style::default().fg(Color::DarkGray)),
                Span::raw(format!(
                    "{} (last {} blocks)",
                    mining.unique_miners, mining.blocks_analyzed
                )),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                " Top Miners:",
                Style::default().fg(Color::DarkGray),
            )),
        ];

        for (addr, count) in &mining.top_miners {
            lines.push(Line::from(vec![
                Span::raw("   "),
                Span::styled(addr, Style::default().fg(Color::White)),
                Span::raw(format!("  ({} blocks)", count)),
            ]));
        }

        lines
    } else if !app.has_direct_node {
        vec![Line::from(Span::styled(
            " Disabled when using Kaspa PNN via Resolver",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        vec![Line::from(Span::styled(
            " Collecting mining data...",
            Style::default().fg(Color::DarkGray),
        ))]
    };

    let block = Block::default().borders(Borders::ALL).title(Span::styled(
        " Mining Info ",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ));

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn format_hashrate(hps: f64) -> String {
    if hps >= 1e18 {
        format!("{:.2} EH/s", hps / 1e18)
    } else if hps >= 1e15 {
        format!("{:.2} PH/s", hps / 1e15)
    } else if hps >= 1e12 {
        format!("{:.2} TH/s", hps / 1e12)
    } else if hps >= 1e9 {
        format!("{:.2} GH/s", hps / 1e9)
    } else if hps >= 1e6 {
        format!("{:.2} MH/s", hps / 1e6)
    } else if hps >= 1e3 {
        format!("{:.2} KH/s", hps / 1e3)
    } else {
        format!("{:.2} H/s", hps)
    }
}

fn format_usd(value: f64) -> String {
    if value >= 1_000_000_000.0 {
        format!("${:.2}B", value / 1_000_000_000.0)
    } else if value >= 1_000_000.0 {
        format!("${:.2}M", value / 1_000_000.0)
    } else if value >= 1_000.0 {
        format!("${:.2}K", value / 1_000.0)
    } else {
        format!("${:.2}", value)
    }
}

fn render_mempool_summary(frame: &mut Frame, area: Rect, app: &App) {
    if app.is_node_syncing() {
        let block = Block::default().borders(Borders::ALL).title(Span::styled(
            " Mempool & Fees ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        let msg = Paragraph::new(Line::from(Span::styled(
            " Node is syncing...",
            Style::default().fg(Color::Yellow),
        )))
        .block(block);
        frame.render_widget(msg, area);
        return;
    }

    let mut lines = Vec::new();

    if let Some(ref mempool) = app.mempool_state {
        lines.push(Line::from(vec![
            Span::styled(" Transactions: ", Style::default().fg(Color::DarkGray)),
            Span::raw(format_number(mempool.entry_count as u64)),
        ]));
        lines.push(Line::from(vec![
            Span::styled(" Total Fees:   ", Style::default().fg(Color::DarkGray)),
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
            Span::styled(" Priority Fee: ", Style::default().fg(Color::DarkGray)),
            Span::raw(&fee.priority_bucket),
        ]));
        if let Some(normal) = fee.normal_buckets.first() {
            lines.push(Line::from(vec![
                Span::styled(" Normal Fee:   ", Style::default().fg(Color::DarkGray)),
                Span::raw(normal),
            ]));
        }
        if let Some(low) = fee.low_buckets.first() {
            lines.push(Line::from(vec![
                Span::styled(" Low Fee:      ", Style::default().fg(Color::DarkGray)),
                Span::raw(low),
            ]));
        }
    }

    let block = Block::default().borders(Borders::ALL).title(Span::styled(
        " Mempool & Fees ",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ));

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rpc::types::format_number;

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

    // --- format_usd ---

    #[test]
    fn format_usd_billions() {
        assert_eq!(format_usd(3_800_000_000.0), "$3.80B");
    }

    #[test]
    fn format_usd_millions() {
        assert_eq!(format_usd(50_000_000.0), "$50.00M");
    }

    #[test]
    fn format_usd_thousands() {
        assert_eq!(format_usd(1_500.0), "$1.50K");
    }

    #[test]
    fn format_usd_small() {
        assert_eq!(format_usd(42.50), "$42.50");
    }

    // --- format_hashrate ---

    #[test]
    fn format_hashrate_ph() {
        assert_eq!(format_hashrate(1.5e15), "1.50 PH/s");
    }

    #[test]
    fn format_hashrate_th() {
        assert_eq!(format_hashrate(500e12), "500.00 TH/s");
    }

    #[test]
    fn format_hashrate_gh() {
        assert_eq!(format_hashrate(2.5e9), "2.50 GH/s");
    }

    #[test]
    fn format_hashrate_mh() {
        assert_eq!(format_hashrate(100e6), "100.00 MH/s");
    }

    #[test]
    fn format_hashrate_small() {
        assert_eq!(format_hashrate(500.0), "500.00 H/s");
    }
}
