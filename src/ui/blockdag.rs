use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, DagFocus};

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),   // visualizer
            Constraint::Length(12),  // metrics
            Constraint::Min(0),     // tips/parents
        ])
        .split(area);

    render_visualizer(frame, chunks[0], app);
    render_metrics(frame, chunks[1], app);
    render_tips(frame, chunks[2], app);

    if app.dag_block_loading {
        render_loading_popup(frame, area);
    } else if let Some(ref detail) = app.dag_block_detail {
        render_block_popup(frame, area, detail);
    }
}

fn render_visualizer(frame: &mut Frame, area: Rect, app: &App) {
    let vis = &app.dag_visualizer;
    let inner_width = area.width.saturating_sub(2) as usize; // borders
    let inner_height = area.height.saturating_sub(2) as usize; // borders

    let mut lines: Vec<Line> = Vec::new();

    if vis.columns.is_empty() {
        lines.push(Line::from(Span::styled(
            " Collecting DAG data...",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        // Each column takes ~10 chars (8 hash + 2 spacing)
        let col_width = 10;
        let max_cols = inner_width / col_width;
        let start = vis.columns.len().saturating_sub(max_cols);
        let visible_cols = &vis.columns[start..];

        // Find max block count per column for row layout
        let max_blocks = visible_cols.iter().map(|c| c.blocks.len()).max().unwrap_or(0);
        let display_rows = max_blocks.min(inner_height);

        for row in 0..display_rows {
            let mut spans: Vec<Span> = Vec::new();
            spans.push(Span::raw(" "));
            for col in visible_cols {
                if row < col.blocks.len() {
                    let block = &col.blocks[row];
                    let style = if block.is_selected_parent {
                        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    };
                    let symbol = if block.is_selected_parent { "◆" } else { "◇" };
                    spans.push(Span::styled(
                        format!("{}{} ", symbol, &block.hash_short[..block.hash_short.len().min(7)]),
                        style,
                    ));
                } else {
                    spans.push(Span::raw("          "));
                }
            }
            lines.push(Line::from(spans));
        }

        // Add connection line between columns
        if display_rows > 0 && visible_cols.len() > 1 {
            let mut conn_spans: Vec<Span> = Vec::new();
            conn_spans.push(Span::raw(" "));
            for _ in 0..visible_cols.len() {
                conn_spans.push(Span::styled("────────→ ", Style::default().fg(Color::DarkGray)));
            }
            lines.push(Line::from(conn_spans));
        }
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " DAG Visualizer (◆ = selected parent chain) ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_metrics(frame: &mut Frame, area: Rect, app: &App) {
    let lines = if let Some(ref dag) = app.dag_info {
        vec![
            Line::from(vec![
                Span::styled(" Network:          ", Style::default().fg(Color::DarkGray)),
                Span::raw(&dag.network),
            ]),
            Line::from(vec![
                Span::styled(" Block Count:      ", Style::default().fg(Color::DarkGray)),
                Span::raw(dag.block_count.to_string()),
            ]),
            Line::from(vec![
                Span::styled(" Header Count:     ", Style::default().fg(Color::DarkGray)),
                Span::raw(dag.header_count.to_string()),
            ]),
            Line::from(vec![
                Span::styled(" Difficulty:       ", Style::default().fg(Color::DarkGray)),
                Span::raw(format!("{:.4}", dag.difficulty)),
            ]),
            Line::from(vec![
                Span::styled(" DAA Score:        ", Style::default().fg(Color::DarkGray)),
                Span::raw(dag.virtual_daa_score.to_string()),
            ]),
            Line::from(vec![
                Span::styled(" Past Median Time: ", Style::default().fg(Color::DarkGray)),
                Span::raw(dag.past_median_time.to_string()),
            ]),
            Line::from(vec![
                Span::styled(" Pruning Point:    ", Style::default().fg(Color::DarkGray)),
                Span::raw(truncate_hash(&dag.pruning_point_hash)),
            ]),
            Line::from(vec![
                Span::styled(" Sink:             ", Style::default().fg(Color::DarkGray)),
                Span::raw(truncate_hash(&dag.sink)),
            ]),
            Line::from(vec![
                Span::styled(" Tips Count:       ", Style::default().fg(Color::DarkGray)),
                Span::raw(dag.tip_hashes.len().to_string()),
            ]),
            Line::from(vec![
                Span::styled(" Virtual Parents:  ", Style::default().fg(Color::DarkGray)),
                Span::raw(dag.virtual_parent_hashes.len().to_string()),
            ]),
        ]
    } else {
        vec![Line::from(Span::styled(
            " Waiting for DAG data...",
            Style::default().fg(Color::DarkGray),
        ))]
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
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

    let tips_focused = app.dag_focus == DagFocus::Tips;
    let parents_focused = app.dag_focus == DagFocus::Parents;

    // Tip hashes
    let tip_items: Vec<ListItem> = if let Some(ref dag) = app.dag_info {
        dag.tip_hashes
            .iter()
            .enumerate()
            .map(|(i, h)| {
                let style = if tips_focused && i == app.dag_tip_selected {
                    Style::default().bg(Color::DarkGray).fg(Color::White)
                } else {
                    Style::default().fg(Color::White)
                };
                ListItem::new(Line::from(Span::styled(h.as_str(), style)))
            })
            .collect()
    } else {
        vec![]
    };

    let tips_border = if tips_focused { Color::Cyan } else { Color::White };
    let tips_list = List::new(tip_items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(tips_border))
            .title(Span::styled(
                " Tip Hashes (←→ switch, ↑↓ select, Enter info) ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
    );

    frame.render_widget(tips_list, chunks[0]);

    // Virtual parent hashes
    let parent_items: Vec<ListItem> = if let Some(ref dag) = app.dag_info {
        dag.virtual_parent_hashes
            .iter()
            .enumerate()
            .map(|(i, h)| {
                let style = if parents_focused && i == app.dag_parent_selected {
                    Style::default().bg(Color::DarkGray).fg(Color::White)
                } else {
                    Style::default().fg(Color::White)
                };
                ListItem::new(Line::from(Span::styled(h.as_str(), style)))
            })
            .collect()
    } else {
        vec![]
    };

    let parents_border = if parents_focused { Color::Cyan } else { Color::White };
    let parents_list = List::new(parent_items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(parents_border))
            .title(Span::styled(
                " Virtual Parent Hashes ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
    );

    frame.render_widget(parents_list, chunks[1]);
}

fn render_loading_popup(frame: &mut Frame, area: Rect) {
    let popup_width = 30;
    let popup_height = 3;
    let x = (area.width.saturating_sub(popup_width)) / 2 + area.x;
    let y = (area.height.saturating_sub(popup_height)) / 2 + area.y;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    frame.render_widget(Clear, popup_area);
    let popup = Paragraph::new(" Loading block info...")
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Yellow)));
    frame.render_widget(popup, popup_area);
}

fn render_block_popup(frame: &mut Frame, area: Rect, detail: &str) {
    let popup_width = area.width.saturating_sub(10).min(80);
    let popup_height = area.height.saturating_sub(6).min(30);
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

fn truncate_hash(hash: &str) -> String {
    if hash.len() > 24 {
        format!("{}...{}", &hash[..12], &hash[hash.len() - 12..])
    } else {
        hash.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_hash_short() {
        assert_eq!(truncate_hash("abcdef"), "abcdef");
    }

    #[test]
    fn truncate_hash_exactly_24() {
        let hash = "a".repeat(24);
        assert_eq!(truncate_hash(&hash), hash);
    }

    #[test]
    fn truncate_hash_long() {
        let hash = "abcdefghijklmnopqrstuvwxyz0123456789";
        let result = truncate_hash(hash);
        // first 12 + "..." + last 12
        assert_eq!(result, "abcdefghijkl...yz0123456789");
        assert_eq!(result.len(), 27);
    }

    #[test]
    fn truncate_hash_empty() {
        assert_eq!(truncate_hash(""), "");
    }

    #[test]
    fn truncate_hash_realistic() {
        let hash = "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890";
        let result = truncate_hash(hash);
        assert_eq!(result.len(), 27); // 12 + 3 + 12
        assert!(result.starts_with("abcdef123456"));
        assert!(result.ends_with("ef1234567890"));
        assert!(result.contains("..."));
    }
}
