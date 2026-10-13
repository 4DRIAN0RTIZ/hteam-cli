use anyhow::{Context, Result};
use std::sync::Arc;

use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::client::{HteamClient, UpdateCardPatch};
use crate::models::{Card, CardDetail, Label, List};

/// Max in-flight list-card fetches while searching a whole board.
#[allow(dead_code)] // used by `search` (wired in #51/#52)
const SEARCH_LIST_CONCURRENCY: usize = 4;

/// Where to look for cards. `list_id: None` means every list on the board.
/// Wired from CLI/MCP (#51) and TUI (#52); kept public for those adapters.
#[allow(dead_code)]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CardSearchScope {
    pub board: Option<u64>,
    pub list_id: Option<u64>,
}

/// Criteria combined with AND. Within `labels`, every name must match some
/// label on the card (case-insensitive equality). Text is a case-insensitive
/// substring over name and description. Responsible/priority match against
/// the display text extracted from the card payload fields.
#[allow(dead_code)]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CardSearchFilters {
    pub text: Option<String>,
    pub responsible: Option<String>,
    pub priority: Option<String>,
    pub labels: Vec<String>,
}

#[allow(dead_code)]
impl CardSearchFilters {
    pub fn is_empty(&self) -> bool {
        self.text.as_ref().is_none_or(|s| s.trim().is_empty())
            && self
                .responsible
                .as_ref()
                .is_none_or(|s| s.trim().is_empty())
            && self.priority.as_ref().is_none_or(|s| s.trim().is_empty())
            && self.labels.iter().all(|s| s.trim().is_empty())
    }
}

/// One card that matched a search, with the list it was found in.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct CardSearchHit {
    pub card: Card,
    pub list_id: u64,
    pub list_name: String,
}

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

/// URL pública de la página de detalle de una card (`/operations/{board}/tasks/{id}/`).
pub async fn card_url(client: &HteamClient, card_id: u64, board: Option<u64>) -> Result<String> {
    client.card_url(card_id, board).await
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

pub async fn update_description(
    client: &HteamClient,
    card_id: u64,
    description: &str,
) -> Result<()> {
    client.update_card_description(card_id, description).await
}

pub async fn card_labels(
    client: &HteamClient,
    card_id: u64,
    board: Option<u64>,
) -> Result<Vec<Label>> {
    client.get_card_labels(card_id, board).await
}

/// Pure in-memory filter shared by CLI, MCP and TUI. `cards` is
/// `(list_id, list_name, card)` so callers that already hold board state
/// (TUI) do not need another HTTP round trip.
///
/// `responsible_user_ids` are optional numeric ids (as strings) already
/// resolved from a name query — pass the result of
/// [`resolve_responsible_user_ids`] when filtering by responsible name so bare
/// id payloads (task detail) still match.
#[allow(dead_code)] // adapters in #51 / #52
pub fn filter_cards<'a, I>(
    cards: I,
    filters: &CardSearchFilters,
    responsible_user_ids: &[String],
) -> Vec<CardSearchHit>
where
    I: IntoIterator<Item = (u64, &'a str, &'a Card)>,
{
    cards
        .into_iter()
        .filter(|(_, _, card)| card_matches(card, filters, responsible_user_ids))
        .map(|(list_id, list_name, card)| CardSearchHit {
            card: card.clone(),
            list_id,
            list_name: list_name.to_string(),
        })
        .collect()
}

/// Resolve a responsible filter string to user ids via username autocomplete.
/// Hteam task detail only stores `responsible: 140`; list payloads embed
/// `{username, email}`. Matching by name therefore needs this id lookup when
/// the in-memory card only has the bare id.
#[allow(dead_code)] // adapters in #51 / #52
pub async fn resolve_responsible_user_ids(
    client: &HteamClient,
    responsible_filter: &str,
) -> Result<Vec<String>> {
    let q = responsible_filter.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    // Pure digits: treat as an explicit user id (autocomplete ignores numeric q).
    if q.chars().all(|c| c.is_ascii_digit()) {
        return Ok(vec![q.to_string()]);
    }
    let users = crate::operations::users::search(client, q).await?;
    Ok(users
        .into_iter()
        .map(|u| u.id)
        .filter(|id| !id.is_empty())
        .collect())
}

