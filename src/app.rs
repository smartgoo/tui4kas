use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Instant;

use crate::analytics::{AggregatedView, AnalyticsEngine};
use crate::config::AppConfig;
use crate::rpc::types::*;

#[derive(Debug, Clone)]
pub struct DagSample {
    pub timestamp: Instant,
    pub blue_score: u64,
}

#[derive(Debug, Clone, Default)]
pub struct DagStats {
    pub samples: VecDeque<DagSample>,
    pub sink_blue_score: Option<u64>,
}

impl DagStats {
    pub fn update(&mut self, blue_score: Option<u64>) {
        self.sink_blue_score = blue_score;
        self.samples.push_back(DagSample {
            timestamp: Instant::now(),
            blue_score: blue_score.unwrap_or(0),
        });
        while self.samples.len() > 120 {
            self.samples.pop_front();
        }
    }

    pub fn blue_block_rate(&self) -> Option<f64> {
        if self.samples.len() < 2 {
            return None;
        }
        let first = self.samples.front()?;
        let last = self.samples.back()?;
        let elapsed = last.timestamp.duration_since(first.timestamp).as_secs_f64();
        if elapsed < 0.1 {
            return None;
        }
        let delta = last.blue_score.saturating_sub(first.blue_score) as f64;
        Some(delta / elapsed)
    }

    pub fn block_interval_ms(&self) -> Option<f64> {
        if self.samples.len() < 2 {
            return None;
        }
        let first = self.samples.front()?;
        let last = self.samples.back()?;
        let delta = last.blue_score.saturating_sub(first.blue_score);
        if delta == 0 {
            return None;
        }
        let elapsed_ms = last.timestamp.duration_since(first.timestamp).as_secs_f64() * 1000.0;
        Some(elapsed_ms / delta as f64)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Dashboard,
    Mining,
    Mempool,
    Analytics,
    RpcExplorer,
    Settings,
}

impl Tab {
    pub fn all() -> &'static [Tab] {
        &[
            Tab::Dashboard,
            Tab::Mining,
            Tab::Mempool,
            Tab::Analytics,
            Tab::RpcExplorer,
            Tab::Settings,
        ]
    }

