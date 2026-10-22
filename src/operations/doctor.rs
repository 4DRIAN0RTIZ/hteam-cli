use anyhow::Result;
use serde::Serialize;

use crate::client::HteamClient;
use crate::config::Config;
use crate::operations::session::{self, AuthCheck};

/// Severity of one doctor check. `Fail` means the CLI is not ready for
/// authenticated board work; `Warn` is recoverable friction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Ok,
    Warn,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DoctorCheck {
    pub id: &'static str,
    pub status: CheckStatus,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

/// Read-only diagnosis of local config + remote session. Never includes
/// cookie or token values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DoctorReport {
    /// `true` when no check has [`CheckStatus::Fail`].
    pub ok: bool,
    pub checks: Vec<DoctorCheck>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub board: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub board_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boards_visible: Option<usize>,
}

fn check(
    id: &'static str,
    status: CheckStatus,
    message: impl Into<String>,
    hint: Option<String>,
) -> DoctorCheck {
    DoctorCheck {
        id,
        status,
        message: message.into(),
        hint,
    }
}

fn finish(
    checks: Vec<DoctorCheck>,
    config_path: Option<String>,
    board: Option<u64>,
    board_name: Option<String>,
    boards_visible: Option<usize>,
) -> DoctorReport {
    let ok = checks.iter().all(|c| c.status != CheckStatus::Fail);
    DoctorReport {
        ok,
        checks,
        config_path,
        board,
        board_name,
        boards_visible,
    }
}

/// Inspect config, cookie presence, session validity, and board access.
/// Does not mutate remote state or local credentials.
pub async fn run(override_board: Option<u64>) -> Result<DoctorReport> {
    let mut checks = Vec::new();

    let config_path = Config::config_file().ok().map(|p| p.display().to_string());
    match Config::config_file() {
        Ok(p) if p.exists() => checks.push(check(
            "config_file",
            CheckStatus::Ok,
            format!("config found at {}", p.display()),
            None,
        )),
        Ok(p) => checks.push(check(
            "config_file",
            CheckStatus::Fail,
            format!("config file missing at {}", p.display()),
            Some("Run `hteam login` to create ~/.config/hteam/config.toml.".into()),
        )),
        Err(_) => checks.push(check(
            "config_file",
            CheckStatus::Fail,
            "could not resolve config path",
            Some("Check that a home/config directory is available.".into()),
        )),
    }

    let config = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            checks.push(check(
                "config_load",
                CheckStatus::Fail,
                format!("could not load config: {e}"),
                Some("Fix or remove the broken config.toml, then run `hteam login`.".into()),
            ));
            return Ok(finish(checks, config_path, None, None, None));
        }
    };

    let client = match HteamClient::new(config.clone()) {
        Ok(c) => Some(c),
        Err(e) => {
            checks.push(check(
                "client",
                CheckStatus::Fail,
                format!("could not build HTTP client: {e}"),
                None,
            ));
            None
        }
    };

    Ok(diagnose(config, override_board, client, checks, config_path).await)
}

