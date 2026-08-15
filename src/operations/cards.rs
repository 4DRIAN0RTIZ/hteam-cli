use anyhow::Result;

use crate::client::HteamClient;
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

pub async fn update_description(client: &HteamClient, card_id: u64, description: &str) -> Result<()> {
    client.update_card_description(card_id, description).await
}

pub async fn card_labels(client: &HteamClient, card_id: u64, board: Option<u64>) -> Result<Vec<Label>> {
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
        .update_card_full(card_id, board, &detail, name, description, priority, responsible)
        .await
}
