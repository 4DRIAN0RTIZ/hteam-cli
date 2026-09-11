use anyhow::Result;

use crate::client::{HteamClient, UpdateCardPatch};
use crate::models::{Card, CardDetail, Label, List};

pub async fn list_lists(client: &HteamClient, board: Option<u64>) -> Result<Vec<List>> {
    client.get_lists(board).await
}

pub async fn list_cards(
    client: &HteamClient,
    list_id: u64,
    board: Option<u64>,
) -> Result<(Vec<Card>, Option<u64>)> {
    client.get_cards(list_id, board).await
}

pub async fn open_cards(client: &HteamClient, board: Option<u64>) -> Result<Vec<Card>> {
    client.get_open_cards(board).await
}

pub async fn closed_cards(client: &HteamClient, board: Option<u64>) -> Result<Vec<Card>> {
    client.get_closed_cards(board).await
}

pub async fn card_detail(client: &HteamClient, card_id: u64) -> Result<CardDetail> {
    client.get_card_detail(card_id).await
}

/// URL pública de la página de detalle de una card (`/operations/{board}/tasks/{id}/`).
pub async fn card_url(client: &HteamClient, card_id: u64, board: Option<u64>) -> Result<String> {
    client.card_url(card_id, board).await
}

pub async fn create_card(client: &HteamClient, name: &str, list_id: Option<u64>) -> Result<Card> {
    client.create_card(name, list_id, None).await
}

/// Moves a card, defaulting the source list to 1 (Open) when not given — the
/// CLI, TUI and MCP server all move cards from the Open list by default.
pub async fn move_card(
    client: &HteamClient,
    card_id: u64,
    from_list: Option<u64>,
    to_list: u64,
    board: Option<u64>,
) -> Result<()> {
    client
        .move_card(card_id, from_list.unwrap_or(1), to_list, board)
        .await
}

pub async fn update_description(
    client: &HteamClient,
    card_id: u64,
    description: &str,
) -> Result<()> {
    client.update_card_description(card_id, description).await
}

pub async fn card_labels(
    client: &HteamClient,
    card_id: u64,
    board: Option<u64>,
) -> Result<Vec<Label>> {
    client.get_card_labels(card_id, board).await
}

/// Updates name/description/priority/responsible on a card. Fetches the
/// current detail first because `update_card_full` needs it as a patch base
/// — the same two-step sequence the CLI and MCP server each ran by hand.
pub async fn update_card(
    client: &HteamClient,
    card_id: u64,
    board: u64,
    name: Option<&str>,
    description: Option<&str>,
    priority: Option<&str>,
    responsible: Option<&str>,
) -> Result<()> {
    let detail = client.get_card_detail(card_id).await?;
    client
        .update_card_full(
            card_id,
            board,
            UpdateCardPatch {
                detail: &detail,
                name,
                description,
                priority,
                responsible,
            },
        )
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AuthConfig, Config};

    fn client(server_url: &str) -> HteamClient {
        let config = Config {
            auth: AuthConfig {
                board_number: Some(483),
                csrf_token: Some("csrf".to_string()),
                ..AuthConfig::default()
            },
            ..Config::default()
        };
        HteamClient::new_for_test(config, server_url.to_string(), server_url.to_string())
            .expect("test client")
    }

    #[tokio::test]
    async fn test_list_lists_delegates_to_client() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/operation/care/operations/483/lists/")
            .match_query(mockito::Matcher::UrlEncoded(
                "format".to_string(),
                "json".to_string(),
            ))
            .with_status(200)
            .with_body(r#"[{"id":1,"name":"Open","total_board_cards":2}]"#)
            .create_async()
            .await;

        let lists = list_lists(&client(&server.url()), Some(483))
            .await
            .expect("lists");

        assert_eq!(lists.len(), 1);
        assert_eq!(lists[0].name, "Open");
        assert_eq!(lists[0].card_count, Some(2));
    }

    #[tokio::test]
    async fn test_card_url_uses_explicit_board_over_configured_default() {
        let url = card_url(&client("https://hteam.mx"), 10784, Some(447))
            .await
            .expect("card url");

        assert_eq!(url, "https://hteam.mx/operations/447/tasks/10784/");
    }

    #[tokio::test]
    async fn test_card_url_falls_back_to_configured_board() {
        let url = card_url(&client("https://hteam.mx"), 10784, None)
            .await
            .expect("card url");

        assert_eq!(url, "https://hteam.mx/operations/483/tasks/10784/");
    }

    #[tokio::test]
    async fn test_list_cards_returns_cards_and_board_id() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/operation/care/operations/483/lists/1/cards/")
            .match_query(mockito::Matcher::UrlEncoded(
                "format".to_string(),
                "json".to_string(),
            ))
            .with_status(200)
            .with_body(
                r#"{"id":1,"name":"Open","cardlist_list":[{"id":10,"board_id":99,"card":{"id":42,"title":"Fix bug","subtitle":"desc","labels_list":[]},"is_closed":false,"card_type":0,"time_status":0,"get_time_status":"","position":1}]}"#,
            )
            .create_async()
            .await;

        let (cards, board_id) = list_cards(&client(&server.url()), 1, Some(483))
            .await
            .expect("cards");

        assert_eq!(board_id, Some(99));
        assert_eq!(cards[0].id, 42);
        assert_eq!(cards[0].name, "Fix bug");
    }

    #[tokio::test]
    async fn test_move_card_defaults_from_list_to_open() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("POST", "/boards/care/labels/update_card_position/")
            .match_body(mockito::Matcher::PartialJson(serde_json::json!({
                "board_id": 483,
                "card_id": 42,
                "from_list": 1,
                "to_list": 2,
                "card_type": 0
            })))
            .with_status(204)
            .create_async()
            .await;

        move_card(&client(&server.url()), 42, None, 2, Some(483))
            .await
            .expect("move card");
    }

    #[tokio::test]
    async fn test_update_card_fetches_detail_and_posts_patch() {
        let mut server = mockito::Server::new_async().await;
        let _detail = server
            .mock("GET", "/operation/care/tasks/42/")
            .match_query(mockito::Matcher::UrlEncoded(
                "format".to_string(),
                "json".to_string(),
            ))
            .with_status(200)
            .with_body(
                r#"{"id":42,"name":"Old","description":"old desc","labels":[],"list":null,"priority":3}"#,
            )
            .create_async()
            .await;
        let _update = server
            .mock("POST", "/operations/483/tasks/42/edit/")
            .match_body(mockito::Matcher::AllOf(vec![
                mockito::Matcher::Regex("name=New".to_string()),
                mockito::Matcher::Regex("description=new\\+desc".to_string()),
                mockito::Matcher::Regex("priority=2".to_string()),
                mockito::Matcher::Regex("id=42".to_string()),
            ]))
            .with_status(200)
            .create_async()
            .await;

        update_card(
            &client(&server.url()),
            42,
            483,
            Some("New"),
            Some("new desc"),
            Some("2"),
            None,
        )
        .await
        .expect("update card");
    }
}
