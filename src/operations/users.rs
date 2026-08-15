use anyhow::Result;

use crate::client::HteamClient;
use crate::models::UserSuggestion;

pub async fn search(client: &HteamClient, query: &str) -> Result<Vec<UserSuggestion>> {
    client.search_users(query).await
}
