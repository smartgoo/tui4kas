use crate::rpc::types::*;

#[derive(Debug, Clone)]
pub struct DagVisualizerBlock {
    pub hash_short: String,
    pub is_selected_parent: bool,
}

#[derive(Debug, Clone)]
pub struct DagVisualizerColumn {
    pub blocks: Vec<DagVisualizerBlock>,
}

#[derive(Debug, Clone, Default)]
pub struct DagVisualizer {
    pub columns: Vec<DagVisualizerColumn>,
}

impl DagVisualizer {
    pub fn update(&mut self, tip_hashes: &[String], virtual_parents: &[String]) {
        let blocks: Vec<DagVisualizerBlock> = tip_hashes
            .iter()
            .map(|h| {
                let short = if h.len() >= 8 { &h[..8] } else { h };
                DagVisualizerBlock {
                    hash_short: short.to_string(),
                    is_selected_parent: virtual_parents.contains(h),
                }
            })
            .collect();

        if !blocks.is_empty() {
            // Only add if tips changed from last column
            let should_add = self.columns.last().is_none_or(|last| {
                let last_hashes: Vec<&str> = last.blocks.iter().map(|b| b.hash_short.as_str()).collect();
                let new_hashes: Vec<&str> = blocks.iter().map(|b| b.hash_short.as_str()).collect();
                last_hashes != new_hashes
            });

            if should_add {
                self.columns.push(DagVisualizerColumn { blocks });
                // Keep last 30 columns
                if self.columns.len() > 30 {
                    self.columns.remove(0);
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DagFocus {
    #[default]
    Tips,
    Parents,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Dashboard,
    Mempool,
    BlockDag,
    Analytics,
    RpcExplorer,
}

impl Tab {
    pub const ALL: [Tab; 5] = [Tab::Dashboard, Tab::Mempool, Tab::BlockDag, Tab::Analytics, Tab::RpcExplorer];

    pub fn title(&self) -> &'static str {
        match self {
            Tab::Dashboard => "Dashboard",
            Tab::Mempool => "Mempool",
            Tab::BlockDag => "BlockDAG",
            Tab::Analytics => "Analytics",
            Tab::RpcExplorer => "RPC Explorer",
        }
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Error(String),
}

pub struct RpcExplorerState {
    pub selected_method: usize,
    pub available_methods: Vec<&'static str>,
    pub last_response: Option<String>,
    pub is_loading: bool,
    pub scroll_offset: usize,
}

impl Default for RpcExplorerState {
    fn default() -> Self {
        Self {
            selected_method: 0,
            available_methods: vec![
                "get_server_info",
                "get_block_dag_info",
                "get_block_count",
                "get_mempool_entries",
                "get_coin_supply",
                "get_fee_estimate",
                "get_fee_estimate_experimental",
                "get_connected_peer_info",
                "get_peer_addresses",
                "get_current_network",
                "get_sink",
                "get_sink_blue_score",
                "get_info",
                "get_sync_status",
                "get_virtual_chain",
                "get_headers",
                "estimate_network_hashes_per_second",
                "ping",
            ],
            last_response: None,
            is_loading: false,
            scroll_offset: 0,
        }
    }
}

#[derive(Default)]
pub struct CommandLine {
    pub active: bool,
    pub input: String,
    pub cursor_pos: usize,
    pub output: Vec<CommandOutput>,
    pub output_scroll: usize,
    pub history: Vec<String>,
    pub history_index: Option<usize>,
    pub show_output: bool,
}

pub struct CommandOutput {
    pub command: String,
    pub result: String,
    pub is_error: bool,
}


impl CommandLine {
    pub fn activate(&mut self) {
        self.active = true;
        self.input.clear();
        self.cursor_pos = 0;
        self.history_index = None;
    }

    pub fn deactivate(&mut self) {
        self.active = false;
        self.input.clear();
        self.cursor_pos = 0;
        self.history_index = None;
    }

    pub fn insert_char(&mut self, c: char) {
        self.input.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
    }

    pub fn delete_char(&mut self) {
        if self.cursor_pos < self.input.len() {
            let next = self.next_char_boundary();
            self.input.drain(self.cursor_pos..next);
        }
    }

    pub fn backspace(&mut self) {
        if self.cursor_pos > 0 {
            let prev = self.prev_char_boundary();
            self.input.drain(prev..self.cursor_pos);
            self.cursor_pos = prev;
        }
    }

    pub fn move_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos = self.prev_char_boundary();
        }
    }

    pub fn move_right(&mut self) {
        if self.cursor_pos < self.input.len() {
            self.cursor_pos = self.next_char_boundary();
        }
    }

    fn prev_char_boundary(&self) -> usize {
        let mut pos = self.cursor_pos - 1;
        while pos > 0 && !self.input.is_char_boundary(pos) {
            pos -= 1;
        }
        pos
    }

    fn next_char_boundary(&self) -> usize {
        let mut pos = self.cursor_pos + 1;
        while pos < self.input.len() && !self.input.is_char_boundary(pos) {
            pos += 1;
        }
        pos
    }

    pub fn move_home(&mut self) {
        self.cursor_pos = 0;
    }

    pub fn move_end(&mut self) {
        self.cursor_pos = self.input.len();
    }

    pub fn history_up(&mut self) {
        if self.history.is_empty() {
            return;
        }
        match self.history_index {
            None => {
                self.history_index = Some(self.history.len() - 1);
            }
            Some(i) if i > 0 => {
                self.history_index = Some(i - 1);
            }
            _ => return,
        }
        if let Some(i) = self.history_index {
            self.input = self.history[i].clone();
            self.cursor_pos = self.input.len();
        }
    }

    pub fn history_down(&mut self) {
        match self.history_index {
            Some(i) if i < self.history.len() - 1 => {
                self.history_index = Some(i + 1);
                self.input = self.history[i + 1].clone();
                self.cursor_pos = self.input.len();
            }
            Some(_) => {
                self.history_index = None;
                self.input.clear();
                self.cursor_pos = 0;
            }
            None => {}
        }
    }

    pub fn submit(&mut self) -> Option<String> {
        let cmd = self.input.trim().to_string();
        if cmd.is_empty() {
            return None;
        }
        self.history.push(cmd.clone());
        if self.history.len() > 100 {
            self.history.remove(0);
        }
        self.history_index = None;
        self.input.clear();
        self.cursor_pos = 0;
        Some(cmd)
    }

    pub fn push_output(&mut self, command: String, result: String, is_error: bool) {
        self.output.push(CommandOutput {
            command,
            result,
            is_error,
        });
        if self.output.len() > 50 {
            self.output.remove(0);
        }
        self.output_scroll = 0;
        self.show_output = true;
    }

    pub fn available_commands() -> &'static [(&'static str, &'static str)] {
        &[
            ("help", "Show this help message"),
            ("clear", "Clear command output"),
            ("get_server_info", "Get server info"),
            ("get_block_dag_info", "Get block DAG info"),
            ("get_block_count", "Get block count"),
            ("get_mempool_entries", "Get mempool entries"),
            ("get_coin_supply", "Get coin supply"),
            ("get_fee_estimate", "Get fee estimate"),
            ("get_fee_estimate_experimental", "Get experimental fee estimate (verbose)"),
            ("get_connected_peer_info", "Get connected peer info"),
            ("get_peer_addresses", "Get known peer addresses"),
            ("get_current_network", "Get current network type"),
            ("get_sink", "Get sink (virtual selected parent) hash"),
            ("get_sink_blue_score", "Get sink blue score"),
            ("get_info", "Get general node info"),
            ("get_headers", "Get header count"),
            ("get_sync_status", "Get sync status"),
            ("get_virtual_chain", "Get virtual selected parent chain"),
            ("estimate_network_hashes_per_second", "Estimate network hashrate"),
            ("ping", "Ping the node"),
        ]
    }
}

pub struct App {
    pub active_tab: Tab,
    pub connection_status: ConnectionStatus,
    pub should_quit: bool,

    pub server_info: Option<ServerInfo>,
    pub dag_info: Option<DagInfo>,
    pub mempool_state: Option<MempoolState>,
    pub coin_supply: Option<CoinSupplyInfo>,
    pub fee_estimate: Option<FeeEstimateInfo>,
    pub market_data: Option<MarketData>,
    pub mining_info: Option<MiningInfo>,
    pub analytics: Option<AnalyticsData>,

    pub rpc_explorer: RpcExplorerState,
    pub command_line: CommandLine,

    pub node_url: Option<String>,
    pub node_uid: Option<String>,

    pub last_error: Option<String>,
    pub last_refresh: Option<std::time::Instant>,
    pub last_poll_duration_ms: Option<f64>,

    // Mempool table state
    pub mempool_selected: usize,
    pub mempool_detail: Option<String>,

    // BlockDAG selection state
    pub dag_focus: DagFocus,
    pub dag_tip_selected: usize,
    pub dag_parent_selected: usize,
    pub dag_block_detail: Option<String>,
    pub dag_block_loading: bool,

    // DAG visualizer state - tracks recent tip snapshots for rendering
    pub dag_visualizer: DagVisualizer,

    // Pause polling
    pub paused: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            active_tab: Tab::Dashboard,
            connection_status: ConnectionStatus::Disconnected,
            should_quit: false,
            server_info: None,
            dag_info: None,
            mempool_state: None,
            coin_supply: None,
            fee_estimate: None,
            market_data: None,
            mining_info: None,
            analytics: None,
            rpc_explorer: RpcExplorerState::default(),
            command_line: CommandLine::default(),
            node_url: None,
            node_uid: None,
            last_error: None,
            last_refresh: None,
            last_poll_duration_ms: None,
            mempool_selected: 0,
            mempool_detail: None,
            dag_focus: DagFocus::default(),
            dag_tip_selected: 0,
            dag_parent_selected: 0,
            dag_block_detail: None,
            dag_block_loading: false,
            dag_visualizer: DagVisualizer::default(),
            paused: false,
        }
    }