/// Human-readable responsible label from whatever shape the API returned.
/// Objects → username/name/email; bare numbers → `None` (id only — resolve
/// via [`resolve_responsible_user_ids`] or the board-card endpoint).
#[allow(dead_code)] // adapters in #51 / #52
pub fn responsible_display(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Null => None,
        serde_json::Value::Number(n) => Some(n.to_string()), // bare id; caller may map further
        serde_json::Value::String(s) if !s.trim().is_empty() => Some(s.clone()),
        serde_json::Value::Object(map) => {
            for key in ["username", "name", "selected_text", "full_name", "email"] {
                if let Some(serde_json::Value::String(s)) = map.get(key) {
                    if !s.trim().is_empty() {
                        return Some(s.clone());
                    }
                }
            }
            let flat = json_value_search_text(value);
            if flat.is_empty() {
                None
            } else {
                Some(flat)
            }
        }
        other => {
            let flat = json_value_search_text(other);
            if flat.is_empty() {
                None
            } else {
                Some(flat)
            }
        }
    }
}

/// Loads cards in `scope` (one list or the whole board) and applies `filters`.
/// List card fetches run with limited concurrency; empty filters return every
/// card in scope. Responsible name filters also match bare user ids by
/// resolving names through username autocomplete first.
#[allow(dead_code)] // adapters in #51 / #52
pub async fn search(
    client: &HteamClient,
    scope: CardSearchScope,
    filters: CardSearchFilters,
) -> Result<Vec<CardSearchHit>> {
    let responsible_ids = match filters
        .responsible
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        Some(q) => resolve_responsible_user_ids(client, q)
            .await
            .unwrap_or_default(),
        None => Vec::new(),
    };

    let lists = match scope.list_id {
        Some(list_id) => {
            let all = list_lists(client, scope.board).await?;
            match all.into_iter().find(|l| l.id == list_id) {
                Some(list) => vec![list],
                None => anyhow::bail!("Lista {} no encontrada en el board", list_id),
            }
        }
        None => list_lists(client, scope.board).await?,
    };

    if lists.is_empty() {
        return Ok(Vec::new());
    }

    let board = scope.board;
    let semaphore = Arc::new(Semaphore::new(SEARCH_LIST_CONCURRENCY));
    let mut set = JoinSet::new();

    for list in lists {
        let client = client.clone();
        let semaphore = semaphore.clone();
        set.spawn(async move {
            let _permit = semaphore
                .acquire()
                .await
                .expect("search list semaphore stays open");
            let result = list_cards(&client, list.id, board).await;
            (list, result)
        });
    }

    let mut hits = Vec::new();
    while let Some(joined) = set.join_next().await {
        let (list, result) = joined.context("search list task failed")?;
        let (cards, _) = result.with_context(|| format!("Error cargando lista '{}'", list.name))?;
        for card in &cards {
            if card_matches(card, &filters, &responsible_ids) {
                hits.push(CardSearchHit {
                    card: card.clone(),
                    list_id: list.id,
                    list_name: list.name.clone(),
                });
            }
        }
    }

    hits.sort_by(|a, b| {
        a.list_name
            .cmp(&b.list_name)
            .then_with(|| a.card.name.cmp(&b.card.name))
            .then_with(|| a.card.id.cmp(&b.card.id))
    });

    Ok(hits)
}

