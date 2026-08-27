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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AuthConfig;

    #[test]
    fn test_resolve_board_number_prefers_override() {
        let config = Config {
            auth: AuthConfig {
                board_number: Some(483),
                ..AuthConfig::default()
            },
            ..Config::default()
        };

        assert_eq!(resolve_board_number(&config, Some(999)), Some(999));
    }

    #[test]
    fn test_resolve_board_number_uses_configured_default() {
        let config = Config {
            auth: AuthConfig {
                board_number: Some(483),
                ..AuthConfig::default()
            },
            ..Config::default()
        };

        assert_eq!(resolve_board_number(&config, None), Some(483));
    }
}