    pub fn tab_index(&self) -> usize {
        Tab::ALL.iter().position(|t| *t == self.active_tab).unwrap_or(0)
    }

    pub fn next_tab(&mut self) {
        let idx = (self.tab_index() + 1) % Tab::ALL.len();
        self.active_tab = Tab::ALL[idx];
    }

    pub fn prev_tab(&mut self) {
        let idx = if self.tab_index() == 0 {
            Tab::ALL.len() - 1
        } else {
            self.tab_index() - 1
        };
        self.active_tab = Tab::ALL[idx];
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;

    // --- Tab ---

    #[test]
    fn tab_titles() {
        assert_eq!(Tab::Dashboard.title(), "Dashboard");
        assert_eq!(Tab::Mempool.title(), "Mempool");
        assert_eq!(Tab::BlockDag.title(), "BlockDAG");
        assert_eq!(Tab::Analytics.title(), "Analytics");
        assert_eq!(Tab::RpcExplorer.title(), "RPC Explorer");
    }

    #[test]
    fn tab_index_matches_all_order() {
        let mut app = App::new();
        for (i, tab) in Tab::ALL.iter().enumerate() {
            app.active_tab = *tab;
            assert_eq!(app.tab_index(), i);
        }
    }

    #[test]
    fn next_tab_cycles_forward() {
        let mut app = App::new();
        assert_eq!(app.active_tab, Tab::Dashboard);
        app.next_tab();
        assert_eq!(app.active_tab, Tab::Mempool);
        app.next_tab();
        assert_eq!(app.active_tab, Tab::BlockDag);
        app.next_tab();
        assert_eq!(app.active_tab, Tab::Analytics);
        app.next_tab();
        assert_eq!(app.active_tab, Tab::RpcExplorer);
        app.next_tab();
        assert_eq!(app.active_tab, Tab::Dashboard); // wraps
    }

    #[test]
    fn prev_tab_cycles_backward() {
        let mut app = App::new();
        app.prev_tab();
        assert_eq!(app.active_tab, Tab::RpcExplorer); // wraps from 0
        app.prev_tab();
        assert_eq!(app.active_tab, Tab::Analytics);
        app.prev_tab();
        assert_eq!(app.active_tab, Tab::BlockDag);
    }

    // --- CommandLine: editing ---

    #[test]
    fn insert_char_ascii() {
        let mut cl = CommandLine::default();
        cl.insert_char('a');
        cl.insert_char('b');
        assert_eq!(cl.input, "ab");
        assert_eq!(cl.cursor_pos, 2);
    }

    #[test]
    fn insert_char_utf8_emoji() {
        let mut cl = CommandLine::default();
        cl.insert_char('🦀');
        assert_eq!(cl.input, "🦀");
        assert_eq!(cl.cursor_pos, 4); // 🦀 is 4 bytes
        cl.insert_char('!');
        assert_eq!(cl.input, "🦀!");
    }

    #[test]
    fn insert_char_mid_string() {
        let mut cl = CommandLine::default();
        cl.input = "ac".to_string();
        cl.cursor_pos = 1;
        cl.insert_char('b');
        assert_eq!(cl.input, "abc");
        assert_eq!(cl.cursor_pos, 2);
    }

    #[test]
    fn delete_char_at_cursor() {
        let mut cl = CommandLine::default();
        cl.input = "abc".to_string();
        cl.cursor_pos = 1;
        cl.delete_char();
        assert_eq!(cl.input, "ac");
        assert_eq!(cl.cursor_pos, 1);
    }

    #[test]
    fn delete_char_at_end_noop() {
        let mut cl = CommandLine::default();
        cl.input = "abc".to_string();
        cl.cursor_pos = 3;
        cl.delete_char();
        assert_eq!(cl.input, "abc");
    }

    #[test]
    fn delete_char_empty_noop() {
        let mut cl = CommandLine::default();
        cl.delete_char();
        assert_eq!(cl.input, "");
    }

    #[test]
    fn backspace_removes_previous() {
        let mut cl = CommandLine::default();
        cl.input = "abc".to_string();
        cl.cursor_pos = 2;
        cl.backspace();
        assert_eq!(cl.input, "ac");
        assert_eq!(cl.cursor_pos, 1);
    }

    #[test]
    fn backspace_at_start_noop() {
        let mut cl = CommandLine::default();
        cl.input = "abc".to_string();
        cl.cursor_pos = 0;
        cl.backspace();
        assert_eq!(cl.input, "abc");
        assert_eq!(cl.cursor_pos, 0);
    }

    #[test]
    fn backspace_utf8() {
        let mut cl = CommandLine::default();
        cl.input = "a🦀b".to_string();
        cl.cursor_pos = 5; // after 🦀
        cl.backspace();
        assert_eq!(cl.input, "ab");
        assert_eq!(cl.cursor_pos, 1);
    }

    // --- CommandLine: cursor movement ---

    #[test]
    fn move_left_right() {
        let mut cl = CommandLine::default();
        cl.input = "abc".to_string();
        cl.cursor_pos = 2;
        cl.move_left();
        assert_eq!(cl.cursor_pos, 1);
        cl.move_right();
        assert_eq!(cl.cursor_pos, 2);
    }

    #[test]
    fn move_left_at_start_noop() {
        let mut cl = CommandLine::default();
        cl.input = "abc".to_string();
        cl.cursor_pos = 0;
        cl.move_left();
        assert_eq!(cl.cursor_pos, 0);
    }

    #[test]
    fn move_right_at_end_noop() {
        let mut cl = CommandLine::default();
        cl.input = "abc".to_string();
        cl.cursor_pos = 3;
        cl.move_right();
        assert_eq!(cl.cursor_pos, 3);
    }

    #[test]
    fn move_left_right_utf8() {
        let mut cl = CommandLine::default();
        cl.input = "a🦀b".to_string();
        cl.cursor_pos = 5; // after 🦀
        cl.move_left();
        assert_eq!(cl.cursor_pos, 1); // before 🦀
        cl.move_right();
        assert_eq!(cl.cursor_pos, 5); // after 🦀
    }

    #[test]
    fn move_home_end() {
        let mut cl = CommandLine::default();
        cl.input = "hello".to_string();
        cl.cursor_pos = 3;
        cl.move_home();
        assert_eq!(cl.cursor_pos, 0);
        cl.move_end();
        assert_eq!(cl.cursor_pos, 5);
    }

    // --- CommandLine: history ---

    #[test]
    fn history_up_empty() {
        let mut cl = CommandLine::default();
        cl.history_up();
        assert_eq!(cl.history_index, None);
        assert_eq!(cl.input, "");
    }

    #[test]
    fn history_up_navigates() {
        let mut cl = CommandLine::default();
        cl.history = vec!["first".to_string(), "second".to_string()];
        cl.history_up();
        assert_eq!(cl.input, "second");
        assert_eq!(cl.history_index, Some(1));
        cl.history_up();
        assert_eq!(cl.input, "first");
        assert_eq!(cl.history_index, Some(0));
        // At top, stays
        cl.history_up();
        assert_eq!(cl.input, "first");
        assert_eq!(cl.history_index, Some(0));
    }

    #[test]
    fn history_down_restores_empty() {
        let mut cl = CommandLine::default();
        cl.history = vec!["cmd".to_string()];
        cl.history_up();
        assert_eq!(cl.input, "cmd");
        cl.history_down();
        assert_eq!(cl.input, "");
        assert_eq!(cl.history_index, None);
    }

    #[test]
    fn history_down_without_history_noop() {
        let mut cl = CommandLine::default();
        cl.input = "typing".to_string();
        cl.history_down();
        assert_eq!(cl.input, "typing");
    }

    // --- CommandLine: submit ---

    #[test]
    fn submit_empty_returns_none() {
        let mut cl = CommandLine::default();
        cl.input = "   ".to_string();
        assert!(cl.submit().is_none());
    }

    #[test]
    fn submit_returns_trimmed_command() {
        let mut cl = CommandLine::default();
        cl.input = "  ping  ".to_string();
        let cmd = cl.submit();
        assert_eq!(cmd, Some("ping".to_string()));
        assert_eq!(cl.input, "");
        assert_eq!(cl.cursor_pos, 0);
        assert_eq!(cl.history_index, None);
    }

    #[test]
    fn submit_appends_to_history() {
        let mut cl = CommandLine::default();
        cl.input = "get_server_info".to_string();
        cl.submit();
        assert_eq!(cl.history, vec!["get_server_info"]);
    }

    #[test]
    fn submit_caps_history_at_100() {
        let mut cl = CommandLine::default();
        for i in 0..105 {
            cl.input = format!("cmd{}", i);
            cl.submit();
        }
        assert_eq!(cl.history.len(), 100);
        assert_eq!(cl.history[0], "cmd5");
        assert_eq!(cl.history[99], "cmd104");
    }

    // --- CommandLine: output ---

    #[test]
    fn push_output_caps_at_50() {
        let mut cl = CommandLine::default();
        for i in 0..55 {
            cl.push_output(format!("cmd{}", i), "ok".to_string(), false);
        }
        assert_eq!(cl.output.len(), 50);
        assert_eq!(cl.output[0].command, "cmd5");
    }

    #[test]
    fn push_output_resets_scroll() {
        let mut cl = CommandLine::default();
        cl.output_scroll = 10;
        cl.push_output("test".to_string(), "result".to_string(), false);
        assert_eq!(cl.output_scroll, 0);
        assert!(cl.show_output);
    }

    #[test]
    fn push_output_tracks_errors() {
        let mut cl = CommandLine::default();
        cl.push_output("bad".to_string(), "fail".to_string(), true);
        assert!(cl.output[0].is_error);
    }

    // --- CommandLine: activate/deactivate ---

    #[test]
    fn activate_clears_state() {
        let mut cl = CommandLine::default();
        cl.input = "leftover".to_string();
        cl.cursor_pos = 5;
        cl.history_index = Some(2);
        cl.activate();
        assert!(cl.active);
        assert_eq!(cl.input, "");
        assert_eq!(cl.cursor_pos, 0);
        assert_eq!(cl.history_index, None);
    }

    #[test]
    fn deactivate_clears_state() {
        let mut cl = CommandLine::default();
        cl.active = true;
        cl.input = "something".to_string();
        cl.deactivate();
        assert!(!cl.active);
        assert_eq!(cl.input, "");
        assert_eq!(cl.cursor_pos, 0);
    }

    // --- RpcExplorerState ---

    #[test]
    fn rpc_explorer_default_has_all_methods() {
        let state = RpcExplorerState::default();
        assert!(state.available_methods.len() >= 18);
        assert!(state.available_methods.contains(&"ping"));
        assert!(state.available_methods.contains(&"get_server_info"));
        assert!(state.available_methods.contains(&"get_sink"));
        assert!(state.available_methods.contains(&"get_sink_blue_score"));
        assert!(state.available_methods.contains(&"get_info"));
        assert!(state.available_methods.contains(&"get_peer_addresses"));
        assert!(state.available_methods.contains(&"get_current_network"));
        assert!(state.available_methods.contains(&"get_fee_estimate_experimental"));
        assert!(state.available_methods.contains(&"estimate_network_hashes_per_second"));
    }

    // --- DagVisualizer ---

    #[test]
    fn dag_visualizer_starts_empty() {
        let vis = DagVisualizer::default();
        assert!(vis.columns.is_empty());
    }

    #[test]
    fn dag_visualizer_adds_column() {
        let mut vis = DagVisualizer::default();
        let tips = vec!["abc123".to_string(), "def456".to_string()];
        let parents = vec!["abc123".to_string()];
        vis.update(&tips, &parents);
        assert_eq!(vis.columns.len(), 1);
        assert_eq!(vis.columns[0].blocks.len(), 2);
        assert!(vis.columns[0].blocks[0].is_selected_parent);
        assert!(!vis.columns[0].blocks[1].is_selected_parent);
    }

    #[test]
    fn dag_visualizer_skips_duplicate() {
        let mut vis = DagVisualizer::default();
        let tips = vec!["abc12345".to_string()];
        let parents = vec![];
        vis.update(&tips, &parents);
        vis.update(&tips, &parents);
        assert_eq!(vis.columns.len(), 1);
    }

    #[test]
    fn dag_visualizer_caps_at_30() {
        let mut vis = DagVisualizer::default();
        for i in 0..35 {
            let tips = vec![format!("hash{:04}", i)];
            vis.update(&tips, &[]);
        }
        assert_eq!(vis.columns.len(), 30);
    }

    #[test]
    fn rpc_explorer_methods_match_available_commands() {
        let state = RpcExplorerState::default();
        let commands = CommandLine::available_commands();
        // Every explorer method should have a corresponding command entry
        for method in &state.available_methods {
            assert!(
                commands.iter().any(|(name, _)| name == method),
                "Explorer method '{}' missing from available_commands",
                method
            );
        }
    }
}
