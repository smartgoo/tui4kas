use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table};
use ratatui::Frame;

use crate::app::App;
use crate::rpc::types::sompi_to_kas;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(0)])
        .split(area);

    render_summary(frame, chunks[0], app);
    render_table(frame, chunks[1], app);
}

fn render_summary(frame: &mut Frame, area: Rect, app: &App) {
    let lines = if let Some(ref mempool) = app.mempool_state {
        let orphan_count = mempool.entries.iter().filter(|e| e.is_orphan).count();
        vec![
            Line::from(vec![
                Span::styled(" Total Entries:  ", Style::default().fg(Color::Gray)),
                Span::styled(
                    mempool.entry_count.to_string(),
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled(" Orphans:        ", Style::default().fg(Color::Gray)),
                Span::raw(orphan_count.to_string()),
            ]),
            Line::from(vec![
                Span::styled(" Total Fees:     ", Style::default().fg(Color::Gray)),
                Span::raw(format!("{:.8} KAS", sompi_to_kas(mempool.total_fees))),
            ]),
        ]
    } else {
        vec![Line::from(Span::styled(
            " Waiting for mempool data...",
            Style::default().fg(Color::DarkGray),
        ))]
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " Mempool Summary ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_table(frame: &mut Frame, area: Rect, app: &App) {
    let header = Row::new(vec!["Transaction ID", "Fee (KAS)", "Orphan"])
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .bottom_margin(1);

    let rows: Vec<Row> = if let Some(ref mempool) = app.mempool_state {
        mempool
            .entries
            .iter()
            .skip(app.mempool_scroll)
            .map(|entry| {
                let id = if entry.transaction_id.len() > 20 {
                    format!("{}...{}", &entry.transaction_id[..10], &entry.transaction_id[entry.transaction_id.len()-10..])
                } else {
                    entry.transaction_id.clone()
                };

                Row::new(vec![
                    id,
                    format!("{:.8}", sompi_to_kas(entry.fee)),
                    if entry.is_orphan {
                        "Yes".to_string()
                    } else {
                        "No".to_string()
                    },
                ])
            })
            .collect()
    } else {
        vec![]
    };

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(50),
            Constraint::Percentage(30),
            Constraint::Percentage(20),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(
                " Entries ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
    )
    .row_highlight_style(Style::default().bg(Color::DarkGray));

    frame.render_widget(table, area);
}
