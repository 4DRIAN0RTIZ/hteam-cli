use anyhow::Result;

use crate::client::HteamClient;
use crate::models::Reminder;

pub async fn list(client: &HteamClient) -> Result<Vec<Reminder>> {
    client.get_reminders().await
}

pub async fn create(client: &HteamClient, card_id: u64) -> Result<()> {
    client.set_reminder(card_id).await
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
    async fn test_list_returns_wrapped_reminders() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/tr/reminders/user/")
            .with_status(200)
            .with_body(
                r#"{"reminders":[{"id":1,"object_id":42,"type":1,"content_object":{"title":"Fix bug","url":"/tasks/42/"}}]}"#,
            )
            .create_async()
            .await;

        let reminders = list(&client(&server.url())).await.expect("reminders");

        assert_eq!(reminders.len(), 1);
        assert_eq!(reminders[0].object_id, 42);
    }

    #[tokio::test]
    async fn test_create_posts_card_reminder() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("POST", "/tr/reminders/")
            .match_body(mockito::Matcher::PartialJson(serde_json::json!({
                "type": 1,
                "object_id": 42,
                "card_type": 0
            })))
            .with_status(201)
            .create_async()
            .await;

        create(&client(&server.url()), 42)
            .await
            .expect("create reminder");
    }
}
