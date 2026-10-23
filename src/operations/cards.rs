use anyhow::{Context, Result};
use std::sync::Arc;

use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::client::{HteamClient, TaskFamily, UpdateCardPatch};
use crate::models::{Card, CardDetail, Label, List};

/// Max in-flight list-card fetches while searching a whole board.
const SEARCH_LIST_CONCURRENCY: usize = 4;

/// Where to look for cards. `list_id: None` means every list on the board.
/// Wired from CLI/MCP (#51) and TUI (#52); kept public for those adapters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardSearchScope {
    pub board: Option<u64>,
    pub list_id: Option<u64>,
    /// Operation vs project board API tree (Board bundle family).
    pub family: TaskFamily,
}

impl Default for CardSearchScope {
    fn default() -> Self {
        Self {
            board: None,
            list_id: None,
            family: TaskFamily::Operation,
        }
    }
}

/// Criteria combined with AND. Within `labels`, every name must match some
/// label on the card (case-insensitive equality). Text is a case-insensitive
/// substring over name and description. Responsible/priority match against
/// the display text extracted from the card payload fields.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CardSearchFilters {
    pub text: Option<String>,
    pub responsible: Option<String>,
    pub priority: Option<String>,
    pub labels: Vec<String>,
}

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
#[derive(Debug, Clone, serde::Serialize)]
pub struct CardSearchHit {
    pub card: Card,
    pub list_id: u64,
    pub list_name: String,
}

pub async fn list_lists(client: &HteamClient, board: Option<u64>) -> Result<Vec<List>> {
    list_lists_for(client, board, TaskFamily::Operation).await
}

pub async fn list_lists_for(
    client: &HteamClient,
    board: Option<u64>,
    family: TaskFamily,
) -> Result<Vec<List>> {
    client.get_lists_for(board, family).await
}

pub async fn list_cards(
    client: &HteamClient,
    list_id: u64,
    board: Option<u64>,
) -> Result<(Vec<Card>, Option<u64>)> {
    list_cards_for(client, list_id, board, TaskFamily::Operation).await
}

pub async fn list_cards_for(
    client: &HteamClient,
    list_id: u64,
    board: Option<u64>,
    family: TaskFamily,
) -> Result<(Vec<Card>, Option<u64>)> {
    client.get_cards_for(list_id, board, family).await
}

#[expect(
    dead_code,
    reason = "operation-default wrapper; CLI/MCP call open_cards_for with explicit family"
)]
pub async fn open_cards(client: &HteamClient, board: Option<u64>) -> Result<Vec<Card>> {
    open_cards_for(client, board, TaskFamily::Operation).await
}

pub async fn open_cards_for(
    client: &HteamClient,
    board: Option<u64>,
    family: TaskFamily,
) -> Result<Vec<Card>> {
    client.get_open_cards_for(board, family).await
}

#[expect(
    dead_code,
    reason = "operation-default wrapper; CLI/MCP call closed_cards_for with explicit family"
)]
pub async fn closed_cards(client: &HteamClient, board: Option<u64>) -> Result<Vec<Card>> {
    closed_cards_for(client, board, TaskFamily::Operation).await
}

pub async fn closed_cards_for(
    client: &HteamClient,
    board: Option<u64>,
    family: TaskFamily,
) -> Result<Vec<Card>> {
    client.get_closed_cards_for(board, family).await
}

pub async fn card_detail(client: &HteamClient, card_id: u64) -> Result<CardDetail> {
    client.get_card_detail(card_id).await
}

/// Task detail + board-card merge. The operation-task endpoint lacks
/// estimated/spent time and rich labels; the board-card endpoint supplies
/// `estimated_time` / `time_spent` / `labels_list` / object responsible.
/// For project tasks, resolves the project's internal board_id and edit form
/// from `title_url` instead of the configured operation board.
pub async fn card_detail_enriched(client: &HteamClient, card_id: u64) -> Result<CardDetail> {
    let mut detail = client.get_card_detail(card_id).await?;
    let (family, process_id) = detail
        .title_url
        .as_deref()
        .and_then(TaskFamily::from_title_url_parts)
        .map(|(f, id)| (f, Some(id)))
        .unwrap_or((TaskFamily::Operation, None));

    let board_id = match process_id {
        Some(pid) => resolve_board_id_for(client, Some(pid), family).await.ok(),
        None => client.get_board_id().await.ok(),
    };
    if let Some(board_id) = board_id {
        if let Ok(raw) = client.get_board_card(board_id, card_id).await {
            merge_board_card_into_detail(&mut detail, &raw);
        }
    }

    // Priority (and the bare dependence id) live on the HTML edit form, not JSON.
    let form_process = match process_id {
        Some(pid) => Some(pid),
        None => client.operation_board_number().await.ok(),
    };
    if let Some(process) = form_process {
        if let Ok(form) = client
            .get_task_edit_form_for(process, card_id, family)
            .await
        {
            merge_edit_form_into_detail(&mut detail, &form);
        }
    }
    Ok(detail)
}

/// Merge fields that only the HTML edit form exposes reliably.
pub fn merge_edit_form_into_detail(
    detail: &mut CardDetail,
    form: &crate::client::TaskEditFormBaseline,
) {
    if !form.priority.is_empty() {
        if let Ok(n) = form.priority.parse::<u64>() {
            detail.priority = Some(serde_json::json!(n));
        } else {
            detail.priority = Some(serde_json::json!(form.priority.clone()));
        }
    }
    if detail
        .time_estimated
        .as_ref()
        .map(|s| s.trim().is_empty())
        .unwrap_or(true)
        && !form.time_estimated.trim().is_empty()
    {
        detail.time_estimated = Some(form.time_estimated.clone());
    }
}

/// Human labels for Hteam priority select (1=very low … 5=very high).
pub fn priority_label(value: &serde_json::Value) -> String {
    let n = match value {
        serde_json::Value::Number(n) => n.as_u64(),
        serde_json::Value::String(s) => s.trim().parse().ok(),
        _ => None,
    };
    match n {
        Some(1) => "1 · Muy baja".into(),
        Some(2) => "2 · Baja".into(),
        Some(3) => "3 · Normal".into(),
        Some(4) => "4 · Alta".into(),
        Some(5) => "5 · Muy alta".into(),
        Some(other) => other.to_string(),
        None => value.as_str().unwrap_or("-").to_string(),
    }
}

