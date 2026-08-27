use anyhow::Result;

use crate::client::HteamClient;
use crate::models::UserSuggestion;

pub async fn search(client: &HteamClient, query: &str) -> Result<Vec<UserSuggestion>> {
    client.search_users(query).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
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
    async fn test_search_returns_wrapped_results() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/users/username-autocomplete/")
            .match_query(Matcher::UrlEncoded("q".to_string(), "ana".to_string()))
            .with_status(200)
            .with_body(
                r#"{"results":[{"id":"8","selected_text":"ana","text":"ana (ana@example.com)"}]}"#,
            )
            .create_async()
            .await;

        let users = search(&client(&server.url()), "ana").await.expect("users");

        assert_eq!(users[0].id, "8");
        assert_eq!(users[0].text, "ana (ana@example.com)");
    }
}
