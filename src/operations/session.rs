use anyhow::Result;

use crate::client::HteamClient;
use crate::config::Config;

/// Bundles a loaded config with an authenticated client — the pair every
/// command-line, TUI and MCP entrypoint used to reconstruct by hand via
/// `Config::load()?; HteamClient::with_auth(config).await?;`.
pub struct Session {
    pub config: Config,
    pub client: HteamClient,
}

impl Session {
    pub async fn open() -> Result<Self> {
        let config = Config::load()?;
        let client = HteamClient::with_auth(config.clone()).await?;
        Ok(Self { config, client })
    }
}

/// Resolves which board number to operate on: an explicit override first,
/// then the configured default, then the last board touched by `hteam tui`
/// or `board switch` (`~/.last_ticket`). This is the one fallback chain used
/// everywhere board resolution matters.
pub fn resolve_board_number(config: &Config, override_board: Option<u64>) -> Option<u64> {
    override_board
        .or(config.auth.board_number)
        .or_else(|| config.load_last_ticket().ok().flatten())
}
