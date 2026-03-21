use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::app::App;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(12), Constraint::Min(0)])
        .split(area);

    render_metrics(frame, chunks[0], app);
    render_tips(frame, chunks[1], app);
}

fn render_metrics(frame: &mut Frame, area: Rect, app: &App) {
    let lines = if let Some(ref dag) = app.dag_info {
        vec![
            Line::from(vec![
                Span::styled(" Network:          ", Style::default().fg(Color::Gray)),
                Span::raw(&dag.network),
            ]),
            Line::from(vec![
                Span::styled(" Block Count:      ", Style::default().fg(Color::Gray)),
                Span::raw(dag.block_count.to_string()),
            ]),
            Line::from(vec![
                Span::styled(" Header Count:     ", Style::default().fg(Color::Gray)),
                Span::raw(dag.header_count.to_string()),
            ]),
            Line::from(vec![
                Span::styled(" Difficulty:       ", Style::default().fg(Color::Gray)),
                Span::raw(format!("{:.4}", dag.difficulty)),
            ]),
            Line::from(vec![
                Span::styled(" DAA Score:        ", Style::default().fg(Color::Gray)),
                Span::raw(dag.virtual_daa_score.to_string()),
            ]),
            Line::from(vec![
                Span::styled(" Past Median Time: ", Style::default().fg(Color::Gray)),
                Span::raw(dag.past_median_time.to_string()),
            ]),
            Line::from(vec![
                Span::styled(" Pruning Point:    ", Style::default().fg(Color::Gray)),
                Span::raw(truncate_hash(&dag.pruning_point_hash)),
            ]),
            Line::from(vec![
                Span::styled(" Sink:             ", Style::default().fg(Color::Gray)),
                Span::raw(truncate_hash(&dag.sink)),
            ]),
            Line::from(vec![
                Span::styled(" Tips Count:       ", Style::default().fg(Color::Gray)),
                Span::raw(dag.tip_hashes.len().to_string()),
            ]),
            Line::from(vec![
                Span::styled(" Virtual Parents:  ", Style::default().fg(Color::Gray)),
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

    // Tip hashes
    let tip_items: Vec<ListItem> = if let Some(ref dag) = app.dag_info {
        dag.tip_hashes
            .iter()
            .skip(app.dag_scroll)
            .map(|h| ListItem::new(Line::from(Span::styled(h.as_str(), Style::default().fg(Color::White)))))
            .collect()
    } else {
        vec![]
    };

    let tips_list = List::new(tip_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(
                " Tip Hashes ",
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
            .map(|h| ListItem::new(Line::from(Span::styled(h.as_str(), Style::default().fg(Color::White)))))
            .collect()
    } else {
        vec![]
    };

    let parents_list = List::new(parent_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(
                " Virtual Parent Hashes ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
    );

    frame.render_widget(parents_list, chunks[1]);
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