fn merge_board_card_into_detail(detail: &mut CardDetail, raw: &serde_json::Value) {
    let card = raw.get("card").unwrap_or(raw);

    if let Some(et) = card.get("estimated_time").and_then(|v| v.as_str()) {
        let t = et.trim();
        if !t.is_empty() && t != "---" {
            detail.time_estimated = Some(t.to_string());
        }
    }
    // Board card uses `time_spent`; times POST response uses `time_spend`.
    if let Some(ts) = card
        .get("time_spent")
        .or_else(|| card.get("time_spend"))
        .and_then(|v| v.as_str())
    {
        let t = ts.trim();
        if !t.is_empty() && t != "---" {
            detail.time_spend = Some(t.to_string());
        }
    }
    if detail.start_date.is_none() {
        if let Some(s) = card.get("start_date").and_then(|v| v.as_str()) {
            detail.start_date = Some(s.to_string());
        }
    }
    if detail.due_date.is_none() {
        if let Some(s) = card.get("due_date").and_then(|v| v.as_str()) {
            detail.due_date = Some(s.to_string());
        }
    }
    if let Some(arr) = card.get("labels_list").and_then(|v| v.as_array()) {
        let mut labels = Vec::new();
        for item in arr {
            if let Ok(lab) = serde_json::from_value::<Label>(item.clone()) {
                labels.push(lab);
            }
        }
        if !labels.is_empty() {
            detail.labels = labels;
        }
    }
    // Prefer object responsible over bare id from task detail.
    if let Some(resp) = card.get("responsible") {
        if resp.is_object() || resp.as_str().is_some() {
            detail.responsible = Some(resp.clone());
        }
    }
    if detail.number.is_none() {
        if let Some(n) = card.get("number").and_then(|v| v.as_u64()) {
            detail.number = Some(n);
        }
    }
    if detail.closed.is_none() {
        if let Some(c) = card.get("closed").and_then(|v| v.as_bool()) {
            detail.closed = Some(c);
        }
    }
    // Priority is often missing from both endpoints; keep whatever detail had.
    if detail.priority.is_none() {
        if let Some(p) = card.get("priority") {
            if !p.is_null() {
                detail.priority = Some(p.clone());
            }
        }
    }
}

/// Internal board id used by board-card AJAX routes (times/dates/labels).
pub async fn board_id(client: &HteamClient) -> Result<u64> {
    client.get_board_id().await
}

/// Resolve the **internal** board id for board-card AJAX routes.
/// `board` is the operation/process number (`--board` / config `board_number`),
/// not the AJAX `board_id`. When omitted, uses the configured board.
pub async fn resolve_board_id(client: &HteamClient, board: Option<u64>) -> Result<u64> {
    resolve_board_id_for(client, board, TaskFamily::Operation).await
}

/// Like [`resolve_board_id`], but walks lists/cards on the given family tree
/// (project ids use `/project-new/care/projects/{id}/…`).
pub async fn resolve_board_id_for(
    client: &HteamClient,
    board: Option<u64>,
    family: TaskFamily,
) -> Result<u64> {
    match board {
        None if family == TaskFamily::Operation => client.get_board_id().await,
        None => anyhow::bail!(
            "indica --board con el id del proyecto (familia project no usa el board de config)"
        ),
        Some(process_id) => {
            let (_, bid) = client.get_cards_for(1, Some(process_id), family).await?;
            if let Some(id) = bid {
                return Ok(id);
            }
            for list in client.get_lists_for(Some(process_id), family).await? {
                if list.card_count.unwrap_or(0) == 0 {
                    continue;
                }
                let (_, bid) = client
                    .get_cards_for(list.id, Some(process_id), family)
                    .await?;
                if let Some(id) = bid {
                    return Ok(id);
                }
            }
            anyhow::bail!(
                "no se pudo resolver board_id interno para {} {process_id}",
                family.as_str()
            )
        }
    }
}

/// `type_time`: `estimate` or `time_spend` (AJAX form, not legacy `spend`).
pub async fn set_time(
    client: &HteamClient,
    board_id: u64,
    card_id: u64,
    type_time: &str,
    time: &str,
) -> Result<crate::models::CardTimesResult> {
    client
        .set_card_time(board_id, card_id, type_time, time)
        .await
}

/// Result of setting an estimate and/or registering spent time.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CardTimeResult {
    pub card_id: u64,
    pub board_id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimate: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spent: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

fn validate_duration(value: &str, name: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        anyhow::bail!("{name} no puede estar vacío");
    }
    Ok(value.to_string())
}

/// Set an estimate and/or register spent time using the board-card times API.
/// `spent` is passed through verbatim: the Hteam API accepts a leading `+`
/// (for example `+30m`) to add to the accumulated spent time.
pub async fn set_times(
    client: &HteamClient,
    card_id: u64,
    board: Option<u64>,
    estimate: Option<&str>,
    spent: Option<&str>,
) -> Result<CardTimeResult> {
    if estimate.is_none() && spent.is_none() {
        anyhow::bail!("indica al menos --estimate o --spent");
    }

    set_times_for(
        client,
        card_id,
        board,
        TaskFamily::Operation,
        estimate,
        spent,
    )
    .await
}

pub async fn set_times_for(
    client: &HteamClient,
    card_id: u64,
    board: Option<u64>,
    family: TaskFamily,
    estimate: Option<&str>,
    spent: Option<&str>,
) -> Result<CardTimeResult> {
    if estimate.is_none() && spent.is_none() {
        anyhow::bail!("indica al menos --estimate o --spent");
    }

    let estimate = estimate
        .map(|v| validate_duration(v, "estimate"))
        .transpose()?;
    let spent = spent.map(|v| validate_duration(v, "spent")).transpose()?;
    let board_id = resolve_board_id_for(client, board, family).await?;
    let mut latest = crate::models::CardTimesResult {
        detail: None,
        warnings: vec![],
        start_date: None,
        due_date: None,
        estimated_time: None,
        time_spend: None,
    };

    if let Some(ref value) = estimate {
        latest = client
            .set_card_time(board_id, card_id, "estimate", value)
            .await
            .with_context(|| format!("error al setear estimado de {card_id}"))?;
    }
    if let Some(ref value) = spent {
        latest = client
            .set_card_time(board_id, card_id, "time_spend", value)
            .await
            .with_context(|| format!("error al registrar tiempo de {card_id}"))?;
    }

    Ok(CardTimeResult {
        card_id,
        board_id,
        estimate: latest.estimated_time.or(estimate),
        spent: latest.time_spend.or(spent),
        warnings: latest.warnings,
    })
}

