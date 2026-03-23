use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use crate::app::{App, Tab};

pub fn render_help(frame: &mut Frame, area: Rect, app: &App) {
    let popup_width = (area.width * 70 / 100).clamp(40, 72);
    let popup_height = (area.height * 80 / 100).clamp(15, 40);
    let x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    frame.render_widget(Clear, popup_area);

    let key_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(Color::White);
    let section_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);

    let mut lines: Vec<Line> = Vec::new();

    // Global keys
    lines.push(Line::from(Span::styled("Global", section_style)));
    let global_keys = [
        ("1-7", "Switch to tab"),
        ("Tab/Shift+Tab", "Next/previous tab"),
        (":", "Open command line"),
        ("p", "Pause/resume polling"),
        ("?", "Toggle this help"),
        ("c / dbl-click", "Copy focused text"),
        ("q q", "Quit (press twice)"),
        ("Ctrl+C", "Quit immediately"),
        ("Esc", "Close overlay/popup"),
    ];
    for (key, desc) in &global_keys {
        lines.push(Line::from(vec![
            Span::styled(format!("  {:<18}", key), key_style),
            Span::styled(*desc, desc_style),
        ]));
    }

    lines.push(Line::from(""));

    // Tab-specific keys
    let (section_name, tab_keys): (&str, Vec<(&str, &str)>) = match app.active_tab {
        Tab::Dashboard => ("Dashboard", vec![("(no extra keys)", "Read-only display")]),
        Tab::Mining => (
            "Mining",
            vec![
                ("h/l or Left/Right", "Switch panel"),
                ("j/k or Up/Down", "Scroll list"),
                ("g/G or Home/End", "Jump to first/last"),
                ("o", "Open address in browser"),
            ],
        ),
        Tab::Mempool => (
            "Mempool",
            vec![
                ("j/k or Up/Down", "Select transaction"),
                ("g/G or Home/End", "Jump to first/last"),
                ("Enter", "Show transaction detail"),
                ("o", "Open transaction in browser"),
                ("Esc", "Close detail popup"),
            ],
        ),
        Tab::BlockDag => (
            "BlockDAG",
            vec![
                ("h/l or Left/Right", "Switch tips/parents focus"),
                ("j/k or Up/Down", "Select item"),
                ("g/G or Home/End", "Jump to first/last"),
                ("Enter", "Show block info"),
                ("o", "Open block in browser"),
                ("Esc", "Close block popup"),
            ],
        ),
        Tab::Analytics => (
            "Analytics",
            vec![
                ("h/j/k/l or Arrows", "Navigate panels"),
                ("v", "Toggle table/chart view"),
                ("t", "Cycle time window (1m/1h/24h)"),
                ("Esc", "Dismiss notification"),
            ],
        ),
        Tab::RpcExplorer => (
            "RPC Explorer",
            vec![
                ("Up/Down", "Select RPC method"),
                ("Enter", "Execute method"),
                ("j/k", "Scroll response"),
                ("J/K", "Scroll fast (10 lines)"),
                ("PgUp/PgDn", "Scroll page (20 lines)"),
                ("g/G or Home/End", "Jump to top/bottom"),
            ],
        ),
        Tab::Settings => {
            if app.settings.editing {
                (
                    "Settings (Editing)",
                    vec![
                        ("Enter", "Save value"),
                        ("Esc", "Cancel editing"),
                        ("Backspace", "Delete character"),
                    ],
                )
            } else {
                (
                    "Settings",
                    vec![
                        ("Up/Down", "Navigate fields"),
                        ("Enter", "Edit/cycle value"),
                        ("Left/Right", "Cycle network"),
                        ("r", "Reload config"),
                    ],
                )
            }
        }
    };

    lines.push(Line::from(Span::styled(
        format!("{} (current tab)", section_name),
        section_style,
    )));
    for (key, desc) in &tab_keys {
        lines.push(Line::from(vec![
            Span::styled(format!("  {:<18}", key), key_style),
            Span::styled(*desc, desc_style),
        ]));
    }

    lines.push(Line::from(""));

    // Command line section
    lines.push(Line::from(Span::styled("Command Line", section_style)));
    let cmd_keys = [
        ("Esc", "Close command line"),
        ("Enter", "Execute command"),
        ("Up/Down", "Command history"),
        ("Left/Right", "Move cursor"),
        ("Home/End", "Start/end of line"),
    ];
    for (key, desc) in &cmd_keys {
        lines.push(Line::from(vec![
            Span::styled(format!("  {:<18}", key), key_style),
            Span::styled(*desc, desc_style),
        ]));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(
            " Help (? or Esc to close) ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));

    let paragraph = Paragraph::new(lines).block(block).wrap(Wrap { trim: false });

    frame.render_widget(paragraph, popup_area);
}