#[allow(dead_code)]
fn card_matches(card: &Card, filters: &CardSearchFilters, responsible_user_ids: &[String]) -> bool {
    if let Some(text) = filters
        .text
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        let needle = text.to_lowercase();
        let in_name = card.name.to_lowercase().contains(&needle);
        let in_desc = card
            .description
            .as_deref()
            .map(|d| d.to_lowercase().contains(&needle))
            .unwrap_or(false);
        if !in_name && !in_desc {
            return false;
        }
    }

    if let Some(resp) = filters
        .responsible
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        if !responsible_matches(card, resp, responsible_user_ids) {
            return false;
        }
    }

    if let Some(prio) = filters
        .priority
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        let needle = prio.to_lowercase();
        let hay = card
            .priority
            .as_ref()
            .map(json_value_search_text)
            .unwrap_or_default()
            .to_lowercase();
        if hay != needle && !hay.contains(&needle) {
            return false;
        }
    }

    for label_filter in filters
        .labels
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        let needle = label_filter.to_lowercase();
        let ok = card.labels.iter().any(|l| {
            l.name.eq_ignore_ascii_case(label_filter) || l.name.to_lowercase().contains(&needle)
        });
        if !ok {
            return false;
        }
    }

    true
}

fn responsible_matches(card: &Card, needle: &str, resolved_ids: &[String]) -> bool {
    let Some(value) = card.responsible.as_ref() else {
        return false;
    };

    let hay = json_value_search_text(value).to_lowercase();
    let needle_l = needle.to_lowercase();
    if !hay.is_empty() && hay.contains(&needle_l) {
        return true;
    }

    // Bare id on the card (task detail shape) vs ids resolved from autocomplete.
    if let Some(card_id) = responsible_id_string(value) {
        if resolved_ids.iter().any(|id| id == &card_id) {
            return true;
        }
        if needle.chars().all(|c| c.is_ascii_digit()) && card_id == needle {
            return true;
        }
    }

    false
}

fn responsible_id_string(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::String(s) if s.chars().all(|c| c.is_ascii_digit()) => Some(s.clone()),
        serde_json::Value::Object(map) => map.get("id").and_then(|v| match v {
            serde_json::Value::Number(n) => Some(n.to_string()),
            serde_json::Value::String(s) => Some(s.clone()),
            _ => None,
        }),
        _ => None,
    }
}