/// `field`: `start_date` or `due_date`; `value`: `YYYY-MM-DD`.
pub async fn set_date(
    client: &HteamClient,
    board_id: u64,
    card_id: u64,
    field: &str,
    value: &str,
) -> Result<()> {
    client.set_card_date(board_id, card_id, field, value).await
}

/// Result of one or more board-card date POSTs.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CardDatesResult {
    pub card_id: u64,
    pub board_id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due_date: Option<String>,
}

/// Validate a real calendar date in `YYYY-MM-DD` format.
pub fn parse_date_yyyy_mm_dd(value: &str) -> Result<String> {
    use chrono::NaiveDate;

    let v = value.trim();
    let date = NaiveDate::parse_from_str(v, "%Y-%m-%d")
        .map_err(|_| anyhow::anyhow!("fecha inválida '{value}' (usa YYYY-MM-DD)"))?;
    if date.format("%Y-%m-%d").to_string() != v || date.format("%Y").to_string().len() != 4 {
        anyhow::bail!("fecha inválida '{value}' (usa YYYY-MM-DD)");
    }
    Ok(date.format("%Y-%m-%d").to_string())
}

/// Set start and/or due date via `POST …/boards/{board_id}/card/{id}/dates/`.
/// Resolves the internal `board_id` (never uses the operation id as board_id).
pub async fn set_dates(
    client: &HteamClient,
    card_id: u64,
    board: Option<u64>,
    start_date: Option<&str>,
    due_date: Option<&str>,
) -> Result<CardDatesResult> {
    set_dates_for(
        client,
        card_id,
        board,
        TaskFamily::Operation,
        start_date,
        due_date,
    )
    .await
}

pub async fn set_dates_for(
    client: &HteamClient,
    card_id: u64,
    board: Option<u64>,
    family: TaskFamily,
    start_date: Option<&str>,
    due_date: Option<&str>,
) -> Result<CardDatesResult> {
    if start_date.is_none() && due_date.is_none() {
        anyhow::bail!("indica al menos --start o --due (YYYY-MM-DD)");
    }

    let start = start_date.map(parse_date_yyyy_mm_dd).transpose()?;
    let due = due_date.map(parse_date_yyyy_mm_dd).transpose()?;
    let board_id = resolve_board_id_for(client, board, family).await?;

    if let Some(ref d) = start {
        client
            .set_card_date(board_id, card_id, "start_date", d)
            .await
            .with_context(|| format!("error al setear start_date de {card_id}"))?;
    }
    if let Some(ref d) = due {
        client
            .set_card_date(board_id, card_id, "due_date", d)
            .await
            .with_context(|| format!("error al setear due_date de {card_id}"))?;
    }

    Ok(CardDatesResult {
        card_id,
        board_id,
        start_date: start,
        due_date: due,
    })
}

/// URL pública de la página de detalle de una card (`/operations/{board}/tasks/{id}/`).
pub async fn card_url(client: &HteamClient, card_id: u64, board: Option<u64>) -> Result<String> {
    card_url_for(client, card_id, board, TaskFamily::Operation).await
}

pub async fn card_url_for(
    client: &HteamClient,
    card_id: u64,
    board: Option<u64>,
    family: TaskFamily,
) -> Result<String> {
    // Prefer the canonical title_url from the task payload when present.
    if let Ok(detail) = client.get_card_detail(card_id).await {
        if let Some(path) = detail.title_url.as_deref() {
            if path.starts_with('/') {
                return Ok(format!(
                    "{}{}",
                    client.site_base_url().trim_end_matches('/'),
                    path
                ));
            }
        }
    }
    client.card_url_for(card_id, board, family).await
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

/// Result of a dedicated close/reopen POST (not a list move).
#[derive(Debug, Clone, serde::Serialize)]
pub struct TaskLifecycleResult {
    pub card_id: u64,
    pub action: &'static str,
    pub family: &'static str,
    pub status: String,
}

fn parse_family_override(family: Option<&str>) -> Result<Option<TaskFamily>> {
    match family {
        None => Ok(None),
        Some(s) if s.trim().is_empty() => Ok(None),
        Some(s) => Ok(Some(TaskFamily::parse(s)?)),
    }
}

async fn resolve_family(
    client: &HteamClient,
    card_id: u64,
    family: Option<&str>,
) -> Result<TaskFamily> {
    if let Some(f) = parse_family_override(family)? {
        return Ok(f);
    }
    client.resolve_task_family(card_id).await
}

/// Close a task via `/operation|project-new/care/tasks/{id}/close/`.
/// When `family` is omitted, resolves operation vs project from the task
/// `title_url` (does not assume closed-list IDs).
pub async fn close_card(
    client: &HteamClient,
    card_id: u64,
    family: Option<&str>,
) -> Result<TaskLifecycleResult> {
    let family = resolve_family(client, card_id, family).await?;
    let status = client.close_task(card_id, family).await?;
    Ok(TaskLifecycleResult {
        card_id,
        action: "close",
        family: family.as_str(),
        status,
    })
}

/// Reopen a task via `/operation|project-new/care/tasks/{id}/reopen/`.
pub async fn reopen_card(
    client: &HteamClient,
    card_id: u64,
    family: Option<&str>,
) -> Result<TaskLifecycleResult> {
    let family = resolve_family(client, card_id, family).await?;
    let status = client.reopen_task(card_id, family).await?;
    Ok(TaskLifecycleResult {
        card_id,
        action: "reopen",
        family: family.as_str(),
        status,
    })
}

pub async fn update_description(
    client: &HteamClient,
    card_id: u64,
    description: &str,
) -> Result<()> {
    client.update_card_description(card_id, description).await
}

pub async fn card_label_catalog(client: &HteamClient) -> Result<Vec<Label>> {
    let mut labels = client.get_label_catalog().await?;
    labels.sort_by_cached_key(|label| label.name.to_lowercase());
    labels.dedup_by_key(|label| label.id);
    Ok(labels)
}

pub async fn card_labels(
    client: &HteamClient,
    card_id: u64,
    board: Option<u64>,
) -> Result<Vec<Label>> {
    let board_id = resolve_board_id(client, board).await?;
    client.get_card_labels(card_id, Some(board_id)).await
}

pub async fn set_card_labels(
    client: &HteamClient,
    card_id: u64,
    board: Option<u64>,
    label_ids: &[u64],
) -> Result<Vec<u64>> {
    let board_id = resolve_board_id(client, board).await?;
    let label_ids = unique_label_ids(label_ids.iter().copied());
    client
        .set_card_labels(board_id, card_id, &label_ids)
        .await?;
    Ok(label_ids)
}

pub async fn add_card_labels(
    client: &HteamClient,
    card_id: u64,
    board: Option<u64>,
    label_ids: &[u64],
) -> Result<Vec<u64>> {
    let board_id = resolve_board_id(client, board).await?;
    let current = client.get_card_labels(card_id, Some(board_id)).await?;
    let mut combined = current_label_ids(current)?;
    combined.extend(label_ids.iter().copied());
    let label_ids = unique_label_ids(combined);
    client
        .set_card_labels(board_id, card_id, &label_ids)
        .await?;
    Ok(label_ids)
}

pub async fn remove_card_labels(
    client: &HteamClient,
    card_id: u64,
    board: Option<u64>,
    label_ids: &[u64],
) -> Result<Vec<u64>> {
    let board_id = resolve_board_id(client, board).await?;
    let current = client.get_card_labels(card_id, Some(board_id)).await?;
    let label_ids = current_label_ids(current)?
        .into_iter()
        .filter(|id| !label_ids.contains(id));
    let label_ids = unique_label_ids(label_ids);
    client
        .set_card_labels(board_id, card_id, &label_ids)
        .await?;
    Ok(label_ids)
}

fn current_label_ids(labels: Vec<Label>) -> Result<Vec<u64>> {
    if labels.iter().any(|label| label.id == 0) {
        anyhow::bail!("el API devolvió labels sin ID; no se modificó el conjunto");
    }
    Ok(unique_label_ids(labels.into_iter().map(|label| label.id)))
}

fn unique_label_ids(ids: impl IntoIterator<Item = u64>) -> Vec<u64> {
    let mut unique = Vec::new();
    for id in ids {
        if !unique.contains(&id) {
            unique.push(id);
        }
    }
    unique
}

/// Pure in-memory filter shared by CLI, MCP and TUI. `cards` is
/// `(list_id, list_name, card)` so callers that already hold board state
/// (TUI, #52) do not need another HTTP round trip.
///
/// `responsible_user_ids` are optional numeric ids (as strings) already
/// resolved from a name query — pass the result of
/// [`resolve_responsible_user_ids`] when filtering by responsible name so bare
/// id payloads (task detail) still match.
#[allow(dead_code)] // used by unit tests + available to adapters
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
        .filter(|(_, _, card)| matches_filters(card, filters, responsible_user_ids))
        .map(|(list_id, list_name, card)| CardSearchHit {
            card: card.clone(),
            list_id,
            list_name: list_name.to_string(),
        })
        .collect()
}

