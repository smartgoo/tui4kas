use crossterm::event::{self, Event, KeyEvent, MouseEvent};
use std::time::Duration;
use tokio::sync::mpsc;

#[allow(dead_code)]
pub enum AppEvent {
    Tick,
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),
}

pub struct EventHandler {
    rx: mpsc::UnboundedReceiver<AppEvent>,
}

impl EventHandler {
    pub fn new(tick_rate: Duration) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();

        std::thread::spawn(move || {
            loop {
                if event::poll(tick_rate).unwrap_or(false) {
                    match event::read() {
                        Ok(Event::Key(key))
                            if tx.send(AppEvent::Key(key)).is_err() => {
                                return;
                            }
                        Ok(Event::Mouse(mouse))
                            if tx.send(AppEvent::Mouse(mouse)).is_err() => {
                                return;
                            }
                        Ok(Event::Resize(w, h))
                            if tx.send(AppEvent::Resize(w, h)).is_err() => {
                                return;
                            }
                        _ => {}
                    }
                } else if tx.send(AppEvent::Tick).is_err() {
                    return;
                }
            }
        });

        Self { rx }
    }

    pub async fn next(&mut self) -> Option<AppEvent> {
        self.rx.recv().await
    }
}