/// Flattens the varied shapes Hteam uses for responsible/priority into text
/// suitable for substring matching.
#[allow(dead_code)]
fn json_value_search_text(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => String::new(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(items) => items
            .iter()
            .map(json_value_search_text)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" "),
        serde_json::Value::Object(map) => {
            for key in [
                "username",
                "name",
                "full_name",
                "email",
                "text",
                "selected_text",
            ] {
                if let Some(serde_json::Value::String(s)) = map.get(key) {
                    return s.clone();
                }
            }
            map.values()
                .map(json_value_search_text)
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
        }
    }
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
        .update_card_full(
            card_id,
            board,
            UpdateCardPatch {
                detail: &detail,
                name,
                description,
                priority,
                responsible,
            },
        )
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AuthConfig, Config};

    fn client(server_url: &str) -> HteamClient {
        let config = Config {
            auth: AuthConfig {
                board_number: Some(483),
                csrf_token: Some("csrf".to_string()),
                ..AuthConfig::default()
            },
            ..Config::default()
        };
        HteamClient::new_for_test(config, server_url.to_string(), server_url.to_string())
            .expect("test client")
    }

    #[tokio::test]
    async fn test_list_lists_delegates_to_client() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/operation/care/operations/483/lists/")
            .match_query(mockito::Matcher::UrlEncoded(
                "format".to_string(),
                "json".to_string(),
            ))
            .with_status(200)
            .with_body(r#"[{"id":1,"name":"Open","total_board_cards":2}]"#)
            .create_async()
            .await;

        let lists = list_lists(&client(&server.url()), Some(483))
            .await
            .expect("lists");

        assert_eq!(lists.len(), 1);
        assert_eq!(lists[0].name, "Open");
        assert_eq!(lists[0].card_count, Some(2));
    }

    #[tokio::test]
    async fn test_card_url_uses_explicit_board_over_configured_default() {
        let url = card_url(&client("https://hteam.mx"), 10784, Some(447))
            .await
            .expect("card url");

        assert_eq!(url, "https://hteam.mx/operations/447/tasks/10784/");
    }

    #[tokio::test]
    async fn test_card_url_falls_back_to_configured_board() {
        let url = card_url(&client("https://hteam.mx"), 10784, None)
            .await
            .expect("card url");

        assert_eq!(url, "https://hteam.mx/operations/483/tasks/10784/");
    }

    #[tokio::test]
    async fn test_list_cards_returns_cards_and_board_id() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/operation/care/operations/483/lists/1/cards/")
            .match_query(mockito::Matcher::UrlEncoded(
                "format".to_string(),
                "json".to_string(),
            ))
            .with_status(200)
            .with_body(
                r#"{"id":1,"name":"Open","cardlist_list":[{"id":10,"board_id":99,"card":{"id":42,"title":"Fix bug","subtitle":"desc","labels_list":[]},"is_closed":false,"card_type":0,"time_status":0,"get_time_status":"","position":1}]}"#,
            )
            .create_async()
            .await;

        let (cards, board_id) = list_cards(&client(&server.url()), 1, Some(483))
            .await
            .expect("cards");

        assert_eq!(board_id, Some(99));
        assert_eq!(cards[0].id, 42);
        assert_eq!(cards[0].name, "Fix bug");
    }

    fn card_list_body(total: usize, ids: &[u64]) -> String {
        let entries: Vec<String> = ids
            .iter()
            .map(|id| {
                format!(
                    r#"{{"id":{id},"board_id":99,"card":{{"id":{id},"title":"Card {id}","subtitle":"","labels_list":[]}},"is_closed":false,"card_type":0,"time_status":0,"get_time_status":"","position":1}}"#
                )
            })
            .collect();
        format!(
            r#"{{"id":1,"name":"Open","total_board_cards":{total},"cardlist_list":[{}]}}"#,
            entries.join(",")
        )
    }

    #[tokio::test]
    async fn test_list_cards_refetches_all_when_list_exceeds_default_page() {
        let mut server = mockito::Server::new_async().await;
        let first_page = server
            .mock("GET", "/operation/care/operations/483/lists/1/cards/")
            .match_query(mockito::Matcher::Exact("format=json".to_string()))
            .with_status(200)
            .with_body(card_list_body(3, &[1, 2]))
            .expect(1)
            .create_async()
            .await;
        let full_list = server
            .mock("GET", "/operation/care/operations/483/lists/1/cards/")
            .match_query(mockito::Matcher::Exact(
                "format=json&items_per_page=3".to_string(),
            ))
            .with_status(200)
            .with_body(card_list_body(3, &[1, 2, 3]))
            .expect(1)
            .create_async()
            .await;

        let (cards, _) = list_cards(&client(&server.url()), 1, Some(483))
            .await
            .expect("cards");

        first_page.assert_async().await;
        full_list.assert_async().await;
        assert_eq!(
            cards.iter().map(|c| c.id).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
    }

    #[tokio::test]
    async fn test_list_cards_skips_refetch_when_all_cards_returned() {
        let mut server = mockito::Server::new_async().await;
        let only_request = server
            .mock("GET", "/operation/care/operations/483/lists/1/cards/")
            .match_query(mockito::Matcher::Exact("format=json".to_string()))
            .with_status(200)
            .with_body(card_list_body(2, &[1, 2]))
            .expect(1)
            .create_async()
            .await;

        let (cards, _) = list_cards(&client(&server.url()), 1, Some(483))
            .await
            .expect("cards");

        only_request.assert_async().await;
        assert_eq!(cards.len(), 2);
    }

    #[tokio::test]
    async fn test_move_card_defaults_from_list_to_open() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("POST", "/boards/care/labels/update_card_position/")
            .match_body(mockito::Matcher::PartialJson(serde_json::json!({
                "board_id": 483,
                "card_id": 42,
                "from_list": 1,
                "to_list": 2,
                "card_type": 0
            })))
            .with_status(204)
            .create_async()
            .await;

        move_card(&client(&server.url()), 42, None, 2, Some(483))
            .await
            .expect("move card");
    }

    #[tokio::test]
    async fn test_update_card_fetches_detail_and_posts_patch() {
        let mut server = mockito::Server::new_async().await;
        let _detail = server
            .mock("GET", "/operation/care/tasks/42/")
            .match_query(mockito::Matcher::UrlEncoded(
                "format".to_string(),
                "json".to_string(),
            ))
            .with_status(200)
            .with_body(
                r#"{"id":42,"name":"Old","description":"old desc","labels":[],"list":null,"priority":3}"#,
            )
            .create_async()
            .await;
        let _update = server
            .mock("POST", "/operations/483/tasks/42/edit/")
            .match_body(mockito::Matcher::AllOf(vec![
                mockito::Matcher::Regex("name=New".to_string()),
                mockito::Matcher::Regex("description=new\\+desc".to_string()),
                mockito::Matcher::Regex("priority=2".to_string()),
                mockito::Matcher::Regex("id=42".to_string()),
            ]))
            .with_status(200)
            .create_async()
            .await;

        update_card(
            &client(&server.url()),
            42,
            483,
            Some("New"),
            Some("new desc"),
            Some("2"),
            None,
        )
        .await
        .expect("update card");
    }

    fn sample_card(id: u64, name: &str) -> Card {
        Card {
            id,
            name: name.to_string(),
            description: Some("desc about login".into()),
            status: None,
            labels: vec![Label {
                id: 1,
                name: "bug".into(),
                color: "#f00".into(),
            }],
            number: None,
            title_url: None,
            subtitle_url: None,
            responsible: Some(serde_json::json!({"username": "ana"})),
            priority: Some(serde_json::json!(2)),
            worker: None,
            time_status: None,
        }
    }

    #[test]
    fn filter_cards_matches_text_case_insensitively() {
        let a = sample_card(1, "Fix Login");
        let mut b = sample_card(2, "Other");
        b.description = Some("unrelated".into());
        let filters = CardSearchFilters {
            text: Some("login".into()),
            ..Default::default()
        };

        let hits = filter_cards([(10, "Open", &a), (10, "Open", &b)], &filters, &[]);

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].card.id, 1);
        assert_eq!(hits[0].list_id, 10);
        assert_eq!(hits[0].list_name, "Open");
        assert!(!filters.is_empty());
        assert!(CardSearchFilters::default().is_empty());
    }

    #[test]
    fn filter_cards_ands_text_label_responsible_and_priority() {
        let mut match_all = sample_card(1, "Fix Login");
        match_all.labels.push(Label {
            id: 2,
            name: "urgent".into(),
            color: "#0f0".into(),
        });
        let wrong_prio = sample_card(2, "Fix Login");
        let filters = CardSearchFilters {
            text: Some("fix".into()),
            responsible: Some("ana".into()),
            priority: Some("2".into()),
            labels: vec!["bug".into(), "urgent".into()],
        };

        let hits = filter_cards([(1, "A", &match_all), (1, "A", &wrong_prio)], &filters, &[]);

        assert_eq!(hits.iter().map(|h| h.card.id).collect::<Vec<_>>(), vec![1]);
    }

    #[test]
    fn filter_cards_empty_filters_keeps_all() {
        let a = sample_card(1, "A");
        let b = sample_card(2, "B");
        let hits = filter_cards(
            [(1, "L", &a), (2, "M", &b)],
            &CardSearchFilters::default(),
            &[],
        );
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn filter_cards_matches_bare_responsible_id_via_resolved_ids() {
        let mut only_id = sample_card(11215, "DITRA");
        only_id.responsible = Some(serde_json::json!(140));
        only_id.labels.clear();
        let filters = CardSearchFilters {
            responsible: Some("adrian".into()),
            ..Default::default()
        };

        assert!(filter_cards([(1, "DOING", &only_id)], &filters, &[]).is_empty());

        let hits = filter_cards([(1, "DOING", &only_id)], &filters, &["140".into()]);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].card.id, 11215);
        assert_eq!(
            responsible_display(only_id.responsible.as_ref().unwrap()).as_deref(),
            Some("140")
        );
        assert_eq!(
            responsible_display(&serde_json::json!({"username": "adrian.ortiz"})).as_deref(),
            Some("adrian.ortiz")
        );
    }

    #[tokio::test]
    async fn resolve_responsible_user_ids_uses_autocomplete() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/users/username-autocomplete/")
            .match_query(mockito::Matcher::UrlEncoded("q".into(), "adrian".into()))
            .with_status(200)
            .with_body(
                r#"{"results":[{"id":"140","text":"adrian.ortiz (a@x)","selected_text":"adrian.ortiz"}]}"#,
            )
            .create_async()
            .await;

        let ids = resolve_responsible_user_ids(&client(&server.url()), "adrian")
            .await
            .expect("ids");
        assert_eq!(ids, vec!["140".to_string()]);
    }

    #[tokio::test]
    async fn resolve_responsible_user_ids_accepts_numeric_id() {
        let ids = resolve_responsible_user_ids(&client("https://example.invalid"), "140")
            .await
            .expect("ids");
        assert_eq!(ids, vec!["140".to_string()]);
    }

    #[tokio::test]
    async fn search_loads_board_lists_and_filters() {
        let mut server = mockito::Server::new_async().await;
        let _lists = server
            .mock("GET", "/operation/care/operations/483/lists/")
            .match_query(mockito::Matcher::UrlEncoded(
                "format".into(),
                "json".into(),
            ))
            .with_status(200)
            .with_body(r#"[{"id":10,"name":"Todo","total_board_cards":1},{"id":20,"name":"Doing","total_board_cards":1}]"#)
            .create_async()
            .await;
        let _todo = server
            .mock("GET", "/operation/care/operations/483/lists/10/cards/")
            .match_query(mockito::Matcher::UrlEncoded(
                "format".into(),
                "json".into(),
            ))
            .with_status(200)
            .with_body(
                r##"{"id":10,"name":"Todo","total_board_cards":1,"cardlist_list":[{"id":1,"board_id":99,"card":{"id":1,"title":"Alpha bug","subtitle":"","labels_list":[{"id":1,"name":"bug","color":"#f00"}],"responsible":{"username":"ana"},"priority":2},"is_closed":false,"card_type":0,"time_status":0,"get_time_status":"","position":1}]}"##,
            )
            .create_async()
            .await;
        let _doing = server
            .mock("GET", "/operation/care/operations/483/lists/20/cards/")
            .match_query(mockito::Matcher::UrlEncoded(
                "format".into(),
                "json".into(),
            ))
            .with_status(200)
            .with_body(
                r#"{"id":20,"name":"Doing","total_board_cards":1,"cardlist_list":[{"id":2,"board_id":99,"card":{"id":2,"title":"Beta feature","subtitle":"","labels_list":[]},"is_closed":false,"card_type":0,"time_status":0,"get_time_status":"","position":1}]}"#,
            )
            .create_async()
            .await;

        let hits = search(
            &client(&server.url()),
            CardSearchScope {
                board: Some(483),
                list_id: None,
            },
            CardSearchFilters {
                text: Some("alpha".into()),
                labels: vec!["bug".into()],
                ..Default::default()
            },
        )
        .await
        .expect("search");

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].card.id, 1);
        assert_eq!(hits[0].list_name, "Todo");
        assert_eq!(
            hits[0]
                .card
                .responsible
                .as_ref()
                .and_then(|v| v.get("username"))
                .and_then(|v| v.as_str()),
            Some("ana")
        );
    }

    #[tokio::test]
    async fn search_unknown_list_errors() {
        let mut server = mockito::Server::new_async().await;
        let _lists = server
            .mock("GET", "/operation/care/operations/483/lists/")
            .match_query(mockito::Matcher::UrlEncoded("format".into(), "json".into()))
            .with_status(200)
            .with_body(r#"[{"id":10,"name":"Todo"}]"#)
            .create_async()
            .await;

        let err = search(
            &client(&server.url()),
            CardSearchScope {
                board: Some(483),
                list_id: Some(999),
            },
            CardSearchFilters::default(),
        )
        .await
        .expect_err("missing list");

        assert!(err.to_string().contains("999"));
    }
}
