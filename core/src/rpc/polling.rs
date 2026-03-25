/// Tracks cancellable background polling tasks.
#[derive(Default)]
pub struct PollingHandles {
    pub core: Option<tokio::task::JoinHandle<()>>,
    pub mining: Option<tokio::task::JoinHandle<()>>,
    pub analytics: Option<tokio::task::JoinHandle<()>>,
}

impl PollingHandles {
    pub fn abort_all(&mut self) {
        for handle in [&mut self.core, &mut self.mining, &mut self.analytics] {
            if let Some(h) = handle.take() {
                h.abort();
            }
        }
    }
}
