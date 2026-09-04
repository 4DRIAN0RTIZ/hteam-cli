use anyhow::{Context, Result};
use chrono::{Local, NaiveDateTime};

use crate::client::{HteamClient, COMMENT_DATE_FORMAT};
use crate::models::{Comment, FollowUp, UserSuggestion};

pub async fn list_comments(client: &HteamClient, card_id: u64) -> Result<Vec<Comment>> {
    client.get_card_comments(card_id).await
}

pub async fn get_follow_up(
    client: &HteamClient,
    card_id: u64,
    board: Option<u64>,
) -> Result<Option<FollowUp>> {
    client.get_card_follow_up(card_id, board).await
}

pub async fn complete_follow_up(client: &HteamClient, follow_up_id: u64) -> Result<()> {
    client.complete_follow_up(follow_up_id).await
}

pub async fn cancel_follow_up(client: &HteamClient, follow_up_id: u64) -> Result<()> {
    client.cancel_follow_up(follow_up_id).await
}

/// Resolves the follow-up date a caller typed against `COMMENT_DATE_FORMAT`,
/// defaulting to "now" in local time when omitted or blank. The site's own
/// `date` field is server-local time, not UTC — confirmed against a captured
/// request where `timestamp` (Unix epoch) and `date` matched only once
/// shifted by the local UTC offset.
pub fn resolve_comment_date(date: Option<&str>) -> Result<NaiveDateTime> {
    match date.map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => NaiveDateTime::parse_from_str(s, COMMENT_DATE_FORMAT).with_context(|| {
            format!(
                "Fecha inválida '{}', formato esperado: {}",
                s, COMMENT_DATE_FORMAT
            )
        }),
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
    client
        .post_comment(card_id, text, board, follow, date)
        .await
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
    async fn test_list_comments_returns_results() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/comments/api/processes-task/42/")
            .with_status(200)
            .with_body(
                r#"{"results":[{"id":1,"user_name":"ana","submit_date":"2026-08-27","comment":"hola"}]}"#,
            )
            .create_async()
            .await;

        let comments = list_comments(&client(&server.url()), 42)
            .await
            .expect("comments");

        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].user_name, "ana");
        assert_eq!(comments[0].comment, "hola");
    }

    #[tokio::test]
    async fn test_get_follow_up_returns_none_when_no_form_in_html() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/operations/447/tasks/42/")
            .with_status(200)
            .with_body(r#"<div class="media"><a name="c1"></a></div>"#)
            .create_async()
            .await;

        let follow_up = get_follow_up(&client(&server.url()), 42, Some(447))
            .await
            .expect("follow up");

        assert!(follow_up.is_none());
    }

    #[tokio::test]
    async fn test_mention_suggestions_uses_query() {
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

        let suggestions = mention_suggestions(&client(&server.url()), "ana")
            .await
            .expect("suggestions");

        assert_eq!(suggestions[0].id, "8");
        assert_eq!(suggestions[0].selected_text, "ana");
    }

    #[test]
    fn test_resolve_comment_date_parses_expected_format() {
        let date = resolve_comment_date(Some("2026-08-27 09:30")).expect("date");

        assert_eq!(
            date.format(COMMENT_DATE_FORMAT).to_string(),
            "2026-08-27 09:30"
        );
    }

    #[test]
    fn test_resolve_comment_date_rejects_invalid_format() {
        let err = resolve_comment_date(Some("27/08/2026")).expect_err("invalid date");

        assert!(err.to_string().contains("Fecha inválida"));
    }

    #[test]
    fn test_mention_query_only_matches_last_unfinished_token() {
        assert_eq!(mention_query("hola @ana"), Some("ana"));
        assert_eq!(mention_query("hola @"), Some(""));
        assert_eq!(mention_query("hola @ana listo"), None);
        assert_eq!(mention_query("correo a@b"), None);
    }
}
