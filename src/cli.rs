use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(name = "tui4kas", version, about = "Terminal UI for Kaspa L1")]
pub struct CliArgs {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_no_args() {
        let _args = CliArgs::parse_from(["tui4kas"]);
    }
}
