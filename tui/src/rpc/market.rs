use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tui4kas_core::price::fetch_market_data;

use crate::app::App;

pub fn start_market_polling(app_state: Arc<RwLock<App>>, interval: Duration) {
    tokio::spawn(async move {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent("tui4kas")
            .build()
            .unwrap();
        let mut ticker = tokio::time::interval(interval);
        loop {
            ticker.tick().await;
            if app_state.read().await.paused {
                continue;
            }
            if let Ok(data) = fetch_market_data(Some(&client)).await {
                let mut app = app_state.write().await;
                app.market_data = Some(data);
                app.dirty = true;
            }
        }
    });
}
