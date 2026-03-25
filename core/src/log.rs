use std::path::Path;

use log::LevelFilter;
use log4rs::{
    append::console::ConsoleAppender,
    append::rolling_file::{
        RollingFileAppender,
        policy::compound::{
            CompoundPolicy, roll::fixed_window::FixedWindowRoller, trigger::size::SizeTrigger,
        },
    },
    config::{Appender, Config, Logger, Root},
    encode::pattern::PatternEncoder,
    filter::threshold::ThresholdFilter,
};

#[derive(Debug, Clone, Copy)]
pub enum LogTarget {
    Tui,
    Cli,
}

impl LogTarget {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogTarget::Tui => "tui",
            LogTarget::Cli => "cli",
        }
    }

    /// Whether this target should also log to stdout.
    /// TUI uses the terminal for rendering, so stdout is off.
    fn stdout_enabled(&self) -> bool {
        match self {
            LogTarget::Tui => false,
            LogTarget::Cli => true,
        }
    }
}

fn log_dir() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".tui4kas")
        .join("logs")
}

fn create_stdout_appender() -> ConsoleAppender {
    let pattern = "{d(%Y-%m-%d %H:%M:%S%.3f %Z)} [{h({l})}] - {m} (({f}:{L})){n}";
    ConsoleAppender::builder()
        .encoder(Box::new(PatternEncoder::new(pattern)))
        .build()
}

fn create_rolling_file_appender(
    base_path: &Path,
    pattern: &str,
) -> anyhow::Result<RollingFileAppender> {
    let log_file_path = base_path.with_extension("log");
    let roll_pattern = format!("{}.{{}}.log", base_path.to_string_lossy());

    let roller = FixedWindowRoller::builder()
        .base(1)
        .build(&roll_pattern, 5)?;

    let trigger = SizeTrigger::new(10 * 1024 * 1024);
    let policy = CompoundPolicy::new(Box::new(trigger), Box::new(roller));

    Ok(RollingFileAppender::builder()
        .encoder(Box::new(PatternEncoder::new(pattern)))
        .build(log_file_path.to_str().unwrap(), Box::new(policy))?)
}

fn setup_target_appender(
    target: LogTarget,
    log_dir: &Path,
    pattern: &str,
) -> anyhow::Result<Appender> {
    let base_path = log_dir.join(target.as_str());
    let rolling_file = create_rolling_file_appender(&base_path, pattern)?;

    Ok(Appender::builder()
        .filter(Box::new(ThresholdFilter::new(LevelFilter::Info)))
        .build(target.as_str(), Box::new(rolling_file)))
}

/// Initialize logging for the given target (Tui or Cli).
///
/// - Logs to `~/.tui4kas/logs/{target}.log` with 10 MB rotation (5 files).
/// - CLI also logs to stdout; TUI logs to file only (stdout would corrupt the terminal).
pub fn init_logger(target: LogTarget, log_level: LevelFilter) -> anyhow::Result<()> {
    let file_pattern = "{d(%Y-%m-%d %H:%M:%S%.3f %Z)} - {h({l})} - {m} (({f}:{L})){n}";
    let log_dir = log_dir();
    std::fs::create_dir_all(&log_dir)?;

    let target_appender = setup_target_appender(target, &log_dir, file_pattern)?;

    let mut logger_builder = Logger::builder().appender(target.as_str());

    let mut config_builder = Config::builder().appender(target_appender);

    if target.stdout_enabled() {
        let stdout = create_stdout_appender();
        config_builder =
            config_builder.appender(Appender::builder().build("stdout", Box::new(stdout)));
        logger_builder = logger_builder.appender("stdout");
    }

    let target_logger = logger_builder
        .additive(false)
        .build(target.as_str(), log_level);

    let config = config_builder
        .logger(target_logger)
        .build(Root::builder().build(log_level))?;

    log4rs::init_config(config)?;

    Ok(())
}