    pub fn title(&self) -> &'static str {
        match self {
            Tab::Dashboard => "1:Dashboard",
            Tab::Mining => "2:Mining",
            Tab::Mempool => "3:Mempool",
            Tab::Analytics => "4:Analytics",
            Tab::RpcExplorer => "5:RPC Cmds",
            Tab::Settings => "6:Settings",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DashboardPanel {
    #[default]
    NodeInfo,
    NetworkStats,
    Markets,
    MempoolFees,
}

impl DashboardPanel {
    pub fn move_left(self) -> Self {
        match self {
            Self::NetworkStats => Self::NodeInfo,
            Self::MempoolFees => Self::Markets,
            other => other,
        }
    }

    pub fn move_right(self) -> Self {
        match self {
            Self::NodeInfo => Self::NetworkStats,
            Self::Markets => Self::MempoolFees,
            other => other,
        }
    }

    pub fn move_up(self) -> Self {
        match self {
            Self::Markets => Self::NodeInfo,
            Self::MempoolFees => Self::NetworkStats,
            other => other,
        }
    }

    pub fn move_down(self) -> Self {
        match self {
            Self::NodeInfo => Self::Markets,
            Self::NetworkStats => Self::MempoolFees,
            other => other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RpcExplorerPanel {
    #[default]
    Methods,
    Response,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MiningPanel {
    #[default]
    Miners,
    Pools,
    Versions,
}

impl MiningPanel {
    pub fn next(self) -> Self {
        match self {
            MiningPanel::Versions => MiningPanel::Pools,
            MiningPanel::Pools => MiningPanel::Miners,
            MiningPanel::Miners => MiningPanel::Versions,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            MiningPanel::Versions => MiningPanel::Miners,
            MiningPanel::Pools => MiningPanel::Versions,
            MiningPanel::Miners => MiningPanel::Pools,
        }
    }
}

pub struct MiningTabState {
    pub active_panel: MiningPanel,
    pub miners_selected: usize,
    pub pools_selected: usize,
    pub versions_selected: usize,
    pub block_count: usize,
}

impl Default for MiningTabState {
    fn default() -> Self {
        Self {
            active_panel: MiningPanel::default(),
            miners_selected: 0,
            pools_selected: 0,
            versions_selected: 0,
            block_count: 1000,
        }
    }
}

impl MiningTabState {
    pub fn selected_mut(&mut self) -> &mut usize {
        match self.active_panel {
            MiningPanel::Miners => &mut self.miners_selected,
            MiningPanel::Pools => &mut self.pools_selected,
            MiningPanel::Versions => &mut self.versions_selected,
        }
    }

    pub fn cycle_block_count(&mut self) {
        self.block_count = match self.block_count {
            100 => 1000,
            1000 => 10000,
            _ => 100,
        };
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    #[default]
    Table,
    Chart,
}

impl ViewMode {
    pub fn toggle(&mut self) {
        *self = match self {
            Self::Table => Self::Chart,
            Self::Chart => Self::Table,
        };
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TimeWindow {
    #[default]
    OneMin,
    FifteenMin,
    ThirtyMin,
    OneHour,
    SixHour,
    TwelveHour,
    TwentyFourHour,
}

impl TimeWindow {
    pub fn cycle(&mut self) {
        *self = match self {
            Self::OneMin => Self::FifteenMin,
            Self::FifteenMin => Self::ThirtyMin,
            Self::ThirtyMin => Self::OneHour,
            Self::OneHour => Self::SixHour,
            Self::SixHour => Self::TwelveHour,
            Self::TwelveHour => Self::TwentyFourHour,
            Self::TwentyFourHour => Self::OneMin,
        };
    }

    pub fn seconds(&self) -> f64 {
        match self {
            Self::OneMin => 60.0,
            Self::FifteenMin => 900.0,
            Self::ThirtyMin => 1800.0,
            Self::OneHour => 3600.0,
            Self::SixHour => 21600.0,
            Self::TwelveHour => 43200.0,
            Self::TwentyFourHour => 86400.0,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::OneMin => "1m",
            Self::FifteenMin => "15m",
            Self::ThirtyMin => "30m",
            Self::OneHour => "1h",
            Self::SixHour => "6h",
            Self::TwelveHour => "12h",
            Self::TwentyFourHour => "24h",
        }
    }
}

pub struct SettingsState {
    pub config: AppConfig,
    pub selected_field: usize,
    pub editing: bool,
    pub edit_buffer: String,
    pub status_message: Option<(String, bool)>, // (message, is_error)
}

impl SettingsState {
    pub fn new(config: AppConfig) -> Self {
        Self {
            config,
            selected_field: 0,
            editing: false,
            edit_buffer: String::new(),
            status_message: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Error(#[allow(dead_code)] String),
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
            available_methods: crate::rpc::types::RPC_METHODS
                .iter()
                .map(|(name, _)| *name)
                .collect(),
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
    pub output: VecDeque<CommandOutput>,
    pub output_scroll: usize,
    pub history: VecDeque<String>,
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
        self.history.push_back(cmd.clone());
        if self.history.len() > 100 {
            self.history.pop_front();
        }
        self.history_index = None;
        self.input.clear();
        self.cursor_pos = 0;
        Some(cmd)
    }

    pub fn push_output(&mut self, command: String, result: String, is_error: bool) {
        self.output.push_back(CommandOutput {
            command,
            result,
            is_error,
        });
        if self.output.len() > 50 {
            self.output.pop_front();
        }
        self.output_scroll = 0;
        self.show_output = true;
    }

    pub fn available_commands() -> Vec<(&'static str, &'static str)> {
        let mut cmds = vec![
            ("help", "Show this help message"),
            ("clear", "Clear command output"),
        ];
        cmds.extend_from_slice(crate::rpc::types::RPC_METHODS);
        cmds
    }
}

pub struct NodeState {
    pub server_info: Option<ServerInfo>,
    pub dag_info: Option<DagInfo>,
    pub mempool_state: Option<MempoolState>,
    pub coin_supply: Option<CoinSupplyInfo>,
    pub fee_estimate: Option<FeeEstimateInfo>,
    pub mining_info: Option<MiningInfo>,
    pub dag_stats: DagStats,
    pub sink_blue_score: Option<u64>,
    pub node_url: Option<String>,
    pub node_uid: Option<String>,
    pub connection_status: ConnectionStatus,
    pub last_poll_duration_ms: Option<f64>,
}

impl Default for NodeState {
    fn default() -> Self {
        Self {
            server_info: None,
            dag_info: None,
            mempool_state: None,
            coin_supply: None,
            fee_estimate: None,
            mining_info: None,
            dag_stats: DagStats::default(),
            sink_blue_score: None,
            node_url: None,
            node_uid: None,
            connection_status: ConnectionStatus::Disconnected,
            last_poll_duration_ms: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AnalyticsSyncProgress {
    pub start_daa: u64,
    pub last_daa: u64,
    pub tip_daa: u64,
    pub from_pruning_point: bool,
}

pub const ANALYTICS_PANEL_COUNT: usize = 6;

#[derive(Default)]
pub struct AnalyticsState {
    pub engine: Option<Arc<tokio::sync::RwLock<AnalyticsEngine>>>,
    pub focus: usize,
    pub view_modes: [ViewMode; ANALYTICS_PANEL_COUNT],
    pub time_windows: [TimeWindow; ANALYTICS_PANEL_COUNT],
    pub sync_progress: Option<AnalyticsSyncProgress>,
    pub reorg_notification: Option<String>,
    pub cached_views: Option<[AggregatedView; ANALYTICS_PANEL_COUNT]>,
}

pub struct App {
    pub active_tab: Tab,
    pub should_quit: bool,

    pub node: NodeState,
    pub analytics: AnalyticsState,
    pub market_data: Option<MarketData>,

    pub rpc_explorer: RpcExplorerState,
    pub command_line: CommandLine,

    pub mempool_selected: usize,
    pub mempool_detail: Option<String>,

    pub paused: bool,
    pub show_help: bool,
    pub quit_confirm: bool,
    pub dirty: bool,

    pub dashboard_panel: DashboardPanel,
    pub rpc_explorer_panel: RpcExplorerPanel,
    pub mining_tab: MiningTabState,
    pub settings: SettingsState,

    pub last_click: Option<Instant>,
    pub clipboard_flash: Option<String>,
}

impl App {
    pub fn new(config: AppConfig) -> Self {
        Self {
            active_tab: Tab::Dashboard,
            should_quit: false,
            node: NodeState::default(),
            analytics: AnalyticsState::default(),
            market_data: None,
            rpc_explorer: RpcExplorerState::default(),
            command_line: CommandLine::default(),
            mempool_selected: 0,
            mempool_detail: None,
            dashboard_panel: DashboardPanel::default(),
            rpc_explorer_panel: RpcExplorerPanel::default(),
            paused: false,
            show_help: false,
            quit_confirm: false,
            dirty: true,
            mining_tab: MiningTabState::default(),
            settings: SettingsState::new(config),
            last_click: None,
            clipboard_flash: None,
        }
    }

    pub fn has_direct_url(&self) -> bool {
        self.settings.config.url.is_some()
    }

    pub fn tab_index(&self) -> usize {
        Tab::all()
            .iter()
            .position(|t| *t == self.active_tab)
            .unwrap_or(0)
    }

    pub fn next_tab(&mut self) {
        let idx = (self.tab_index() + 1) % Tab::all().len();
        self.active_tab = Tab::all()[idx];
    }

    pub fn prev_tab(&mut self) {
        let idx = if self.tab_index() == 0 {
            Tab::all().len() - 1
        } else {
            self.tab_index() - 1
        };
        self.active_tab = Tab::all()[idx];
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;

    // --- Tab ---

    #[test]
    fn tab_titles() {
        assert_eq!(Tab::Dashboard.title(), "1:Dashboard");
        assert_eq!(Tab::Mining.title(), "2:Mining");
        assert_eq!(Tab::Mempool.title(), "3:Mempool");
        assert_eq!(Tab::Analytics.title(), "4:Analytics");
        assert_eq!(Tab::RpcExplorer.title(), "5:RPC Cmds");
        assert_eq!(Tab::Settings.title(), "6:Settings");
    }

    #[test]
    fn tab_index_matches_all_order() {
        let mut app = App::new(AppConfig::default());
        for (i, tab) in Tab::all().iter().enumerate() {
            app.active_tab = *tab;
            assert_eq!(app.tab_index(), i);
        }
    }

    #[test]
    fn next_tab_cycles_forward() {
        let mut app = App::new(AppConfig::default());
        assert_eq!(app.active_tab, Tab::Dashboard);
        app.next_tab();
        assert_eq!(app.active_tab, Tab::Mining);
        app.next_tab();
        assert_eq!(app.active_tab, Tab::Mempool);
        app.next_tab();
        assert_eq!(app.active_tab, Tab::Analytics);
        app.next_tab();
        assert_eq!(app.active_tab, Tab::RpcExplorer);
        app.next_tab();
        // With daemon feature, next is IntegratedNode; without, wraps to Dashboard
        let last = *Tab::all().last().unwrap();
        if last == Tab::RpcExplorer {
            assert_eq!(app.active_tab, Tab::Dashboard); // wraps
        } else {
            assert_eq!(app.active_tab, last);
            app.next_tab();
            assert_eq!(app.active_tab, Tab::Dashboard); // wraps
        }
    }

    #[test]
    fn prev_tab_cycles_backward() {
        let mut app = App::new(AppConfig::default());
        app.prev_tab();
        let last = *Tab::all().last().unwrap();
        assert_eq!(app.active_tab, last); // wraps from 0
        // Navigate back a couple
        app.active_tab = Tab::Analytics;
        app.prev_tab();
        assert_eq!(app.active_tab, Tab::Mempool);
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
        cl.history = VecDeque::from(vec!["first".to_string(), "second".to_string()]);
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
        cl.history = VecDeque::from(vec!["cmd".to_string()]);
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
        assert_eq!(
            cl.history,
            VecDeque::from(vec!["get_server_info".to_string()])
        );
    }

    #[test]
    fn submit_caps_history_at_100() {
        let mut cl = CommandLine::default();
        for i in 0..105 {
            cl.input = format!("cmd{}", i);
            cl.submit();
        }
        assert_eq!(cl.history.len(), 100);
        assert_eq!(cl.history.front().unwrap(), "cmd5");
        assert_eq!(cl.history.back().unwrap(), "cmd104");
    }

    // --- CommandLine: output ---

    #[test]
    fn push_output_caps_at_50() {
        let mut cl = CommandLine::default();
        for i in 0..55 {
            cl.push_output(format!("cmd{}", i), "ok".to_string(), false);
        }
        assert_eq!(cl.output.len(), 50);
        assert_eq!(cl.output.front().unwrap().command, "cmd5");
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
        assert!(cl.output.front().unwrap().is_error);
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
        assert!(
            state
                .available_methods
                .contains(&"get_fee_estimate_experimental")
        );
        assert!(
            state
                .available_methods
                .contains(&"estimate_network_hashes_per_second")
        );
    }

    // --- DagStats ---

    #[test]
    fn dag_stats_default_empty() {
        let stats = DagStats::default();
        assert!(stats.samples.is_empty());
        assert!(stats.sink_blue_score.is_none());
        assert!(stats.blue_block_rate().is_none());
        assert!(stats.block_interval_ms().is_none());
    }

    #[test]
    fn dag_stats_update_adds_sample() {
        let mut stats = DagStats::default();
        stats.update(Some(500));
        assert_eq!(stats.samples.len(), 1);
        assert_eq!(stats.sink_blue_score, Some(500));
    }

    #[test]
    fn dag_stats_caps_at_120() {
        let mut stats = DagStats::default();
        for _ in 0..130 {
            stats.update(Some(500));
        }
        assert_eq!(stats.samples.len(), 120);
    }

    #[test]
    fn dag_stats_blue_block_rate_needs_two_samples() {
        let mut stats = DagStats::default();
        stats.update(Some(500));
        assert!(stats.blue_block_rate().is_none());
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