async fn diagnose(
    config: Config,
    override_board: Option<u64>,
    client: Option<HteamClient>,
    mut checks: Vec<DoctorCheck>,
    config_path: Option<String>,
) -> DoctorReport {
    let mut board = None;
    let mut board_name = None;
    let mut boards_visible = None;

    let has_session = config
        .auth
        .session_id
        .as_ref()
        .is_some_and(|s| !s.is_empty());
    let has_csrf = config
        .auth
        .csrf_token
        .as_ref()
        .is_some_and(|s| !s.is_empty());

    match (has_session, has_csrf) {
        (true, true) => checks.push(check(
            "credentials",
            CheckStatus::Ok,
            "sessionid and csrftoken are present",
            None,
        )),
        (false, false) => checks.push(check(
            "credentials",
            CheckStatus::Fail,
            "sessionid and csrftoken are missing",
            Some("Run `hteam login` and paste the sessionid and csrftoken cookies.".into()),
        )),
        (false, true) => checks.push(check(
            "credentials",
            CheckStatus::Fail,
            "sessionid is missing",
            Some("Run `hteam login` and paste the sessionid cookie.".into()),
        )),
        (true, false) => checks.push(check(
            "credentials",
            CheckStatus::Fail,
            "csrftoken is missing",
            Some("Run `hteam login` and paste the csrftoken cookie.".into()),
        )),
    }

    if !has_session || !has_csrf {
        let resolved = session::resolve_board_number(&config, override_board);
        board = resolved;
        if let Some(id) = resolved {
            board_name = config.boards.get(&id.to_string()).map(|b| b.name.clone());
            checks.push(check(
                "board",
                CheckStatus::Warn,
                format!("board {id} is configured locally but session was not verified"),
                Some("Complete login first, then re-run `hteam doctor`.".into()),
            ));
        } else {
            checks.push(check(
                "board",
                CheckStatus::Warn,
                "no default board configured",
                Some(
                    "After login, run `hteam board switch <id>` or `hteam login --board <id>`."
                        .into(),
                ),
            ));
        }
        return finish(checks, config_path, board, board_name, boards_visible);
    }

    let Some(client) = client else {
        return finish(checks, config_path, board, board_name, boards_visible);
    };

    match session::check_auth(&client).await {
        Ok(AuthCheck::Ok { boards }) => {
            boards_visible = Some(boards.len());
            checks.push(check(
                "auth",
                CheckStatus::Ok,
                format!(
                    "session accepted ({} visible Execution board(s))",
                    boards.len()
                ),
                None,
            ));

            let resolved = session::resolve_board_number(&config, override_board);
            board = resolved;
            match resolved {
                None => {
                    checks.push(check(
                        "board",
                        CheckStatus::Warn,
                        "no default board configured",
                        Some("Run `hteam board switch <id>` or `hteam login --board <id>`.".into()),
                    ));
                    if !boards.is_empty() {
                        let preview: Vec<String> = boards
                            .iter()
                            .take(5)
                            .map(|b| format!("{} ({})", b.name, b.id))
                            .collect();
                        checks.push(check(
                            "board_list",
                            CheckStatus::Ok,
                            format!("visible boards: {}", preview.join(", ")),
                            None,
                        ));
                    }
                }
                Some(id) => match boards.iter().find(|b| b.id == id) {
                    Some(entry) => {
                        board_name = Some(entry.name.clone());
                        checks.push(check(
                            "board",
                            CheckStatus::Ok,
                            format!("board {} ({}) is visible to this session", entry.name, id),
                            None,
                        ));
                        match client.get_lists(Some(id)).await {
                            Ok(lists) => checks.push(check(
                                "board_access",
                                CheckStatus::Ok,
                                format!("lists endpoint OK ({} list(s))", lists.len()),
                                None,
                            )),
                            Err(e) => checks.push(check(
                                "board_access",
                                CheckStatus::Fail,
                                format!("cannot read lists for board {id}: {e}"),
                                Some(
                                    "Session is valid but this board is not reachable. Try another board with `hteam board switch <id>`."
                                        .into(),
                                ),
                            )),
                        }
                    }
                    None => {
                        board_name = config.boards.get(&id.to_string()).map(|b| b.name.clone());
                        checks.push(check(
                            "board",
                            CheckStatus::Fail,
                            format!(
                                "configured board {id} is not in your visible Execution list"
                            ),
                            Some(
                                "Run `hteam board switch <id>` with a board you can see, or `hteam login` again."
                                    .into(),
                            ),
                        ));
                    }
                },
            }
        }
        Ok(AuthCheck::Unauthorized) => {
            checks.push(check(
                "auth",
                CheckStatus::Fail,
                "session rejected (unauthorized)",
                Some(
                    "Cookies expired or invalid. Run `hteam login` with fresh sessionid and csrftoken from hteam.mx."
                        .into(),
                ),
            ));
            board = session::resolve_board_number(&config, override_board);
            if let Some(id) = board {
                board_name = config.boards.get(&id.to_string()).map(|b| b.name.clone());
            }
        }
        Err(e) => {
            checks.push(check(
                "auth",
                CheckStatus::Fail,
                format!("network or server error while probing session: {e}"),
                Some(
                    "Check connectivity to hteam.mx and retry. If the outage persists, try again later."
                        .into(),
                ),
            ));
            board = session::resolve_board_number(&config, override_board);
            if let Some(id) = board {
                board_name = config.boards.get(&id.to_string()).map(|b| b.name.clone());
            }
        }
    }

    finish(checks, config_path, board, board_name, boards_visible)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AuthConfig;
    use mockito::Matcher;

    async fn run_with_config(config: Config, server_url: &str) -> DoctorReport {
        let client = HteamClient::new_for_test(
            config.clone(),
            server_url.to_string(),
            server_url.to_string(),
        )
        .expect("client");
        diagnose(
            config,
            None,
            Some(client),
            vec![check(
                "config_file",
                CheckStatus::Ok,
                "config found at test",
                None,
            )],
            Some("/tmp/hteam-doctor-test.toml".into()),
        )
        .await
    }

    fn boards_body(id: u64, name: &str) -> String {
        format!(
            r#"{{"data":[{{"id":{id},"name":"{name}","service":"Managed","total_tasks":1,"total_tasks_closed":0}}]}}"#
        )
    }

    async fn mock_boards(server: &mut mockito::Server, status: usize, body: &str) -> mockito::Mock {
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

    #[tokio::test]
    async fn doctor_ok_when_session_and_board_visible() {
        let mut server = mockito::Server::new_async().await;
        let _boards = mock_boards(&mut server, 200, &boards_body(447, "AdrianBoard")).await;
        let _lists = server
            .mock("GET", "/operation/care/operations/447/lists/")
            .match_query(Matcher::UrlEncoded(
                "format".to_string(),
                "json".to_string(),
            ))
            .with_status(200)
            .with_body(r#"[{"id":1,"name":"Open","total_board_cards":0}]"#)
            .create_async()
            .await;

        let config = Config {
            auth: AuthConfig {
                session_id: Some("s".into()),
                csrf_token: Some("c".into()),
                board_number: Some(447),
                ..AuthConfig::default()
            },
            ..Config::default()
        };

        let report = run_with_config(config, &server.url()).await;
        assert!(report.ok, "{report:?}");
        assert_eq!(report.board, Some(447));
        assert_eq!(report.board_name.as_deref(), Some("AdrianBoard"));
        assert!(report
            .checks
            .iter()
            .any(|c| c.id == "auth" && c.status == CheckStatus::Ok));
        assert!(report
            .checks
            .iter()
            .any(|c| c.id == "board_access" && c.status == CheckStatus::Ok));

        let json = serde_json::to_string(&report).unwrap();
        // Cookie *values* must never appear; field names in prose are fine.
        assert!(!json.contains("\"s\""));
        assert!(!json.contains("\"c\""));
    }

    #[tokio::test]
    async fn doctor_fails_on_unauthorized() {
        let mut server = mockito::Server::new_async().await;
        let _m = mock_boards(&mut server, 403, "nope").await;

        let config = Config {
            auth: AuthConfig {
                session_id: Some("s".into()),
                csrf_token: Some("c".into()),
                board_number: Some(447),
                ..AuthConfig::default()
            },
            ..Config::default()
        };

        let report = run_with_config(config, &server.url()).await;
        assert!(!report.ok);
        let auth = report.checks.iter().find(|c| c.id == "auth").unwrap();
        assert_eq!(auth.status, CheckStatus::Fail);
        assert!(auth.message.contains("unauthorized"));
        assert!(auth.hint.as_ref().unwrap().contains("login"));
    }

    #[tokio::test]
    async fn doctor_fails_when_board_not_visible() {
        let mut server = mockito::Server::new_async().await;
        let _m = mock_boards(&mut server, 200, &boards_body(12, "Other")).await;

        let config = Config {
            auth: AuthConfig {
                session_id: Some("s".into()),
                csrf_token: Some("c".into()),
                board_number: Some(447),
                ..AuthConfig::default()
            },
            ..Config::default()
        };

        let report = run_with_config(config, &server.url()).await;
        assert!(!report.ok);
        let board = report.checks.iter().find(|c| c.id == "board").unwrap();
        assert_eq!(board.status, CheckStatus::Fail);
        assert!(board.message.contains("447"));
    }

    #[tokio::test]
    async fn doctor_fails_without_credentials() {
        let report = run_with_config(Config::default(), "http://127.0.0.1:9").await;
        assert!(!report.ok);
        assert!(report
            .checks
            .iter()
            .any(|c| c.id == "credentials" && c.status == CheckStatus::Fail));
    }

    #[test]
    fn finish_ok_only_without_fails() {
        assert!(
            finish(
                vec![check("a", CheckStatus::Ok, "x", None)],
                None,
                None,
                None,
                None
            )
            .ok
        );
        assert!(
            finish(
                vec![check("a", CheckStatus::Warn, "x", None)],
                None,
                None,
                None,
                None
            )
            .ok
        );
        assert!(
            !finish(
                vec![check("a", CheckStatus::Fail, "x", None)],
                None,
                None,
                None,
                None
            )
            .ok
        );
    }
}
