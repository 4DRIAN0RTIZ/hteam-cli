use anyhow::Result;

use crate::client::{AuthProbe, HteamClient};
use crate::config::{BoardInfo, Config};
use crate::models::BoardEntry;

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

/// Outcome of validating session cookies without tying auth to a board id.
/// Network / unexpected HTTP errors bubble as `Err` from [`check_auth`].
#[derive(Debug, Clone)]
pub enum AuthCheck {
    Ok { boards: Vec<BoardEntry> },
    Unauthorized,
}

/// How login should pick a default board after a successful auth probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginBoardResolution {
    /// Board id to persist as the active default.
    Selected { id: u64, name: String },
    /// Session is fine; caller should point the user at `hteam board switch`.
    NeedsSwitch { boards: Vec<(u64, String)> },
    /// Cookies work but the account has no Execution boards.
    NoneAvailable,
    /// `--board` was set but that id is not in the visible list.
    NotFound { requested: u64 },
}

/// Probes the session via the boards list endpoint (no hardcoded board).
pub async fn check_auth(client: &HteamClient) -> Result<AuthCheck> {
    match client.probe_auth().await? {
        AuthProbe::Authorized(boards) => Ok(AuthCheck::Ok { boards }),
        AuthProbe::Unauthorized => Ok(AuthCheck::Unauthorized),
    }
}

/// Prefer `--board`, then a still-visible configured board, then auto-pick
/// when there is exactly one board; otherwise leave board selection to the user.
pub fn resolve_login_board(
    boards: &[BoardEntry],
    explicit_board: Option<u64>,
    configured_board: Option<u64>,
) -> LoginBoardResolution {
    if let Some(requested) = explicit_board {
        return match boards.iter().find(|b| b.id == requested) {
            Some(board) => LoginBoardResolution::Selected {
                id: board.id,
                name: board.name.clone(),
            },
            None => LoginBoardResolution::NotFound { requested },
        };
    }

    if let Some(configured) = configured_board {
        if let Some(board) = boards.iter().find(|b| b.id == configured) {
            return LoginBoardResolution::Selected {
                id: board.id,
                name: board.name.clone(),
            };
        }
    }

    match boards {
        [] => LoginBoardResolution::NoneAvailable,
        [only] => LoginBoardResolution::Selected {
            id: only.id,
            name: only.name.clone(),
        },
        many => LoginBoardResolution::NeedsSwitch {
            boards: many.iter().map(|b| (b.id, b.name.clone())).collect(),
        },
    }
}

/// Writes session cookies (and optional default board) after a successful auth check.
/// Callers must not invoke this when [`AuthCheck::Unauthorized`].
pub fn persist_login(
    config: &mut Config,
    session_id: String,
    csrf_token: String,
    board: Option<(u64, String)>,
) -> Result<()> {
    config.auth.session_id = Some(session_id);
    config.auth.csrf_token = Some(csrf_token);

    if let Some((board_id, board_name)) = board {
        config.set_board_number(board_id);
        Config::save_last_ticket(board_id)?;
        config.boards.insert(
            board_id.to_string(),
            BoardInfo {
                name: board_name,
                last_used: Some(chrono::Utc::now().to_rfc3339()),
            },
        );
    }

    config.save()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AuthConfig;
    use crate::models::BoardEntry;
    use mockito::Matcher;

    fn entry(id: u64, name: &str) -> BoardEntry {
        BoardEntry {
            id,
            name: name.to_string(),
            service: String::new(),
            responsible: String::new(),
            total_tasks: 0,
            total_tasks_closed: 0,
        }
    }

    fn client(server_url: &str, config: Config) -> HteamClient {
        HteamClient::new_for_test(config, server_url.to_string(), server_url.to_string())
            .expect("test client")
    }

    async fn boards_mock(server: &mut mockito::Server, status: usize, body: &str) -> mockito::Mock {
        server
            .mock("GET", "/operation/care/operations/")
            .match_query(Matcher::AllOf(vec![
                Matcher::UrlEncoded("format".to_string(), "datatables".to_string()),
                Matcher::UrlEncoded("status".to_string(), "Execution".to_string()),
            ]))
            .with_status(status)
            .with_body(body)
            .create_async()
            .await
    }

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

    #[test]
    fn resolve_login_board_prefers_explicit() {
        let boards = vec![entry(1, "A"), entry(2, "B")];
        assert_eq!(
            resolve_login_board(&boards, Some(2), Some(1)),
            LoginBoardResolution::Selected {
                id: 2,
                name: "B".into(),
            }
        );
    }

    #[test]
    fn resolve_login_board_keeps_configured_when_visible() {
        let boards = vec![entry(1, "A"), entry(2, "B")];
        assert_eq!(
            resolve_login_board(&boards, None, Some(2)),
            LoginBoardResolution::Selected {
                id: 2,
                name: "B".into(),
            }
        );
    }

    #[test]
    fn resolve_login_board_auto_picks_single() {
        let boards = vec![entry(9, "Only")];
        assert_eq!(
            resolve_login_board(&boards, None, Some(483)),
            LoginBoardResolution::Selected {
                id: 9,
                name: "Only".into(),
            }
        );
    }

    #[test]
    fn resolve_login_board_needs_switch_when_many_and_config_stale() {
        let boards = vec![entry(1, "A"), entry(2, "B")];
        match resolve_login_board(&boards, None, Some(999)) {
            LoginBoardResolution::NeedsSwitch { boards } => {
                assert_eq!(boards, vec![(1, "A".into()), (2, "B".into())]);
            }
            other => panic!("expected NeedsSwitch, got {other:?}"),
        }
    }

    #[test]
    fn resolve_login_board_not_found_for_missing_explicit() {
        let boards = vec![entry(1, "A")];
        assert_eq!(
            resolve_login_board(&boards, Some(99), None),
            LoginBoardResolution::NotFound { requested: 99 }
        );
    }

    #[test]
    fn resolve_login_board_none_when_empty() {
        assert_eq!(
            resolve_login_board(&[], None, Some(1)),
            LoginBoardResolution::NoneAvailable
        );
    }

    #[tokio::test]
    async fn check_auth_ok_returns_boards() {
        let mut server = mockito::Server::new_async().await;
        let _m = boards_mock(
            &mut server,
            200,
            r#"{"data":[{"id":5,"name":"Ops","service":"Managed","total_tasks":1,"total_tasks_closed":0}]}"#,
        )
        .await;

        let result = check_auth(&client(&server.url(), Config::default()))
            .await
            .expect("check");
        match result {
            AuthCheck::Ok { boards } => {
                assert_eq!(boards.len(), 1);
                assert_eq!(boards[0].id, 5);
            }
            AuthCheck::Unauthorized => panic!("expected ok"),
        }
    }

    #[tokio::test]
    async fn check_auth_maps_forbidden_to_unauthorized() {
        let mut server = mockito::Server::new_async().await;
        let _m = boards_mock(&mut server, 403, "nope").await;

        let result = check_auth(&client(&server.url(), Config::default()))
            .await
            .expect("check");
        assert!(matches!(result, AuthCheck::Unauthorized));
    }
}
