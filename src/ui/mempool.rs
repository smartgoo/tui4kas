use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Row, Table, Wrap};

use crate::app::App;
use crate::rpc::types::sompi_to_kas;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    if app.is_node_syncing() {
        let block = Block::default().borders(Borders::ALL).title(Span::styled(
            " Mempool ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        let msg = Paragraph::new(Line::from(Span::styled(
            " Node is syncing... Mempool data will be available once synced.",
            Style::default().fg(Color::Yellow),
        )))
        .block(block);
        frame.render_widget(msg, area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(0)])
        .split(area);

    render_summary(frame, chunks[0], app);
    render_table(frame, chunks[1], app);

    if let Some(ref detail) = app.mempool_detail {
        render_detail_popup(frame, area, detail);
    }
}

fn render_summary(frame: &mut Frame, area: Rect, app: &App) {
    let lines = if let Some(ref mempool) = app.mempool_state {
        let orphan_count = mempool.entries.iter().filter(|e| e.is_orphan).count();
        vec![
            Line::from(vec![
                Span::styled(" Total Entries:  ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    mempool.entry_count.to_string(),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled(" Orphans:        ", Style::default().fg(Color::DarkGray)),
                Span::raw(orphan_count.to_string()),
            ]),
            Line::from(vec![
                Span::styled(" Total Fees:     ", Style::default().fg(Color::DarkGray)),
                Span::raw(format!("{:.8} KAS", sompi_to_kas(mempool.total_fees))),
            ]),
        ]
    } else {
        vec![Line::from(Span::styled(
            " Waiting for mempool data...",
            Style::default().fg(Color::DarkGray),
        ))]
    };

    let block = Block::default().borders(Borders::ALL).title(Span::styled(
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

    // Calculate visible area height (area minus border top/bottom minus header minus header margin)
    let visible_rows = area.height.saturating_sub(4) as usize;
    let selected = app.mempool_selected;

    // Calculate scroll offset to keep selection visible
    let scroll_offset = if selected >= visible_rows {
        selected - visible_rows + 1
    } else {
        0
    };

    let rows: Vec<Row> = if let Some(ref mempool) = app.mempool_state {
        mempool
            .entries
            .iter()
            .enumerate()
            .skip(scroll_offset)
            .map(|(i, entry)| {
                let id = if entry.transaction_id.len() > 20 {
                    format!(
                        "{}...{}",
                        &entry.transaction_id[..10],
                        &entry.transaction_id[entry.transaction_id.len() - 10..]
                    )
                } else {
                    entry.transaction_id.clone()
                };

                let style = if i == selected {
                    Style::default().bg(Color::DarkGray).fg(Color::White)
                } else {
                    Style::default()
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
                .style(style)
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
        Block::default().borders(Borders::ALL).title(Span::styled(
            " Entries (↑↓ select, Enter details, Esc close) ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
    );

    frame.render_widget(table, area);
}

fn render_detail_popup(frame: &mut Frame, area: Rect, detail: &str) {
    let popup_width = area.width.min(60);
    let popup_height = area.height.min(10);
    let x = (area.width.saturating_sub(popup_width)) / 2 + area.x;
    let y = (area.height.saturating_sub(popup_height)) / 2 + area.y;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    frame.render_widget(Clear, popup_area);

    let popup = Paragraph::new(detail.to_string())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(
                    " Transaction Detail (Esc to close) ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ))
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(popup, popup_area);
}
