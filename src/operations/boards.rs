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
