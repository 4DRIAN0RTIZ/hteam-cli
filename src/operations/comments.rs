use anyhow::{Context, Result};
use chrono::{Local, NaiveDateTime};

use crate::client::{HteamClient, COMMENT_DATE_FORMAT};
use crate::models::{Comment, UserSuggestion};

pub async fn list_comments(client: &HteamClient, card_id: u64) -> Result<Vec<Comment>> {
    client.get_card_comments(card_id).await
}

/// Resolves the follow-up date a caller typed against `COMMENT_DATE_FORMAT`,
/// defaulting to "now" in local time when omitted or blank. The site's own
/// `date` field is server-local time, not UTC — confirmed against a captured
/// request where `timestamp` (Unix epoch) and `date` matched only once
/// shifted by the local UTC offset.
pub fn resolve_comment_date(date: Option<&str>) -> Result<NaiveDateTime> {
    match date.map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => NaiveDateTime::parse_from_str(s, COMMENT_DATE_FORMAT)
            .with_context(|| format!("Fecha inválida '{}', formato esperado: {}", s, COMMENT_DATE_FORMAT)),
        None => Ok(Local::now().naive_local()),
    }
}

pub async fn post_comment(
    client: &HteamClient,
    card_id: u64,
    text: &str,
    board: Option<u64>,
    follow: bool,
    date: Option<&str>,
) -> Result<()> {
    let date = resolve_comment_date(date)?;
    client.post_comment(card_id, text, board, follow, date).await
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
