use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;

pub fn render_input(frame: &mut Frame, area: Rect, app: &App) {
    let (prompt_style, input_text, cursor_hint) = if app.command_line.active {
        (
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            app.command_line.input.as_str(),
            "",
        )
    } else {
        (
            Style::default().fg(Color::DarkGray),
            "",
            "Press ':' to enter a command, 'help' for available commands",
        )
    };

    let line = if app.command_line.active {
        Line::from(vec![
            Span::styled("> ", prompt_style),
            Span::styled(input_text, Style::default().fg(Color::White)),
        ])
    } else {
        Line::from(vec![
            Span::styled("> ", prompt_style),
            Span::styled(cursor_hint, Style::default().fg(Color::DarkGray)),
        ])
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(if app.command_line.active {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        })
        .title(Span::styled(
            " Command ",
            if app.command_line.active {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            },
        ));

    let paragraph = Paragraph::new(line).block(block);
    frame.render_widget(paragraph, area);

    // Show cursor when command line is active
    if app.command_line.active {
        // Count display characters up to cursor byte position
        let display_chars = app.command_line.input[..app.command_line.cursor_pos].chars().count();
        // +1 for border, +2 for "> " prompt
        let cursor_x = area.x + 1 + 2 + display_chars as u16;
        let cursor_y = area.y + 1;
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}

pub fn render_output(frame: &mut Frame, area: Rect, app: &App) {
    let mut lines: Vec<Line> = Vec::new();

    for entry in &app.command_line.output {
        lines.push(Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Cyan)),
            Span::styled(&entry.command, Style::default().fg(Color::Yellow)),
        ]));

        let result_style = if entry.is_error {
            Style::default().fg(Color::Red)
        } else {
            Style::default().fg(Color::White)
        };

        for result_line in entry.result.lines() {
            lines.push(Line::from(Span::styled(result_line, result_style)));
        }
        lines.push(Line::from(""));
    }

    let total_lines = lines.len();
    let visible_height = area.height.saturating_sub(2) as usize;
    let max_scroll = total_lines.saturating_sub(visible_height);
    let scroll = if app.command_line.output_scroll == 0 {
        max_scroll
    } else {
        max_scroll.saturating_sub(app.command_line.output_scroll)
    };

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(
                    " Output ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )),
        )
        .wrap(Wrap { trim: false })
        .scroll((scroll as u16, 0));

    frame.render_widget(paragraph, area);
}
