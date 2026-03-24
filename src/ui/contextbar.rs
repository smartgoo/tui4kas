use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::{App, Tab};

fn context_hint(app: &App) -> String {
    if app.quit_confirm {
        return "Press q again to quit, any other key to cancel".to_string();
    }

    if app.mempool_detail.is_some() {
        return "Esc close popup | ? help".to_string();
    }

    match app.active_tab {
        Tab::Dashboard => "Tab/1-6 switch | p pause | ? help".to_string(),
        Tab::Mining => "h/l panel | j/k scroll | g/G top/bottom | ? help".to_string(),
        Tab::Mempool => "j/k select | Enter detail | g/G top/bottom | ? help".to_string(),
        Tab::Analytics => "Tab/1-6 switch | p pause | ? help".to_string(),
        Tab::RpcExplorer => "Up/Down method | Enter exec | j/k scroll | ? help".to_string(),
        Tab::Settings => {
            if app.settings.editing {
                "Enter save | Esc cancel".to_string()
            } else {
                "Up/Down nav | Enter edit/cycle | r reload | ? help".to_string()
            }
        }
    }
}

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let hint = context_hint(app);
    let hint_style = if app.quit_confirm {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let line = Line::from(vec![Span::styled(hint, hint_style)]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let paragraph = Paragraph::new(line).block(block);
    frame.render_widget(paragraph, area);
}
