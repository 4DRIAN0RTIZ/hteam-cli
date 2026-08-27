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
