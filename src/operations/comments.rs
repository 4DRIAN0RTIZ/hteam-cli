use anyhow::Result;

use crate::client::HteamClient;
use crate::models::{Comment, UserSuggestion};

pub async fn list_comments(client: &HteamClient, card_id: u64) -> Result<Vec<Comment>> {
    client.get_card_comments(card_id).await
}

pub async fn post_comment(client: &HteamClient, card_id: u64, text: &str, board: Option<u64>) -> Result<()> {
    client.post_comment(card_id, text, board).await
}

/// Returns the partial username being typed if `text` ends in an unfinished
/// `@mention` token (mirrors the Neovim plugin's `@([%w_]*)$` end-of-line
/// match). Search on a bare '@' too (empty query) so suggestions show up
/// immediately instead of only after the first letter.
pub fn mention_query(text: &str) -> Option<&str> {
    let word_start = text
        .rfind(|c: char| c.is_whitespace())
        .map(|i| i + 1)
        .unwrap_or(0);
    text[word_start..].strip_prefix('@')
}

pub async fn mention_suggestions(client: &HteamClient, query: &str) -> Result<Vec<UserSuggestion>> {
    client.search_users(query).await
}
