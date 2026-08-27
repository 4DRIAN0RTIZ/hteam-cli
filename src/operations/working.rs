use anyhow::Result;

use crate::client::HteamClient;
use crate::models::WorkingOnStatus;

pub async fn list_working(client: &HteamClient) -> Result<Vec<WorkingOnStatus>> {
    client.get_working_on().await
}

pub async fn start_working(client: &HteamClient, card_id: u64) -> Result<()> {
    client.start_working(card_id).await
}

pub async fn stop_working(client: &HteamClient, working_id: u64) -> Result<()> {
    client.stop_working(working_id).await
}

/// The "working on it" record id for `card_id` within `working_on`, if any —
/// this is what `stop_working` needs, not the card id itself.
pub fn working_id_for(working_on: &[WorkingOnStatus], card_id: u64) -> Option<u64> {
    working_on
        .iter()
        .find(|w| w.card_id == card_id)
        .map(|w| w.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn client(server_url: &str) -> HteamClient {
        HteamClient::new_for_test(
            Config::default(),
            server_url.to_string(),
            server_url.to_string(),
        )
        .expect("test client")
    }

    #[tokio::test]
    async fn test_list_working_parses_array_response() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/tr/workingonit/user/")
            .with_status(200)
            .with_body(
                r#"[{"id":7,"user":3,"object_id":42,"content_type":99,"created_at":"2026-08-27T10:00:00Z","updated_at":"2026-08-27T10:00:00Z","content_object":{"title":"Fix bug","url":"/tasks/42/"}}]"#,
            )
            .create_async()
            .await;

        let working = list_working(&client(&server.url())).await.expect("working");

        assert_eq!(working.len(), 1);
        assert_eq!(working[0].id, 7);
        assert_eq!(working[0].card_id, 42);
        assert_eq!(working[0].card_name, "Fix bug");
    }

    #[tokio::test]
    async fn test_start_and_stop_working_call_expected_endpoints() {
        let mut server = mockito::Server::new_async().await;
        let _start = server
            .mock("POST", "/tr/workingonit/")
            .match_body("object_id=42&content_type=99")
            .with_status(204)
            .create_async()
            .await;
        let _stop = server
            .mock("PUT", "/tr/workingonit/7/content_type/99/")
            .match_body("object_id=7&content_type=99&state=4")
            .with_status(204)
            .create_async()
            .await;
        let client = client(&server.url());

        start_working(&client, 42).await.expect("start working");
        stop_working(&client, 7).await.expect("stop working");
    }

    #[test]
    fn test_working_id_for_returns_matching_record_id() {
        let working_on = vec![
            WorkingOnStatus {
                id: 7,
                card_id: 42,
                card_name: "Fix bug".to_string(),
                started_at: "now".to_string(),
            },
            WorkingOnStatus {
                id: 9,
                card_id: 99,
                card_name: "Ship feature".to_string(),
                started_at: "later".to_string(),
            },
        ];

        assert_eq!(working_id_for(&working_on, 42), Some(7));
        assert_eq!(working_id_for(&working_on, 123), None);
    }
}
