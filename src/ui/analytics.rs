use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::App;
use crate::rpc::types::{format_number, sompi_to_kas};

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    if app.is_node_syncing() {
        let block = Block::default().borders(Borders::ALL).title(Span::styled(
            " Analytics ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        let msg = Paragraph::new(Line::from(Span::styled(
            " Node is syncing... Analytics will be available once synced.",
            Style::default().fg(Color::Yellow),
        )))
        .block(block);
        frame.render_widget(msg, area);
        return;
    }

    if !app.has_direct_node {
        let block = Block::default().borders(Borders::ALL).title(Span::styled(
            " Analytics ",
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

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Percentage(30),
            Constraint::Percentage(30),
        ])
        .split(area);

    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[0]);

    render_fee_stats(frame, top[0], app);
    render_tx_summary(frame, top[1], app);
    render_address_list(
        frame,
        rows[1],
        app,
        "Most Active Senders (recent blocks)",
        "No sender data available",
        |a| &a.top_senders,
    );
    render_address_list(
        frame,
        rows[2],
        app,
        "Most Active Receivers (recent blocks)",
        "No receiver data available",
        |a| &a.top_receivers,
    );
}

fn render_fee_stats(frame: &mut Frame, area: Rect, app: &App) {
    let lines = if let Some(ref analytics) = app.analytics {
        let fee = &analytics.fee_stats;
        vec![
            Line::from(vec![
                Span::styled(" Avg Fee (mass):  ", Style::default().fg(Color::DarkGray)),
                Span::raw(format!("{:.2}", fee.avg_fee_sompi)),
            ]),
            Line::from(vec![
                Span::styled(" Total Fees:      ", Style::default().fg(Color::DarkGray)),
                Span::raw(format!("{:.8} KAS", sompi_to_kas(fee.total_fees_sompi))),
            ]),
            Line::from(vec![
                Span::styled(" Min Fee (mass):  ", Style::default().fg(Color::DarkGray)),
                Span::raw(fee.min_fee_sompi.to_string()),
            ]),
            Line::from(vec![
                Span::styled(" Max Fee (mass):  ", Style::default().fg(Color::DarkGray)),
                Span::raw(fee.max_fee_sompi.to_string()),
            ]),
            Line::from(vec![
                Span::styled(" Tx w/ Fee Data:  ", Style::default().fg(Color::DarkGray)),
                Span::raw(fee.tx_count.to_string()),
            ]),
        ]
    } else {
        vec![Line::from(Span::styled(
            " Collecting fee data...",
            Style::default().fg(Color::DarkGray),
        ))]
    };

    let block = Block::default().borders(Borders::ALL).title(Span::styled(
        " Fee Analysis ",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ));

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_tx_summary(frame: &mut Frame, area: Rect, app: &App) {
    let lines = if let Some(ref analytics) = app.analytics {
        vec![
            Line::from(vec![
                Span::styled(" Blocks Analyzed:   ", Style::default().fg(Color::DarkGray)),
                Span::raw(format_number(analytics.blocks_analyzed as u64)),
            ]),
            Line::from(vec![
                Span::styled(" Total Transactions:", Style::default().fg(Color::DarkGray)),
                Span::raw(format!(
                    " {}",
                    format_number(analytics.total_transactions as u64)
                )),
            ]),
            Line::from(vec![
                Span::styled(" Unique Senders:    ", Style::default().fg(Color::DarkGray)),
                Span::raw(analytics.top_senders.len().to_string()),
            ]),
            Line::from(vec![
                Span::styled(" Unique Receivers:  ", Style::default().fg(Color::DarkGray)),
                Span::raw(analytics.top_receivers.len().to_string()),
            ]),
        ]
    } else {
        vec![Line::from(Span::styled(
            " Collecting transaction data...",
            Style::default().fg(Color::DarkGray),
        ))]
    };

    let block = Block::default().borders(Borders::ALL).title(Span::styled(
        " Transaction Summary ",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ));

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_address_list(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    title: &str,
    empty_msg: &str,
    get_entries: fn(&crate::rpc::types::AnalyticsData) -> &[crate::rpc::types::AddressActivity],
) {
    let lines = if let Some(ref analytics) = app.analytics {
        let entries = get_entries(analytics);
        if entries.is_empty() {
            vec![Line::from(Span::styled(
                format!(" {}", empty_msg),
                Style::default().fg(Color::DarkGray),
            ))]
        } else {
            let mut lines = vec![Line::from(vec![
                Span::styled(
                    "  Address",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("                                         "),
                Span::styled(
                    "Txs",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
            ])];
            for entry in entries {
                lines.push(Line::from(vec![
                    Span::raw(format!("  {:<45} ", entry.address)),
                    Span::styled(
                        entry.tx_count.to_string(),
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
            }
            lines
        }
    } else {
        vec![Line::from(Span::styled(
            " Collecting data...",
            Style::default().fg(Color::DarkGray),
        ))]
    };

    let block = Block::default().borders(Borders::ALL).title(Span::styled(
        format!(" {} ", title),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ));

    frame.render_widget(Paragraph::new(lines).block(block), area);
}
