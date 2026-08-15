use anyhow::Result;

use crate::client::HteamClient;
use crate::models::Reminder;

pub async fn list(client: &HteamClient) -> Result<Vec<Reminder>> {
    client.get_reminders().await
}

pub async fn create(client: &HteamClient, card_id: u64) -> Result<()> {
    client.set_reminder(card_id).await
}