/// Whether `card` satisfies `filters` (same rules as [`filter_cards`] / [`search`]).
pub fn matches_filters(
    card: &Card,
    filters: &CardSearchFilters,
    responsible_user_ids: &[String],
) -> bool {
    card_matches(card, filters, responsible_user_ids)
}

/// Parse a single search line into filters. Tokens:
/// - `@user` → responsible
/// - `#label` → label (AND if several)
/// - anything else → free-text (joined with spaces)
pub fn parse_search_query(raw: &str) -> CardSearchFilters {
    let mut filters = CardSearchFilters::default();
    let mut text_parts: Vec<&str> = Vec::new();
    for token in raw.split_whitespace() {
        if let Some(rest) = token.strip_prefix('@') {
            if !rest.is_empty() {
                filters.responsible = Some(rest.to_string());
            }
        } else if let Some(rest) = token.strip_prefix('#') {
            if !rest.is_empty() {
                filters.labels.push(rest.to_string());
            }
        } else {
            text_parts.push(token);
        }
    }
    if !text_parts.is_empty() {
        filters.text = Some(text_parts.join(" "));
    }
    filters
}

/// Resolve a responsible filter string to user ids via username autocomplete.
/// Hteam task detail only stores `responsible: 140`; list payloads embed
/// `{username, email}`. Matching by name therefore needs this id lookup when
/// the in-memory card only has the bare id.
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
            let all = list_lists_for(client, scope.board, scope.family).await?;
            match all.into_iter().find(|l| l.id == list_id) {
                Some(list) => vec![list],
                None => anyhow::bail!("Lista {} no encontrada en el board", list_id),
            }
        }
        None => list_lists_for(client, scope.board, scope.family).await?,
    };

    if lists.is_empty() {
        return Ok(Vec::new());
    }

    let board = scope.board;
    let family = scope.family;
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
            let result = list_cards_for(&client, list.id, board, family).await;
            (list, result)
        });
    }

    let mut hits = Vec::new();
    while let Some(joined) = set.join_next().await {
        let (list, result) = joined.context("search list task failed")?;
        let (cards, _) = result.with_context(|| format!("Error cargando lista '{}'", list.name))?;
        for card in &cards {
            if matches_filters(card, &filters, &responsible_ids) {
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
    update_card_fields(
        client,
        card_id,
        board,
        CardFieldUpdates {
            name,
            description,
            priority,
            responsible,
            ..Default::default()
        },
    )
    .await
}

/// Optional overrides for the full HTML task edit form.
#[derive(Debug, Clone, Default)]
pub struct CardFieldUpdates<'a> {
    pub name: Option<&'a str>,
    pub description: Option<&'a str>,
    pub priority: Option<&'a str>,
    pub responsible: Option<&'a str>,
    /// Depends-on task id (empty string clears).
    pub dependence: Option<&'a str>,
    pub time_estimated: Option<&'a str>,
}

/// Full edit form fields including dependence (depends-on task id) and estimate.
pub async fn update_card_fields(
    client: &HteamClient,
    card_id: u64,
    board: u64,
    fields: CardFieldUpdates<'_>,
) -> Result<()> {
    let detail = client.get_card_detail(card_id).await?;
    client
        .update_card_full(
            card_id,
            board,
            UpdateCardPatch {
                detail: &detail,
                name: fields.name,
                description: fields.description,
                priority: fields.priority,
                responsible: fields.responsible,
                dependence: fields.dependence,
                time_estimated: fields.time_estimated,
            },
        )
        .await
}

/// Parse a dependence JSON value (number, string id, or object) to a task id.
/// Note: task detail often returns a *display* string like
/// `"#5 - Lectura - Mide lo que importa"` (not the internal id); that does
/// not parse here — use [`dependence_display`] for UI.
pub fn dependence_task_id(value: &Option<serde_json::Value>) -> Option<u64> {
    let v = value.as_ref()?;
    match v {
        serde_json::Value::Null => None,
        serde_json::Value::Number(n) => n.as_u64(),
        serde_json::Value::String(s) => {
            let t = s.trim();
            // Pure digits only — reject labels like "#5 - Title".
            if !t.is_empty() && t.chars().all(|c| c.is_ascii_digit()) {
                t.parse().ok()
            } else {
                None
            }
        }
        serde_json::Value::Object(map) => map
            .get("id")
            .and_then(|x| x.as_u64().or_else(|| x.as_str()?.parse().ok())),
        _ => None,
    }
}

/// Parse visible task number from labels like `#5 - Lectura - Mide lo que importa`.
pub fn dependence_display_number(label: &str) -> Option<u64> {
    let t = label.trim().trim_start_matches('#');
    let digits: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

/// Resolve free text / #n / id against loaded board cards.
pub fn resolve_dependence_query<'a, I>(query: &str, cards: I) -> Option<u64>
where
    I: IntoIterator<Item = &'a Card>,
{
    let q = query.trim();
    if q.is_empty() {
        return None;
    }
    let cards: Vec<&Card> = cards.into_iter().collect();

    if q.chars().all(|c| c.is_ascii_digit()) {
        let n: u64 = q.parse().ok()?;
        if let Some(c) = cards.iter().find(|c| c.id == n) {
            return Some(c.id);
        }
        if let Some(c) = cards.iter().find(|c| c.number == Some(n)) {
            return Some(c.id);
        }
        return Some(n);
    }

    if let Some(num) = dependence_display_number(q) {
        let name_hint = q
            .trim()
            .trim_start_matches('#')
            .chars()
            .skip_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .trim()
            .trim_start_matches('-')
            .trim()
            .to_lowercase();
        let by_num: Vec<&Card> = cards
            .iter()
            .copied()
            .filter(|c| c.number == Some(num))
            .collect();
        if by_num.len() == 1 {
            return Some(by_num[0].id);
        }
        if !name_hint.is_empty() {
            if let Some(c) = by_num
                .into_iter()
                .find(|c| c.name.to_lowercase().contains(&name_hint))
            {
                return Some(c.id);
            }
            if let Some(c) = cards
                .iter()
                .find(|c| c.name.to_lowercase().contains(&name_hint))
            {
                return Some(c.id);
            }
        } else if let Some(c) = by_num.first() {
            return Some(c.id);
        }
    }

    let ql = q.to_lowercase();
    let matches: Vec<&Card> = cards
        .iter()
        .copied()
        .filter(|c| c.name.to_lowercase().contains(&ql))
        .collect();
    if matches.len() == 1 {
        return Some(matches[0].id);
    }
    matches
        .into_iter()
        .find(|c| c.name.eq_ignore_ascii_case(q))
        .map(|c| c.id)
}

/// Human-readable depends-on label from whatever the API returned.
pub fn dependence_display(value: &Option<serde_json::Value>) -> Option<String> {
    let v = value.as_ref()?;
    match v {
        serde_json::Value::Null => None,
        serde_json::Value::Number(n) => Some(format!("id {n}")),
        serde_json::Value::String(s) => {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            }
        }
        serde_json::Value::Object(map) => {
            for key in ["text", "name", "label", "title", "selected_text"] {
                if let Some(serde_json::Value::String(s)) = map.get(key) {
                    if !s.trim().is_empty() {
                        return Some(s.clone());
                    }
                }
            }
            dependence_task_id(value).map(|id| format!("id {id}"))
        }
        _ => None,
    }
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

    fn client_with_board_id(server_url: &str) -> HteamClient {
        let config = Config {
            auth: AuthConfig {
                board_number: Some(483),
                board_id: Some(483),
                csrf_token: Some("csrf".to_string()),
                ..AuthConfig::default()
            },
            ..Config::default()
        };
        HteamClient::new_for_test(config, server_url.to_string(), server_url.to_string())
            .expect("test client")
    }

    fn sample_edit_form_html() -> &'static str {
        r#"<!DOCTYPE html><html><head><title>Edit task</title></head><body>
<form method="post">
<input type="hidden" name="csrfmiddlewaretoken" value="tok">
<input type="text" name="name" value="GBI">
<textarea name="description">d</textarea>
<input type="text" name="time_estimated" value="1h 30m">
<input type="text" name="start_date" value="2026-10-13">
<select name="dependence"><option value="">---------</option><option value="8965" selected>Lectura</option></select>
<select name="responsible"><option value="140" selected>adrian</option></select>
<select name="priority"><option value="3" selected>Normal</option></select>
<input type="hidden" name="id" value="11216">
</form></body></html>"#
    }

    #[test]
    fn current_label_ids_rejects_entries_without_ids() {
        let labels = vec![Label {
            id: 0,
            name: "unknown".into(),
            color: String::new(),
        }];

        assert!(current_label_ids(labels).is_err());
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
    async fn test_list_lists_for_project_uses_project_path() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/project-new/care/projects/469/lists/")
            .match_query(mockito::Matcher::UrlEncoded(
                "format".to_string(),
                "json".to_string(),
            ))
            .with_status(200)
            .with_body(r#"[{"id":1,"name":"Open","total_board_cards":121}]"#)
            .create_async()
            .await;

        let lists = list_lists_for(&client(&server.url()), Some(469), TaskFamily::Project)
            .await
            .expect("project lists");

        assert_eq!(lists[0].card_count, Some(121));
    }

    #[tokio::test]
    async fn test_resolve_board_id_for_project_from_cards() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/project-new/care/projects/469/lists/1/cards/")
            .match_query(mockito::Matcher::UrlEncoded(
                "format".to_string(),
                "json".to_string(),
            ))
            .with_status(200)
            .with_body(
                r#"{"id":1,"name":"Open","cardlist_list":[{"id":10,"board_id":505,"card":{"id":10702,"title":"t","subtitle":"","labels_list":[]},"is_closed":false,"card_type":0,"time_status":0,"get_time_status":"","position":1}]}"#,
            )
            .create_async()
            .await;

        let bid = resolve_board_id_for(&client(&server.url()), Some(469), TaskFamily::Project)
            .await
            .expect("board id");

        assert_eq!(bid, 505);
    }

    #[tokio::test]
    async fn test_set_card_labels_resolves_internal_board_id() {
        let mut server = mockito::Server::new_async().await;
        let _cards = server
            .mock("GET", "/operation/care/operations/483/lists/1/cards/")
            .match_query(mockito::Matcher::UrlEncoded("format".into(), "json".into()))
            .with_status(200)
            .with_body(r#"{"id":1,"name":"Open","cardlist_list":[{"id":10,"board_id":99,"card":{"id":42,"title":"t","subtitle":"","labels_list":[]},"is_closed":false,"card_type":0,"time_status":0,"get_time_status":"","position":1}]}"#)
            .create_async()
            .await;
        let _post = server
            .mock("POST", "/boards/care/boards/99/card/42/labels/")
            .match_body("labels=473&labels=315")
            .with_status(200)
            .create_async()
            .await;

        let label_ids = set_card_labels(&client(&server.url()), 42, None, &[473, 315])
            .await
            .expect("set labels");

        assert_eq!(label_ids, vec![473, 315]);
    }

    #[tokio::test]
    async fn test_add_card_labels_merges_without_duplicates() {
        let mut server = mockito::Server::new_async().await;
        let _get = server
            .mock("GET", "/boards/care/boards/483/card/42/labels/")
            .match_query(mockito::Matcher::UrlEncoded("format".into(), "json".into()))
            .with_status(200)
            .with_body(r#"{"label":"Labels","labels":[{"id":473,"name":"Bug"}]}"#)
            .create_async()
            .await;
        let _post = server
            .mock("POST", "/boards/care/boards/483/card/42/labels/")
            .match_body("labels=473&labels=315")
            .with_status(200)
            .create_async()
            .await;

        let result = add_card_labels(&client_with_board_id(&server.url()), 42, None, &[473, 315])
            .await
            .expect("add labels");

        assert_eq!(result, vec![473, 315]);
    }

    #[tokio::test]
    async fn test_remove_card_labels_posts_remaining_ids() {
        let mut server = mockito::Server::new_async().await;
        let _get = server
            .mock("GET", "/boards/care/boards/483/card/42/labels/")
            .match_query(mockito::Matcher::UrlEncoded("format".into(), "json".into()))
            .with_status(200)
            .with_body(r#"{"label":"Labels","labels":[{"id":473,"name":"Bug"},{"id":315,"name":"Enhancement"}]}"#)
            .create_async()
            .await;
        let _post = server
            .mock("POST", "/boards/care/boards/483/card/42/labels/")
            .match_body("labels=315")
            .with_status(200)
            .create_async()
            .await;

        let result = remove_card_labels(&client_with_board_id(&server.url()), 42, None, &[473])
            .await
            .expect("remove label");

        assert_eq!(result, vec![315]);
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
    async fn test_close_card_resolves_project_family_from_title_url() {
        let mut server = mockito::Server::new_async().await;
        let _detail = server
            .mock("GET", "/operation/care/tasks/6005/")
            .match_query(mockito::Matcher::UrlEncoded(
                "format".to_string(),
                "json".to_string(),
            ))
            .with_status(200)
            .with_body(
                r#"{"id":6005,"name":"t","labels":[],"list":null,"title_url":"/projects/335/tasks/6005/"}"#,
            )
            .create_async()
            .await;
        let _close = server
            .mock("POST", "/project-new/care/tasks/6005/close/")
            .with_status(200)
            .with_body(r#"{"status":"The task has been closed"}"#)
            .create_async()
            .await;

        let result = close_card(&client(&server.url()), 6005, None)
            .await
            .expect("close");
        assert_eq!(result.family, "project");
        assert_eq!(result.action, "close");
        assert_eq!(result.status, "The task has been closed");
    }

    #[tokio::test]
    async fn test_reopen_card_honors_family_override() {
        let mut server = mockito::Server::new_async().await;
        let _reopen = server
            .mock("POST", "/operation/care/tasks/42/reopen/")
            .with_status(200)
            .with_body(r#"{"status":"The task has been reopened"}"#)
            .create_async()
            .await;

        let result = reopen_card(&client(&server.url()), 42, Some("operation"))
            .await
            .expect("reopen");
        assert_eq!(result.family, "operation");
        assert_eq!(result.action, "reopen");
    }

    #[tokio::test]
    async fn test_close_card_rejects_unknown_family() {
        let err = close_card(&client("https://example.test"), 1, Some("board"))
            .await
            .expect_err("bad family");
        assert!(
            err.to_string().contains("familia"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn parse_date_yyyy_mm_dd_accepts_valid_and_rejects_bad() {
        assert_eq!(parse_date_yyyy_mm_dd("2026-10-14").unwrap(), "2026-10-14");
        assert!(parse_date_yyyy_mm_dd("2026-10-14T00:00:00").is_err());
        assert!(parse_date_yyyy_mm_dd("14/10/2026").is_err());
        assert!(parse_date_yyyy_mm_dd("2026-13-01").is_err());
        assert!(parse_date_yyyy_mm_dd("2026-02-30").is_err());
    }

    #[tokio::test]
    async fn test_set_dates_posts_to_internal_board_id() {
        let mut server = mockito::Server::new_async().await;
        // Cached board_id in test client is unset → resolve via open list cards.
        let _lists = server
            .mock("GET", "/operation/care/operations/483/lists/1/cards/")
            .match_query(mockito::Matcher::UrlEncoded(
                "format".to_string(),
                "json".to_string(),
            ))
            .with_status(200)
            .with_body(
                r#"{"id":1,"name":"Open","cardlist_list":[{"id":10,"board_id":99,"card":{"id":42,"title":"t","subtitle":"","labels_list":[]},"is_closed":false,"card_type":0,"time_status":0,"get_time_status":"","position":1}]}"#,
            )
            .create_async()
            .await;
        let _start = server
            .mock("POST", "/boards/care/boards/99/card/42/dates/")
            .match_body(mockito::Matcher::Regex(r"start_date=2026-10-14".into()))
            .with_status(200)
            .with_body("{}")
            .create_async()
            .await;
        let _due = server
            .mock("POST", "/boards/care/boards/99/card/42/dates/")
            .match_body(mockito::Matcher::Regex(r"due_date=2026-10-20".into()))
            .with_status(200)
            .with_body("{}")
            .create_async()
            .await;

        let result = set_dates(
            &client(&server.url()),
            42,
            None,
            Some("2026-10-14"),
            Some("2026-10-20"),
        )
        .await
        .expect("set dates");

        assert_eq!(result.board_id, 99);
        assert_eq!(result.start_date.as_deref(), Some("2026-10-14"));
        assert_eq!(result.due_date.as_deref(), Some("2026-10-20"));
    }

    #[tokio::test]
    async fn test_set_dates_requires_at_least_one_field() {
        let err = set_dates(&client("https://example.test"), 1, None, None, None)
            .await
            .expect_err("need dates");
        assert!(
            err.to_string().contains("--start") || err.to_string().contains("--due"),
            "unexpected: {err}"
        );
    }

    #[tokio::test]
    async fn test_set_times_posts_estimate_and_spent_to_internal_board_id() {
        let mut server = mockito::Server::new_async().await;
        let _lists = server
            .mock("GET", "/operation/care/operations/483/lists/1/cards/")
            .match_query(mockito::Matcher::UrlEncoded(
                "format".to_string(),
                "json".to_string(),
            ))
            .with_status(200)
            .with_body(
                r#"{"id":1,"name":"Open","cardlist_list":[{"id":10,"board_id":99,"card":{"id":42,"title":"t","subtitle":"","labels_list":[]},"is_closed":false,"card_type":0,"time_status":0,"get_time_status":"","position":1}]}"#,
            )
            .create_async()
            .await;
        let _estimate = server
            .mock("POST", "/boards/care/boards/99/card/42/times/")
            .match_body(mockito::Matcher::Regex(
                r"type_time=estimate&time=1h".into(),
            ))
            .with_status(200)
            .with_body(r#"{"estimated_time":"1h","time_spend":"0m"}"#)
            .create_async()
            .await;
        let _spent = server
            .mock("POST", "/boards/care/boards/99/card/42/times/")
            .match_body(mockito::Matcher::Regex(
                r"type_time=time_spend&time=%2B30m".into(),
            ))
            .with_status(200)
            .with_body(r#"{"estimated_time":"1h","time_spend":"30m"}"#)
            .create_async()
            .await;

        let result = set_times(&client(&server.url()), 42, None, Some("1h"), Some("+30m"))
            .await
            .expect("set times");

        assert_eq!(result.board_id, 99);
        assert_eq!(result.estimate.as_deref(), Some("1h"));
        assert_eq!(result.spent.as_deref(), Some("30m"));
    }

    #[tokio::test]
    async fn test_set_times_requires_a_nonempty_value() {
        let no_args = set_times(&client("https://example.test"), 1, None, None, None)
            .await
            .expect_err("need a time");
        assert!(no_args.to_string().contains("--estimate"));

        let blank = set_times(&client("https://example.test"), 1, None, Some(" "), None)
            .await
            .expect_err("blank estimate");
        assert!(blank.to_string().contains("vacío"));
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
        let _form = server
            .mock("GET", "/operations/483/tasks/42/edit/")
            .with_status(200)
            .with_body(sample_edit_form_html())
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
    fn parse_search_query_splits_text_at_and_hash() {
        let f = parse_search_query("GBI @adrian #DOING #urgent");
        assert_eq!(f.text.as_deref(), Some("GBI"));
        assert_eq!(f.responsible.as_deref(), Some("adrian"));
        assert_eq!(f.labels, vec!["DOING".to_string(), "urgent".to_string()]);
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
                ..Default::default()
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
    async fn card_detail_enriched_merges_board_card_times_and_labels() {
        let mut server = mockito::Server::new_async().await;
        let _detail = server
            .mock("GET", "/operation/care/tasks/11216/")
            .match_query(mockito::Matcher::UrlEncoded(
                "format".into(),
                "json".into(),
            ))
            .with_status(200)
            .with_body(
                r#"{"id":11216,"name":"GBI","description":"desc","labels":[315],"responsible":140,"start_date":"2026-10-13T00:00:00-06:00","due_date":"2026-10-13T00:00:00-06:00","number":304}"#,
            )
            .create_async()
            .await;
        let _lists = server
            .mock("GET", "/operation/care/operations/483/lists/")
            .match_query(mockito::Matcher::UrlEncoded("format".into(), "json".into()))
            .with_status(200)
            .with_body(r#"[{"id":1,"name":"Open","total_board_cards":1}]"#)
            .create_async()
            .await;
        let _open_cards = server
            .mock("GET", "/operation/care/operations/483/lists/1/cards/")
            .match_query(mockito::Matcher::UrlEncoded(
                "format".into(),
                "json".into(),
            ))
            .with_status(200)
            .with_body(
                r#"{"id":1,"name":"Open","cardlist_list":[{"id":1,"board_id":483,"card":{"id":1,"title":"x","subtitle":"","labels_list":[]},"is_closed":false,"card_type":0,"time_status":0,"get_time_status":"","position":1}]}"#,
            )
            .create_async()
            .await;
        let _board_card = server
            .mock("GET", "/boards/care/boards/483/card/11216/")
            .match_query(mockito::Matcher::UrlEncoded(
                "format".into(),
                "json".into(),
            ))
            .with_status(200)
            .with_body(
                r##"{"card":{"id":11216,"number":304,"name":"GBI","labels_list":[{"name":"DOING","color":"#53FF1F"}],"responsible":{"username":"adrian.ortiz","email":"a@x"},"start_date":"2026-10-13T00:00:00-06:00","due_date":"2026-10-13T00:00:00-06:00","estimated_time":"1h 30m","time_spent":"45m","closed":false}}"##,
            )
            .create_async()
            .await;

        let detail = card_detail_enriched(&client(&server.url()), 11216)
            .await
            .expect("enriched");

        assert_eq!(detail.time_estimated.as_deref(), Some("1h 30m"));
        assert_eq!(detail.time_spend.as_deref(), Some("45m"));
        assert_eq!(detail.labels.len(), 1);
        assert_eq!(detail.labels[0].name, "DOING");
        assert_eq!(
            responsible_display(detail.responsible.as_ref().unwrap()).as_deref(),
            Some("adrian.ortiz")
        );
    }

    #[test]
    fn merge_edit_form_sets_priority_from_form() {
        let mut detail = CardDetail {
            id: 1,
            name: "t".into(),
            description: None,
            status: None,
            labels: vec![],
            list: None,
            number: None,
            created_at: None,
            updated_at: None,
            time_estimated: None,
            time_spend: None,
            start_date: None,
            due_date: None,
            dependence: None,
            responsible: None,
            priority: None,
            closed: None,
            title_url: None,
            milestone: None,
            milestone_name: None,
        };
        let form = crate::client::TaskEditFormBaseline {
            priority: "5".into(),
            time_estimated: "1h 30m".into(),
            ..Default::default()
        };
        merge_edit_form_into_detail(&mut detail, &form);
        assert_eq!(detail.priority, Some(serde_json::json!(5)));
        assert_eq!(detail.time_estimated.as_deref(), Some("1h 30m"));
        assert_eq!(
            priority_label(detail.priority.as_ref().unwrap()),
            "5 · Muy alta"
        );
    }

    #[test]
    fn dependence_task_id_parses_number_and_string() {
        assert_eq!(
            dependence_task_id(&Some(serde_json::json!(8965))),
            Some(8965)
        );
        assert_eq!(
            dependence_task_id(&Some(serde_json::json!("8965"))),
            Some(8965)
        );
        assert_eq!(dependence_task_id(&None), None);
        assert_eq!(dependence_task_id(&Some(serde_json::Value::Null)), None);
        // Web/API display label is not an id:
        assert_eq!(
            dependence_task_id(&Some(serde_json::json!(
                "#5 - Lectura - Mide lo que importa"
            ))),
            None
        );
    }

    #[test]
    fn dependence_display_keeps_api_label_string() {
        assert_eq!(
            dependence_display(&Some(serde_json::json!(
                "#5 - Lectura - Mide lo que importa"
            )))
            .as_deref(),
            Some("#5 - Lectura - Mide lo que importa")
        );
        assert_eq!(
            dependence_display(&Some(serde_json::json!(8965))).as_deref(),
            Some("id 8965")
        );
    }

    #[tokio::test]
    async fn update_card_fields_posts_empty_dependence_to_clear() {
        let mut server = mockito::Server::new_async().await;
        let _detail = server
            .mock("GET", "/operation/care/tasks/11216/")
            .match_query(mockito::Matcher::UrlEncoded(
                "format".into(),
                "json".into(),
            ))
            .with_status(200)
            .with_body(
                r##"{"id":11216,"name":"GBI","description":"d","labels":[],"dependence":"#5 - Lectura","start_date":"2026-10-13T00:00:00-06:00","responsible":140}"##,
            )
            .create_async()
            .await;
        let _form = server
            .mock("GET", "/operations/483/tasks/11216/edit/")
            .with_status(200)
            .with_body(sample_edit_form_html())
            .create_async()
            .await;
        let _edit = server
            .mock("POST", "/operations/483/tasks/11216/edit/")
            .match_body(mockito::Matcher::Regex("dependence=(&|$)".into()))
            .with_status(200)
            .create_async()
            .await;

        update_card_fields(
            &client(&server.url()),
            11216,
            483,
            CardFieldUpdates {
                dependence: Some(""),
                ..Default::default()
            },
        )
        .await
        .expect("clear dependence");
    }

    #[tokio::test]
    async fn update_card_fields_posts_dependence() {
        let mut server = mockito::Server::new_async().await;
        let _detail = server
            .mock("GET", "/operation/care/tasks/11216/")
            .match_query(mockito::Matcher::UrlEncoded("format".into(), "json".into()))
            .with_status(200)
            .with_body(r#"{"id":11216,"name":"GBI","description":"d","labels":[],"priority":3,"start_date":"2026-10-13T00:00:00-06:00","responsible":140}"#)
            .create_async()
            .await;
        let _form = server
            .mock("GET", "/operations/483/tasks/11216/edit/")
            .with_status(200)
            .with_body(sample_edit_form_html())
            .create_async()
            .await;
        let _edit = server
            .mock("POST", "/operations/483/tasks/11216/edit/")
            .match_body(mockito::Matcher::AllOf(vec![
                mockito::Matcher::Regex(r"start_date=2026-10-13".into()),
                mockito::Matcher::Regex(r"dependence=8965".into()),
                mockito::Matcher::Regex(r"time_estimated=1h\+30m".into()),
            ]))
            .with_status(200)
            .create_async()
            .await;

        update_card_fields(
            &client(&server.url()),
            11216,
            483,
            CardFieldUpdates {
                dependence: Some("8965"),
                ..Default::default()
            },
        )
        .await
        .expect("update dependence");
    }

    #[tokio::test]
    async fn update_priority_preserves_dependence_and_time_from_form() {
        let mut server = mockito::Server::new_async().await;
        let _detail = server
            .mock("GET", "/operation/care/tasks/11216/")
            .match_query(mockito::Matcher::UrlEncoded("format".into(), "json".into()))
            .with_status(200)
            .with_body(
                r##"{"id":11216,"name":"GBI","description":"d","labels":[],"dependence":"#5 - Lectura","start_date":"2026-10-13T00:00:00-06:00","responsible":140}"##,
            )
            .create_async()
            .await;
        let _form = server
            .mock("GET", "/operations/483/tasks/11216/edit/")
            .with_status(200)
            .with_body(sample_edit_form_html())
            .create_async()
            .await;
        let _edit = server
            .mock("POST", "/operations/483/tasks/11216/edit/")
            .match_body(mockito::Matcher::AllOf(vec![
                mockito::Matcher::Regex("priority=4".into()),
                mockito::Matcher::Regex("dependence=8965".into()),
                mockito::Matcher::Regex(r"time_estimated=1h\+30m".into()),
            ]))
            .with_status(200)
            .create_async()
            .await;

        update_card_fields(
            &client(&server.url()),
            11216,
            483,
            CardFieldUpdates {
                priority: Some("4"),
                ..Default::default()
            },
        )
        .await
        .expect("priority preserves dep+time");
    }

    #[tokio::test]
    async fn set_time_posts_time_spend_form() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("POST", "/boards/care/boards/483/card/11215/times/")
            .match_body(mockito::Matcher::AllOf(vec![
                mockito::Matcher::Regex("type_time=time_spend".into()),
                mockito::Matcher::Regex("time=45m".into()),
            ]))
            .with_status(200)
            .with_body(
                r#"{"detail":"Time spent updated","warnings":[],"estimated_time":"1h","time_spend":"45m"}"#,
            )
            .create_async()
            .await;

        let result = set_time(&client(&server.url()), 483, 11215, "time_spend", "45m")
            .await
            .expect("times");
        assert_eq!(result.time_spend.as_deref(), Some("45m"));
        assert_eq!(result.estimated_time.as_deref(), Some("1h"));
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
                ..Default::default()
            },
            CardSearchFilters::default(),
        )
        .await
        .expect_err("missing list");

        assert!(err.to_string().contains("999"));
    }
}
