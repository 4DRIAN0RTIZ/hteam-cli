use anyhow::Result;

use crate::client::HteamClient;
use crate::config::{BoardInfo, Config};
use crate::models::BoardEntry;

/// Live boards from the API — used by the TUI's board-switch popup ('B').
pub async fn list_live(client: &HteamClient) -> Result<Vec<BoardEntry>> {
    client.get_boards().await
}

/// Switches the active board: updates `config.toml`, persists the
/// last-ticket file, and upserts the board's display name in the boards
/// map. Returns the resolved display name.
pub fn switch_board(config: &mut Config, board_id: u64, name: Option<String>) -> Result<String> {
    config.set_board_number(board_id);
    Config::save_last_ticket(board_id)?;

    let board_name = name.unwrap_or_else(|| {
        config
            .boards
            .get(&board_id.to_string())
            .map(|b| b.name.clone())
            .unwrap_or_else(|| format!("Board {}", board_id))
    });

    config.boards.insert(
        board_id.to_string(),
        BoardInfo {
            name: board_name.clone(),
            last_used: Some(chrono::Utc::now().to_rfc3339()),
        },
    );

    config.save()?;
    Ok(board_name)
}

/// The currently configured board id and its display name, if any.
pub fn current_board(config: &Config) -> Option<(u64, String)> {
    let id = config.get_board_number()?;
    let name = config
        .boards
        .get(&id.to_string())
        .map(|b| b.name.clone())
        .unwrap_or_else(|| "Desconocido".to_string());
    Some((id, name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Matcher;

    fn client(server_url: &str) -> HteamClient {
        HteamClient::new_for_test(
            Config::default(),
            server_url.to_string(),
            server_url.to_string(),
        )
        .expect("test client")
    }

    #[tokio::test]
    async fn test_list_live_returns_datatable_boards() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/operation/care/operations/")
            .match_query(Matcher::AllOf(vec![
                Matcher::UrlEncoded("format".to_string(), "datatables".to_string()),
                Matcher::UrlEncoded("status".to_string(), "Execution".to_string()),
            ]))
            .with_status(200)
            .with_body(
                r#"{"data":[{"id":483,"name":"Ops","service":"Managed","total_tasks":10,"total_tasks_closed":3}]}"#,
            )
            .create_async()
            .await;

        let boards = list_live(&client(&server.url())).await.expect("boards");

        assert_eq!(boards.len(), 1);
        assert_eq!(boards[0].id, 483);
        assert_eq!(boards[0].name, "Ops");
    }

    #[test]
    fn test_current_board_returns_configured_name() {
        let mut config = Config::default();
        config.set_board_number(483);
        config.boards.insert(
            "483".to_string(),
            BoardInfo {
                name: "Ops".to_string(),
                last_used: None,
            },
        );

        assert_eq!(current_board(&config), Some((483, "Ops".to_string())));
    }

    #[test]
    fn test_current_board_uses_placeholder_for_unknown_name() {
        let mut config = Config::default();
        config.set_board_number(483);

        assert_eq!(
            current_board(&config),
            Some((483, "Desconocido".to_string()))
        );
    }
}
