use anyhow::{Context, Result};
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE, COOKIE, REFERER};
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::Mutex;

pub mod objectives;
pub mod update;

use crate::config::Config;
use crate::models::{
    BoardEntry, BoardsResponse, Card, CardDetail, CardTimesResult, CheckInResult, Comment,
    CommentsResponse, DailyWorkEntry, FollowUp, Label, List, ProjectMilestone, ProjectSummary,
    ProjectTasksResponse, ProjectsResponse, Reminder, UserSuggestion, WorkShiftResume,
    WorkingOnStatus,
};

const BASE_URL: &str = "https://hteam.mx/api";
const SITE_URL: &str = "https://hteam.mx";

/// Result of probing cookies against the boards list (no board id required).
#[derive(Debug, Clone)]
pub enum AuthProbe {
    Authorized(Vec<BoardEntry>),
    Unauthorized,
}

/// Hteam keeps operation tasks and project tasks on separate API trees.
/// Close/reopen must hit the matching family (both POSTs may succeed on the
/// wrong tree today, but the web always uses the URL family from `title_url`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskFamily {
    Operation,
    Project,
}

impl TaskFamily {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Operation => "operation",
            Self::Project => "project",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "operation" | "operations" | "op" => Ok(Self::Operation),
            "project" | "projects" | "proj" => Ok(Self::Project),
            other => {
                anyhow::bail!("familia de task desconocida '{other}' (usa operation o project)")
            }
        }
    }

    /// Infer family from a task `title_url` (`/operations/…` vs `/projects/…`).
    pub fn from_title_url(title_url: &str) -> Option<Self> {
        if title_url.contains("/projects/") {
            Some(Self::Project)
        } else if title_url.contains("/operations/") {
            Some(Self::Operation)
        } else {
            None
        }
    }
}

/// Format the site's own hidden `date` comment field uses — server-local
/// time, not UTC. `operations::comments` re-exposes this as the format
/// every caller (CLI, MCP, REPL, TUI) validates user input against.
pub const COMMENT_DATE_FORMAT: &str = "%Y-%m-%d %H:%M";

/// Trunca `s` a `max_chars` caracteres para incluirlo en un mensaje de error.
/// Corta por char, no por byte, para no panickear si el límite cae en medio
/// de un carácter multibyte (tildes, ñ) en la respuesta del API.
fn truncate_for_error(s: &str, max_chars: usize) -> String {
    s.chars().take(max_chars).collect()
}

/// Values scraped from GET `/operations/{board}/tasks/{id}/edit/`.
/// Task JSON often omits or mangles these (dependence label string, null priority,
/// missing time_estimated); the form is the source of truth for preserve-on-patch.
#[derive(Debug, Clone, Default)]
pub struct TaskEditFormBaseline {
    pub csrf: Option<String>,
    pub name: String,
    pub description: String,
    pub time_estimated: String,
    pub start_date: String,
    /// Bare depends-on task id, or empty if none.
    pub dependence: String,
    pub responsible: String,
    /// Priority select value `1`..=`5` (web: 1 very low … 5 very high).
    pub priority: String,
}

fn parse_task_edit_form(html: &str) -> TaskEditFormBaseline {
    use scraper::{Html, Selector};

    let doc = Html::parse_document(html);
    let mut out = TaskEditFormBaseline::default();

    let input_value = |name: &str| -> String {
        let sel = match Selector::parse(&format!(r#"input[name="{name}"]"#)) {
            Ok(s) => s,
            Err(_) => return String::new(),
        };
        doc.select(&sel)
            .next()
            .and_then(|el| el.value().attr("value"))
            .unwrap_or("")
            .to_string()
    };

    let textarea_value = |name: &str| -> String {
        let sel = match Selector::parse(&format!(r#"textarea[name="{name}"]"#)) {
            Ok(s) => s,
            Err(_) => return String::new(),
        };
        doc.select(&sel)
            .next()
            .map(|el| el.text().collect::<String>())
            .unwrap_or_default()
            .trim()
            .to_string()
    };

    let select_selected = |name: &str| -> String {
        let sel = match Selector::parse(&format!(r#"select[name="{name}"] option[selected]"#)) {
            Ok(s) => s,
            Err(_) => return String::new(),
        };
        if let Some(el) = doc.select(&sel).next() {
            return el.value().attr("value").unwrap_or("").to_string();
        }
        // Fallback: first option with a value attribute if nothing marked selected.
        let all = match Selector::parse(&format!(r#"select[name="{name}"] option"#)) {
            Ok(s) => s,
            Err(_) => return String::new(),
        };
        for el in doc.select(&all) {
            if el.value().attr("selected").is_some() {
                return el.value().attr("value").unwrap_or("").to_string();
            }
        }
        String::new()
    };

    let csrf = input_value("csrfmiddlewaretoken");
    out.csrf = if csrf.is_empty() { None } else { Some(csrf) };
    out.name = input_value("name");
    out.description = textarea_value("description");
    out.time_estimated = input_value("time_estimated");
    out.start_date = input_value("start_date");
    out.dependence = select_selected("dependence");
    out.responsible = select_selected("responsible");
    out.priority = select_selected("priority");
    out
}

fn dependence_id_from_detail(value: &Option<serde_json::Value>) -> Option<String> {
    match value.as_ref() {
        Some(serde_json::Value::Number(n)) => Some(n.to_string()),
        Some(serde_json::Value::String(s))
            if !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()) =>
        {
            Some(s.clone())
        }
        Some(serde_json::Value::Object(map)) => map.get("id").and_then(|v| match v {
            serde_json::Value::Number(n) => Some(n.to_string()),
            serde_json::Value::String(s)
                if !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()) =>
            {
                Some(s.clone())
            }
            _ => None,
        }),
        _ => None,
    }
}

/// HTML task edit form expects `YYYY-MM-DD`; detail JSON often has full ISO.
fn form_date_yyyy_mm_dd(raw: Option<&str>) -> String {
    let s = raw.unwrap_or("").trim();
    if s.len() >= 10 && s.as_bytes().get(4) == Some(&b'-') && s.as_bytes().get(7) == Some(&b'-') {
        s.chars().take(10).collect()
    } else {
        s.to_string()
    }
}

/// Form `responsible` is a bare user id; some JSON payloads use `{id, username}`.
fn form_user_id(value: Option<&serde_json::Value>) -> String {
    match value {
        Some(serde_json::Value::Number(n)) => n.to_string(),
        Some(serde_json::Value::String(s))
            if !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()) =>
        {
            s.clone()
        }
        Some(serde_json::Value::Object(map)) => map
            .get("id")
            .map(|v| match v {
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            })
            .unwrap_or_default(),
        _ => String::new(),
    }
}

#[derive(Clone)]
pub struct HteamClient {
    client: Client,
    config: Arc<Mutex<Config>>,
    api_base_url: String,
    site_base_url: String,
}

pub struct UpdateCardPatch<'a> {
    pub detail: &'a CardDetail,
    pub name: Option<&'a str>,
    pub description: Option<&'a str>,
    pub priority: Option<&'a str>,
    pub responsible: Option<&'a str>,
    /// Task id this card depends on (form field `dependence`), or empty to clear.
    pub dependence: Option<&'a str>,
    pub time_estimated: Option<&'a str>,
}

impl HteamClient {
    pub fn new(config: Config) -> Result<Self> {
        let client = Client::builder()
            // Don't use cookie store, we'll handle cookies manually
            .user_agent("curl/7.81.0") // Match curl's user-agent
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .context("Error al crear el cliente HTTP")?;

        Ok(Self {
            client,
            config: Arc::new(Mutex::new(config)),
            api_base_url: BASE_URL.to_string(),
            site_base_url: SITE_URL.to_string(),
        })
    }

    pub async fn with_auth(config: Config) -> Result<Self> {
        if !config.is_authenticated() {
            anyhow::bail!("No autenticado. Ejecuta 'hteam login' primero.");
        }
        Self::new(config)
    }

    #[cfg(test)]
    pub(crate) fn new_for_test(
        config: Config,
        api_base_url: String,
        site_base_url: String,
    ) -> Result<Self> {
        let mut client = Self::new(config)?;
        client.api_base_url = api_base_url;
        client.site_base_url = site_base_url;
        Ok(client)
    }

    fn build_headers(&self, config: &Config) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();

        headers.insert(REFERER, HeaderValue::from_static("https://hteam.mx/"));

        if let (Some(session_id), Some(csrf_token)) =
            (&config.auth.session_id, &config.auth.csrf_token)
        {
            let cookie_value = format!("sessionid={}; csrftoken={}", session_id, csrf_token);
            headers.insert(COOKIE, HeaderValue::from_str(&cookie_value)?);

            headers.insert("X-CSRFToken", HeaderValue::from_str(csrf_token)?);
        }

        if let Some(access_token) = &config.auth.access_token {
            headers.insert(
                reqwest::header::AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {}", access_token))?,
            );
        }

        // Add X-Requested-With for AJAX requests (like plugin Lua does)
        headers.insert(
            "X-Requested-With",
            HeaderValue::from_static("XMLHttpRequest"),
        );

        // Add Accept header like curl does
        headers.insert(reqwest::header::ACCEPT, HeaderValue::from_static("*/*"));

        Ok(headers)
    }

    async fn get_board_number(&self) -> Result<u64> {
        let config = self.config.lock().await;
        config
            .get_board_number()
            .or_else(|| config.load_last_ticket().ok().flatten())
            .context("No se especificó el board number. Usa --board o configura uno por defecto.")
    }

    /// Probes session cookies without a board id by listing operations boards.
    /// `401`/`403` mean bad or expired credentials; other failures stay as `Err`
    /// (network / unexpected HTTP) so callers can tell unauthorized from red.
    pub async fn probe_auth(&self) -> Result<AuthProbe> {
        let response = self.fetch_boards_response().await?;
        let status = response.status();

        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Ok(AuthProbe::Unauthorized);
        }

        if !status.is_success() {
            anyhow::bail!(
                "Error HTTP {}: {}",
                status,
                response.text().await.unwrap_or_default()
            );
        }

        let data: BoardsResponse = response
            .json()
            .await
            .context("Error al parsear boards en el probe de auth")?;
        Ok(AuthProbe::Authorized(data.data))
    }

    pub async fn get_lists(&self, board_number: Option<u64>) -> Result<Vec<List>> {
        let board = match board_number {
            Some(b) => b,
            None => self.get_board_number().await?,
        };

        // Build URL/headers under the config lock, then drop it before HTTP so
        // parallel list/card fetches are not serialized on this mutex.
        let (url, headers) = {
            let config = self.config.lock().await;
            let url = format!(
                "{}/operation/care/operations/{}/lists/?format=json",
                self.api_base_url, board
            );
            let headers = self.build_headers(&config)?;
            (url, headers)
        };

        let response = self
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .context("Error al obtener listas")?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Error HTTP {}: {}",
                response.status(),
                response.text().await?
            );
        }

        let lists: Vec<List> = response.json().await?;
        Ok(lists)
    }

    pub async fn get_cards(
        &self,
        list_id: u64,
        board_number: Option<u64>,
    ) -> Result<(Vec<Card>, Option<u64>)> {
        let board = match board_number {
            Some(b) => b,
            None => self.get_board_number().await?,
        };

        // Hteam returns only 10 cards by default and ignores page/offset/limit.
        // If the list holds more (`total_board_cards`), request it again asking
        // for all of them with `items_per_page`, as the Hteam web client does.
        let mut card_response = self.fetch_card_list(board, list_id, None).await?;
        if let Some(total) = card_response.total_board_cards {
            if total > card_response.cardlist_list.len() {
                card_response = self.fetch_card_list(board, list_id, Some(total)).await?;
            }
        }

        // Extract board_id from first entry if available
        let board_id = card_response
            .cardlist_list
            .first()
            .map(|entry| entry.board_id);

        let cards: Vec<Card> = card_response
            .cardlist_list
            .into_iter()
            .map(|entry| {
                let mut card = entry.card;
                if card.has_follow_up() {
                    card.time_status = Some(entry.get_time_status);
                }
                card
            })
            .collect();

        Ok((cards, board_id))
    }

    async fn fetch_card_list(
        &self,
        board: u64,
        list_id: u64,
        items_per_page: Option<usize>,
    ) -> Result<crate::models::CardListResponse> {
        // Same pattern as get_lists: hold config only while copying headers.
        let (url, headers) = {
            let config = self.config.lock().await;
            let mut url = format!(
                "{}/operation/care/operations/{}/lists/{}/cards/?format=json",
                self.api_base_url, board, list_id
            );
            if let Some(n) = items_per_page {
                url.push_str(&format!("&items_per_page={}", n));
            }
            let headers = self.build_headers(&config)?;
            (url, headers)
        };

        let response = self
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .context("Error al obtener cards")?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Error HTTP {}: {}",
                response.status(),
                response.text().await?
            );
        }

        Ok(response.json().await?)
    }

    pub async fn get_open_cards(&self, board_number: Option<u64>) -> Result<Vec<Card>> {
        let (cards, _) = self.get_cards(1, board_number).await?;
        Ok(cards)
    }

    pub async fn get_closed_cards(&self, board_number: Option<u64>) -> Result<Vec<Card>> {
        let (cards, _) = self.get_cards(2, board_number).await?;
        Ok(cards)
    }

    pub async fn get_card_detail(&self, card_id: u64) -> Result<CardDetail> {
        let config = self.config.lock().await;
        let url = format!(
            "{}/operation/care/tasks/{}/?format=json",
            self.api_base_url, card_id
        );
        let headers = self.build_headers(&config)?;

        let response = self
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .context("Error al obtener detalle de card")?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Error HTTP {}: {}",
                response.status(),
                response.text().await?
            );
        }

        let detail: CardDetail = response.json().await?;
        Ok(detail)
    }

    /// Resolve whether `task_id` is an operation or project task.
    /// Prefers `title_url` from the operation-task detail payload (works for both).
    pub async fn resolve_task_family(&self, task_id: u64) -> Result<TaskFamily> {
        let detail = self.get_card_detail(task_id).await?;
        if let Some(url) = detail.title_url.as_deref() {
            if let Some(family) = TaskFamily::from_title_url(url) {
                return Ok(family);
            }
        }
        // Board/operation tasks are the CLI default when the URL is missing.
        Ok(TaskFamily::Operation)
    }

    /// POST `/operation|project-new/care/tasks/{id}/close/`.
    pub async fn close_task(&self, task_id: u64, family: TaskFamily) -> Result<String> {
        self.post_task_lifecycle(task_id, family, "close").await
    }

    /// POST `/operation|project-new/care/tasks/{id}/reopen/`.
    pub async fn reopen_task(&self, task_id: u64, family: TaskFamily) -> Result<String> {
        self.post_task_lifecycle(task_id, family, "reopen").await
    }

    async fn post_task_lifecycle(
        &self,
        task_id: u64,
        family: TaskFamily,
        action: &str,
    ) -> Result<String> {
        let (url, headers) = {
            let config = self.config.lock().await;
            let path = match family {
                TaskFamily::Operation => format!(
                    "{}/operation/care/tasks/{}/{}/",
                    self.api_base_url, task_id, action
                ),
                TaskFamily::Project => format!(
                    "{}/project-new/care/tasks/{}/{}/",
                    self.api_base_url, task_id, action
                ),
            };
            let headers = self.build_headers(&config)?;
            (path, headers)
        };

        let response = self
            .client
            .post(&url)
            .headers(headers)
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header("Origin", self.site_base_url.as_str())
            .body("")
            .send()
            .await
            .with_context(|| format!("Error al {action} la task {task_id}"))?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Error HTTP {}: {}",
                response.status(),
                response.text().await?
            );
        }

        let text = response.text().await.unwrap_or_default();
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(status) = v.get("status").and_then(|s| s.as_str()) {
                return Ok(status.to_string());
            }
        }
        if text.trim().is_empty() {
            Ok(format!("task {action}d"))
        } else {
            Ok(text.trim().chars().take(200).collect())
        }
    }

    /// Board-card JSON used by the web sidebar: has `estimated_time`,
    /// `time_spent`, `labels_list`, object `responsible` — fields the task
    /// detail endpoint often omits or only exposes as bare ids.
    pub async fn get_board_card(&self, board_id: u64, card_id: u64) -> Result<serde_json::Value> {
        let (url, headers) = {
            let config = self.config.lock().await;
            let url = format!(
                "{}/boards/care/boards/{}/card/{}/?format=json",
                self.api_base_url, board_id, card_id
            );
            let headers = self.build_headers(&config)?;
            (url, headers)
        };

        let response = self
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .context("Error al obtener board card")?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Error HTTP {}: {}",
                response.status(),
                response.text().await?
            );
        }

        Ok(response.json().await?)
    }

    pub async fn get_card_comments(&self, card_id: u64) -> Result<Vec<Comment>> {
        let config = self.config.lock().await;
        let url = format!(
            "{}/comments/api/processes-task/{}/",
            self.site_base_url, card_id
        );
        let headers = self.build_headers(&config)?;

        let response = self
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .context("Error al obtener comentarios")?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Error HTTP {}: {}",
                response.status(),
                response.text().await?
            );
        }

        let resp: CommentsResponse = response.json().await?;
        Ok(resp.results)
    }

    /// URL pública de la página de detalle de una card, la misma que scrapea
    /// `get_card_follow_up` — no requiere red, solo resuelve el board.
    pub async fn card_url(&self, card_id: u64, board_number: Option<u64>) -> Result<String> {
        let board = match board_number {
            Some(b) => b,
            None => self.get_board_number().await?,
        };
        Ok(format!(
            "{}/operations/{}/tasks/{}/",
            self.site_base_url, board, card_id
        ))
    }

    /// El seguimiento agendado de una card no viene en la API JSON de
    /// comentarios — solo en el HTML server-rendered de la página de detalle
    /// (`/operations/{board}/tasks/{id}/`), dentro del bloque del comentario
    /// al que está atado (`div.media`), como dos `<form>` con action
    /// `/followup/{id}/cancel` y `/followup/{id}/complete`.
    pub async fn get_card_follow_up(
        &self,
        card_id: u64,
        board_number: Option<u64>,
    ) -> Result<Option<FollowUp>> {
        let board = match board_number {
            Some(b) => b,
            None => self.get_board_number().await?,
        };

        let config = self.config.lock().await;
        let url = format!(
            "{}/operations/{}/tasks/{}/",
            self.site_base_url, board, card_id
        );
        let headers = self.build_headers(&config)?;

        let response = self
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .context("Error al obtener la página de la card")?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Error HTTP {}: {}",
                response.status(),
                response.text().await?
            );
        }

        let html = response.text().await?;
        Self::parse_follow_up(&html)
    }

    fn parse_follow_up(html: &str) -> Result<Option<FollowUp>> {
        use scraper::{ElementRef, Html, Selector};

        let document = Html::parse_document(html);
        let media_sel = Selector::parse("div.media").map_err(|e| anyhow::anyhow!("{:?}", e))?;
        let name_sel = Selector::parse("a[name]").map_err(|e| anyhow::anyhow!("{:?}", e))?;
        let cancel_form_sel = Selector::parse("form[action^='/followup/'][action$='/cancel']")
            .map_err(|e| anyhow::anyhow!("{:?}", e))?;
        // El sitio marca la fecha con un ícono de Font Awesome ("far
        // fa-calendar") dentro de un contenedor cuyo nombre de tag hemos visto
        // mal escrito ("<spam>" en vez de "<span>") — matchear por el ícono en
        // vez de esa clase/tag evita depender de ese detalle frágil. También
        // se evita parsear el formato de fecha en sí (varía por locale: AP
        // style en inglés en daily-work, "7 de Septiembre..." en español acá)
        // tomando el texto crudo del contenedor tal cual.
        let calendar_icon_sel =
            Selector::parse("i.fa-calendar").map_err(|e| anyhow::anyhow!("{:?}", e))?;

        for media in document.select(&media_sel) {
            let Some(cancel_form) = media.select(&cancel_form_sel).next() else {
                continue;
            };
            let action = cancel_form.value().attr("action").unwrap_or("");
            let Some(id) = action
                .trim_start_matches("/followup/")
                .trim_end_matches("/cancel")
                .parse::<u64>()
                .ok()
            else {
                continue;
            };

            let comment_id = media
                .select(&name_sel)
                .next()
                .and_then(|a| a.value().attr("name"))
                .and_then(|n| n.trim_start_matches('c').parse::<u64>().ok())
                .unwrap_or(0);

            let date = media
                .select(&calendar_icon_sel)
                .next()
                .and_then(|icon| icon.parent())
                .and_then(ElementRef::wrap)
                .map(|el| el.text().collect::<Vec<_>>().join(" "))
                .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
                .unwrap_or_default();

            return Ok(Some(FollowUp {
                id,
                comment_id,
                date,
            }));
        }

        Ok(None)
    }

    pub async fn complete_follow_up(&self, follow_up_id: u64) -> Result<()> {
        self.post_follow_up_action(follow_up_id, "complete").await
    }

    pub async fn cancel_follow_up(&self, follow_up_id: u64) -> Result<()> {
        self.post_follow_up_action(follow_up_id, "cancel").await
    }

    /// Espeja el `<form method="post">` que la página de detalle de la card
    /// renderiza junto al comentario de seguimiento — solo lleva
    /// `csrfmiddlewaretoken`, sin más campos.
    async fn post_follow_up_action(&self, follow_up_id: u64, action: &str) -> Result<()> {
        let config = self.config.lock().await;
        let url = format!(
            "{}/followup/{}/{}",
            self.site_base_url, follow_up_id, action
        );
        let headers = self.build_headers(&config)?;
        let csrf = config.auth.csrf_token.as_deref().unwrap_or("");
        let body = format!("csrfmiddlewaretoken={}", urlencoding::encode(csrf));

        let response = self
            .client
            .post(&url)
            .headers(headers)
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header("Origin", self.site_base_url.as_str())
            .header(REFERER, url.as_str())
            .body(body)
            .send()
            .await
            .with_context(|| format!("Error al {} el seguimiento", action))?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Error HTTP {}: {}",
                response.status(),
                response.text().await?
            );
        }

        Ok(())
    }

    /// Actualiza el board activo del cliente en memoria (no toca disco —
    /// eso lo hace el caller vía `Config::save`). Invalida el `board_id`
    /// cacheado porque es específico del board anterior; reusarlo tras un
    /// cambio de board haría que `create_card`/`move_card` operen sobre el
    /// board equivocado.
    pub async fn set_board_number(&self, board_number: u64) {
        let mut config = self.config.lock().await;
        config.auth.board_number = Some(board_number);
        config.auth.board_id = None;
    }

    /// Operation / process id used in `/operations/{board}/…` URLs (`board_number` in config).
    pub async fn operation_board_number(&self) -> Result<u64> {
        self.get_board_number().await
    }

    pub async fn get_board_id(&self) -> Result<u64> {
        {
            let config = self.config.lock().await;
            if let Some(bid) = config.auth.board_id {
                return Ok(bid);
            }
        }
        // Fetch from any list (list 1 = Open) to extract board_id
        let mut board_id = self.get_cards(1, None).await?.1;
        if board_id.is_none() {
            // "Open" can be empty; fall back to any list that actually has cards
            for list in self.get_lists(None).await? {
                if list.card_count.unwrap_or(0) > 0 {
                    board_id = self.get_cards(list.id, None).await?.1;
                    if board_id.is_some() {
                        break;
                    }
                }
            }
        }
        let bid = board_id.ok_or_else(|| anyhow::anyhow!("No se pudo obtener board_id"))?;
        let mut config = self.config.lock().await;
        config.auth.board_id = Some(bid);
        Ok(bid)
    }

    pub async fn get_user_id(&self) -> Result<u64> {
        {
            let config = self.config.lock().await;
            if let Some(uid) = config.auth.user_id {
                return Ok(uid);
            }
        }

        let config = self.config.lock().await;
        let url = format!("{}/tr/workingonit/user/", self.api_base_url);
        let headers = self.build_headers(&config)?;
        drop(config);

        let response = self.client.get(&url).headers(headers).send().await?;
        let text = response.text().await?;

        if let Ok(array) = serde_json::from_str::<Vec<crate::models::WorkingOnResponse>>(&text) {
            if let Some(first) = array.first() {
                return Ok(self.cache_user_id(first.user).await);
            }
        }
        if let Ok(single) = serde_json::from_str::<crate::models::WorkingOnResponse>(&text) {
            return Ok(self.cache_user_id(single.user).await);
        }

        // `/tr/workingonit/user/` sólo devuelve algo mientras el usuario
        // tiene una card marcada "en curso" (start_working). Si no, caemos
        // al último registro de turno, que trae el user_id sin depender de
        // ese estado.
        if let Ok(resume) = self.get_workshift_resume().await {
            if let Some(last) = resume.last {
                return Ok(self.cache_user_id(last.user.id).await);
            }
        }

        anyhow::bail!("No se pudo obtener el user_id del usuario autenticado")
    }

    /// Guarda `user_id` en memoria y en disco (`config.toml`) para que no
    /// haya que volver a resolverlo en cada arranque del TUI/CLI — es un
    /// dato del usuario, no de la sesión, así que sigue siendo válido
    /// aunque las cookies expiren y se vuelva a hacer login.
    async fn cache_user_id(&self, uid: u64) -> u64 {
        let mut config = self.config.lock().await;
        config.auth.user_id = Some(uid);
        let _ = config.save();
        uid
    }

    pub async fn create_card(
        &self,
        name: &str,
        list_id: Option<u64>,
        board_id: Option<u64>,
    ) -> Result<Card> {
        let board_id = match board_id {
            Some(id) => id,
            None => self.get_board_id().await?,
        };
        let board_number = self.get_board_number().await?;
        let user_id = self.get_user_id().await?;

        let url = format!(
            "{}/boards/care/boards/{}/create_card/",
            self.api_base_url, board_id
        );
        let headers = {
            let config = self.config.lock().await;
            self.build_headers(&config)?
        };

        let labels: Vec<u64> = list_id.into_iter().collect();
        let body = serde_json::json!({
            "name": name,
            "complement": user_id,
            "process": board_number,
            "labels": labels,
        });

        let response = self
            .client
            .post(&url)
            .headers(headers)
            .header(CONTENT_TYPE, "application/json")
            .json(&body)
            .send()
            .await
            .context("Error al crear card")?;

        let status = response.status();
        let response_text = response.text().await?;

        if !status.is_success() {
            anyhow::bail!("Error HTTP {}: {}", status, response_text);
        }

        // API returns empty body on success — lookup the card by name in the target list
        let needs_lookup = response_text.trim().is_empty() || {
            serde_json::from_str::<Card>(&response_text)
                .map(|c| c.id == 0)
                .unwrap_or(true)
        };

        if needs_lookup {
            if let Some(lid) = list_id {
                if let Ok((cards, _)) = self.get_cards(lid, None).await {
                    if let Some(found) = cards.into_iter().find(|c| c.name == name) {
                        return Ok(found);
                    }
                }
            }
            return Ok(Card {
                id: 0,
                name: name.to_string(),
                description: None,
                status: None,
                labels: vec![],
                number: None,
                title_url: None,
                subtitle_url: None,
                responsible: None,
                priority: None,
                worker: None,
                time_status: None,
            });
        }

        serde_json::from_str::<Card>(&response_text).with_context(|| {
            format!(
                "Respuesta inesperada del API al crear card: {:?}",
                truncate_for_error(&response_text, 300)
            )
        })
    }

    pub async fn move_card(
        &self,
        card_id: u64,
        from_list: u64,
        to_list: u64,
        board_number: Option<u64>,
    ) -> Result<()> {
        let board = match board_number {
            Some(b) => b,
            None => self.get_board_number().await?,
        };

        let config = self.config.lock().await;
        let url = format!(
            "{}/boards/care/labels/update_card_position/",
            self.api_base_url
        );
        let headers = self.build_headers(&config)?;

        let body = serde_json::json!({
            "board_id": board,
            "card_id": card_id,
            "from_list": from_list,
            "to_list": to_list,
            "card_type": 0,
        });

        let response = self
            .client
            .post(&url)
            .headers(headers)
            .header(CONTENT_TYPE, "application/json")
            .json(&body)
            .send()
            .await
            .context("Error al mover card")?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Error HTTP {}: {}",
                response.status(),
                response.text().await?
            );
        }

        Ok(())
    }

    pub async fn update_card_description(&self, card_id: u64, description: &str) -> Result<()> {
        let board_number = self.get_board_number().await?;
        let detail = self.get_card_detail(card_id).await?;
        self.update_card_full(
            card_id,
            board_number,
            UpdateCardPatch {
                detail: &detail,
                name: None,
                description: Some(description),
                priority: None,
                responsible: None,
                dependence: None,
                time_estimated: None,
            },
        )
        .await
    }

    pub async fn get_working_on(&self) -> Result<Vec<WorkingOnStatus>> {
        let config = self.config.lock().await;
        let url = format!("{}/tr/workingonit/user/", self.api_base_url);
        let headers = self.build_headers(&config)?;

        let response = self
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .context("Error al obtener working on")?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Error HTTP {}: {}",
                response.status(),
                response.text().await?
            );
        }

        // Try to parse as array first, then as single object
        let response_text = response.text().await?;

        // Check if response is empty or null
        if response_text.trim().is_empty() || response_text.trim() == "null" {
            return Ok(vec![]);
        }

        // Try to parse as array
        if let Ok(array) =
            serde_json::from_str::<Vec<crate::models::WorkingOnResponse>>(&response_text)
        {
            return Ok(array.into_iter().map(|resp| resp.into()).collect());
        }

        // Try to parse as single object
        if let Ok(single) = serde_json::from_str::<crate::models::WorkingOnResponse>(&response_text)
        {
            return Ok(vec![single.into()]);
        }

        anyhow::bail!(
            "No se pudo parsear la respuesta de working on: {}",
            truncate_for_error(&response_text, 200)
        )
    }

    pub async fn start_working(&self, card_id: u64) -> Result<()> {
        let config = self.config.lock().await;
        let url = format!("{}/tr/workingonit/", self.api_base_url);
        let headers = self.build_headers(&config)?;

        let body = format!("object_id={}&content_type=99", card_id);

        let response = self
            .client
            .post(&url)
            .headers(headers)
            .header(
                CONTENT_TYPE,
                "application/x-www-form-urlencoded; charset=UTF-8",
            )
            .header("X-Requested-With", "XMLHttpRequest")
            .body(body)
            .send()
            .await
            .context("Error al iniciar working on")?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Error HTTP {}: {}",
                response.status(),
                response.text().await?
            );
        }

        Ok(())
    }

    pub async fn stop_working(&self, working_id: u64) -> Result<()> {
        let config = self.config.lock().await;
        let url = format!(
            "{}/tr/workingonit/{}/content_type/99/",
            self.api_base_url, working_id
        );
        let headers = self.build_headers(&config)?;

        let body = format!("object_id={}&content_type=99&state=4", working_id);

        let response = self
            .client
            .put(&url)
            .headers(headers)
            .header(
                CONTENT_TYPE,
                "application/x-www-form-urlencoded; charset=UTF-8",
            )
            .header("X-Requested-With", "XMLHttpRequest")
            .body(body)
            .send()
            .await
            .context("Error al detener working on")?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Error HTTP {}: {}",
                response.status(),
                response.text().await?
            );
        }

        Ok(())
    }

    pub async fn post_comment(
        &self,
        card_id: u64,
        comment: &str,
        board_number: Option<u64>,
        follow: bool,
        date: chrono::NaiveDateTime,
        reply_to: Option<u64>,
    ) -> Result<()> {
        let board = match board_number {
            Some(b) => b,
            None => self.get_board_number().await?,
        };

        let config = self.config.lock().await;
        let task_url = format!(
            "{}/operations/{}/tasks/{}/",
            self.site_base_url, board, card_id
        );
        let headers = self.build_headers(&config)?;

        let html_response = self
            .client
            .get(&task_url)
            .headers(headers.clone())
            .send()
            .await
            .context("Error al obtener página de card")?;

        if !html_response.status().is_success() {
            anyhow::bail!(
                "Error HTTP {} al obtener página de card",
                html_response.status()
            );
        }

        let html = html_response.text().await?;

        let (timestamp, security_hash, csrf) = self
            .extract_comment_tokens(&html)
            .context("No se pudo extraer el formulario de comentarios de la página")?;

        let encode = |s: &str| {
            let mut out = String::new();
            for b in s.bytes() {
                match b {
                    b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                        out.push(b as char)
                    }
                    b' ' => out.push('+'),
                    _ => out.push_str(&format!("%{:02X}", b)),
                }
            }
            out
        };

        let date_str = date.format(COMMENT_DATE_FORMAT).to_string();
        let mut fields = vec![
            format!("csrfmiddlewaretoken={}", encode(&csrf)),
            "next=%2Fcomments%2Fsent%2F".to_string(),
            "content_type=processes.task".to_string(),
            format!("object_pk={}", card_id),
            format!("timestamp={}", timestamp),
            format!("security_hash={}", security_hash),
            format!("reply_to={}", reply_to.unwrap_or(0)),
            "honeypot=".to_string(),
            format!("comment={}", encode(comment)),
        ];
        // Mirrors the HTML checkbox: only sent when checked, omitted otherwise.
        if follow {
            fields.push("follow=on".to_string());
        }
        fields.push(format!("date={}", encode(&date_str)));
        let body = fields.join("&");

        let comment_url = format!("{}/comments/post/", self.site_base_url);

        let response = self
            .client
            .post(&comment_url)
            .headers(headers)
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header("Origin", self.site_base_url.as_str())
            .header(REFERER, task_url.as_str())
            .body(body)
            .send()
            .await
            .context("Error al publicar comentario")?;

        let code = response.status().as_u16();
        if code >= 400 {
            anyhow::bail!("Error HTTP {}: {}", code, response.text().await?);
        }

        Ok(())
    }

    pub async fn get_card_labels(&self, card_id: u64, board_id: Option<u64>) -> Result<Vec<Label>> {
        let board = match board_id {
            Some(id) => id,
            None => self.get_board_id().await?,
        };

        let config = self.config.lock().await;
        let url = format!(
            "{}/boards/care/boards/{}/card/{}/labels/?format=json",
            self.api_base_url, board, card_id
        );
        let headers = self.build_headers(&config)?;

        let response = self
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .context("Error al obtener labels")?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Error HTTP {}: {}",
                response.status(),
                response.text().await?
            );
        }

        #[derive(serde::Deserialize)]
        #[serde(untagged)]
        enum LabelsResponse {
            Wrapped {
                #[serde(default)]
                labels: Vec<Label>,
            },
            Plain(Vec<Label>),
        }

        let response: LabelsResponse = response.json().await?;
        Ok(match response {
            LabelsResponse::Wrapped { labels } | LabelsResponse::Plain(labels) => labels,
        })
    }

    /// Load the paginated label catalog used by the web label selector.
    pub async fn get_label_catalog(&self) -> Result<Vec<Label>> {
        #[derive(serde::Deserialize)]
        struct LabelCatalogPage {
            #[serde(default)]
            count: usize,
            #[serde(default)]
            results: Vec<Label>,
        }

        let (url, headers) = {
            let config = self.config.lock().await;
            (
                format!("{}/boards/care/labels/", self.api_base_url),
                self.build_headers(&config)?,
            )
        };
        let mut labels = Vec::new();
        let mut page = 1usize;
        loop {
            let response = self
                .client
                .get(&url)
                .query(&[
                    ("format", "json"),
                    ("page_size", "50"),
                    ("page", &page.to_string()),
                ])
                .headers(headers.clone())
                .send()
                .await
                .context("Error al obtener catálogo de labels")?;
            if !response.status().is_success() {
                anyhow::bail!(
                    "Error HTTP {}: {}",
                    response.status(),
                    response.text().await?
                );
            }
            let result: LabelCatalogPage = response
                .json()
                .await
                .context("Error al interpretar catálogo de labels")?;
            let previous_count = labels.len();
            labels.extend(result.results);
            if labels.len() >= result.count || labels.len() == previous_count {
                break;
            }
            page += 1;
            if page > 100 {
                anyhow::bail!("El catálogo de labels excede el límite de páginas");
            }
        }
        Ok(labels)
    }

    pub async fn set_card_labels(
        &self,
        board_id: u64,
        card_id: u64,
        label_ids: &[u64],
    ) -> Result<()> {
        let (url, headers) = {
            let config = self.config.lock().await;
            let url = format!(
                "{}/boards/care/boards/{}/card/{}/labels/",
                self.api_base_url, board_id, card_id
            );
            let headers = self.build_headers(&config)?;
            (url, headers)
        };
        let body = label_ids
            .iter()
            .map(|id| format!("labels={id}"))
            .collect::<Vec<_>>()
            .join("&");

        let response = self
            .client
            .post(&url)
            .headers(headers)
            .header(
                CONTENT_TYPE,
                "application/x-www-form-urlencoded; charset=UTF-8",
            )
            .header("Origin", self.site_base_url.as_str())
            .body(body)
            .send()
            .await
            .context("Error al actualizar labels de la card")?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Error HTTP {}: {}",
                response.status(),
                response.text().await?
            );
        }

        Ok(())
    }

    /// URL pública de la página de detalle de un proyecto (`/projects/{id}/`)
    /// — a diferencia de `card_url`, no depende del board, así que no
    /// necesita tocar la red ni el config.
    pub fn project_url(&self, project_id: u64) -> String {
        format!("{}/projects/{}/", self.site_base_url, project_id)
    }

    pub async fn get_project_milestones(&self, project_id: u64) -> Result<Vec<ProjectMilestone>> {
        let config = self.config.lock().await;
        let url = format!(
            "{}/project-new/care/projects/{}/milestones_progress/",
            self.api_base_url, project_id
        );
        let headers = self.build_headers(&config)?;

        let response = self
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .context("Error al obtener milestones")?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Error HTTP {}: {}",
                response.status(),
                response.text().await?
            );
        }

        let milestones: Vec<ProjectMilestone> = response.json().await?;
        Ok(milestones)
    }

    pub async fn get_project_tasks(&self, project_id: u64) -> Result<ProjectTasksResponse> {
        let config = self.config.lock().await;
        let url = format!(
            "{}/project-new/care/{}/tasks-projects/",
            self.api_base_url, project_id
        );
        let headers = self.build_headers(&config)?;

        let response = self
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .context("Error al obtener tasks de proyecto")?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Error HTTP {}: {}",
                response.status(),
                response.text().await?
            );
        }

        let resp: ProjectTasksResponse = response.json().await?;
        Ok(resp)
    }

    /// Load current HTML edit-form values (priority, dependence id, estimate, …).
    /// Task JSON often leaves these null or as display labels only.
    pub async fn get_task_edit_form(
        &self,
        board_number: u64,
        card_id: u64,
    ) -> Result<TaskEditFormBaseline> {
        let url = format!(
            "{}/operations/{}/tasks/{}/edit/",
            self.site_base_url, board_number, card_id
        );
        let headers = {
            let config = self.config.lock().await;
            self.build_headers(&config)?
        };
        let response = self
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .context("Error al cargar formulario de edit")?;
        if !response.status().is_success() {
            anyhow::bail!(
                "Error HTTP {}: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            );
        }
        let html = response.text().await.unwrap_or_default();
        Ok(parse_task_edit_form(&html))
    }

    pub async fn update_card_full(
        &self,
        card_id: u64,
        board_number: u64,
        patch: UpdateCardPatch<'_>,
    ) -> Result<()> {
        let url = format!(
            "{}/operations/{}/tasks/{}/edit/",
            self.site_base_url, board_number, card_id
        );
        let (headers, csrf_config) = {
            let config = self.config.lock().await;
            let headers = self.build_headers(&config)?;
            let csrf = config.auth.csrf_token.clone().unwrap_or_default();
            (headers, csrf)
        };

        // Task JSON omits / mangles fields the HTML form has (dependence id,
        // priority select). Load the edit form so we preserve current values
        // when the patch only touches one field (e.g. priority must not wipe
        // dependence).
        let form_html = {
            let response = self
                .client
                .get(&url)
                .headers(headers.clone())
                .send()
                .await
                .context("Error al cargar formulario de edit")?;
            if !response.status().is_success() {
                anyhow::bail!(
                    "Error HTTP {}: {}",
                    response.status(),
                    response.text().await.unwrap_or_default()
                );
            }
            response.text().await.unwrap_or_default()
        };
        let baseline = parse_task_edit_form(&form_html);

        let encode = |s: &str| {
            let mut out = String::new();
            for b in s.bytes() {
                match b {
                    b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                        out.push(b as char);
                    }
                    b' ' => out.push('+'),
                    _ => out.push_str(&format!("%{:02X}", b)),
                }
            }
            out
        };

        let val_to_str = |v: &serde_json::Value| -> String {
            match v {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::Null => String::new(),
                other => other.to_string(),
            }
        };

        let detail = patch.detail;
        let csrf = baseline
            .csrf
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(csrf_config.as_str());

        // Form expects a bare task id (or empty to clear). Never re-post the
        // human label string the detail API returns ("#5 - Title…").
        let dependence_owned: String = if let Some(dep) = patch.dependence {
            dep.to_string()
        } else if let Some(id) = dependence_id_from_detail(&detail.dependence) {
            id
        } else {
            baseline.dependence.clone()
        };

        // Detail JSON uses ISO datetimes; the HTML form only accepts YYYY-MM-DD.
        let start_date_owned = {
            let from_detail = form_date_yyyy_mm_dd(detail.start_date.as_deref());
            if !from_detail.is_empty() {
                from_detail
            } else {
                form_date_yyyy_mm_dd(Some(baseline.start_date.as_str()))
            }
        };

        let responsible_owned = patch.responsible.map(str::to_string).unwrap_or_else(|| {
            let from_detail = form_user_id(detail.responsible.as_ref());
            if !from_detail.is_empty() {
                from_detail
            } else {
                baseline.responsible.clone()
            }
        });

        let priority_owned = patch.priority.map(str::to_string).unwrap_or_else(|| {
            detail
                .priority
                .as_ref()
                .map(val_to_str)
                .filter(|s| !s.is_empty())
                .or_else(|| {
                    let p = baseline.priority.trim();
                    if p.is_empty() {
                        None
                    } else {
                        Some(p.to_string())
                    }
                })
                .unwrap_or_else(|| "3".to_string())
        });

        let time_estimated_owned = patch.time_estimated.map(str::to_string).unwrap_or_else(|| {
            detail
                .time_estimated
                .clone()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| baseline.time_estimated.clone())
        });

        let name_owned = patch
            .name
            .map(str::to_string)
            .unwrap_or_else(|| detail.name.clone());
        let description_owned = patch.description.map(str::to_string).unwrap_or_else(|| {
            detail
                .description
                .clone()
                .unwrap_or_else(|| baseline.description.clone())
        });

        let fields = vec![
            format!("csrfmiddlewaretoken={}", encode(csrf)),
            format!("process_id={}", board_number),
            format!("name={}", encode(&name_owned)),
            format!("description={}", encode(&description_owned)),
            format!("time_estimated={}", encode(&time_estimated_owned)),
            format!("start_date={}", encode(&start_date_owned)),
            format!("dependence={}", encode(&dependence_owned)),
            format!("responsible={}", encode(&responsible_owned)),
            format!("priority={}", encode(&priority_owned)),
            format!("id={}", card_id),
        ];
        let body = fields.join("&");

        let response = self
            .client
            .post(&url)
            .headers(headers)
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header("Origin", self.site_base_url.as_str())
            .header(REFERER, url.as_str())
            .body(body)
            .send()
            .await
            .context("Error al actualizar card")?;

        let code = response.status().as_u16();
        let text = response.text().await.unwrap_or_default();
        if code >= 400 {
            anyhow::bail!("Error HTTP {}: {}", code, text);
        }
        // Django often returns 200 with the edit form again when validation fails
        // (e.g. full ISO start_date). Success redirects to the task view (no Edit title).
        if text.contains("errorlist")
            || text.contains("<title>Edit task</title>")
            || text.contains("value=Update")
        {
            anyhow::bail!(
                "El servidor no aplicó el cambio del task (formulario rechazado). Revisa fechas/campos."
            );
        }

        Ok(())
    }

    /// POST board-card times API (`type_time=estimate|time_spend`, `time=45m`).
    /// `board_id` is the internal board id from list payloads, not the operation id.
    pub async fn set_card_time(
        &self,
        board_id: u64,
        card_id: u64,
        type_time: &str,
        time: &str,
    ) -> Result<CardTimesResult> {
        let (url, headers) = {
            let config = self.config.lock().await;
            let url = format!(
                "{}/boards/care/boards/{}/card/{}/times/",
                self.api_base_url, board_id, card_id
            );
            let headers = self.build_headers(&config)?;
            (url, headers)
        };

        let body = format!(
            "type_time={}&time={}",
            urlencoding::encode(type_time),
            urlencoding::encode(time)
        );

        let response = self
            .client
            .post(&url)
            .headers(headers)
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header("Origin", self.site_base_url.as_str())
            .body(body)
            .send()
            .await
            .context("Error al actualizar tiempos de la card")?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Error HTTP {}: {}",
                response.status(),
                response.text().await?
            );
        }

        Ok(response.json().await?)
    }

    /// POST board-card dates API (`start_date` or `due_date` as YYYY-MM-DD).
    pub async fn set_card_date(
        &self,
        board_id: u64,
        card_id: u64,
        field: &str,
        value: &str,
    ) -> Result<()> {
        let (url, headers) = {
            let config = self.config.lock().await;
            let url = format!(
                "{}/boards/care/boards/{}/card/{}/dates/",
                self.api_base_url, board_id, card_id
            );
            let headers = self.build_headers(&config)?;
            (url, headers)
        };

        let body = format!(
            "{}={}",
            urlencoding::encode(field),
            urlencoding::encode(value)
        );

        let response = self
            .client
            .post(&url)
            .headers(headers)
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header("Origin", self.site_base_url.as_str())
            .body(body)
            .send()
            .await
            .context("Error al actualizar fechas de la card")?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Error HTTP {}: {}",
                response.status(),
                response.text().await?
            );
        }

        Ok(())
    }

    pub async fn set_reminder(&self, card_id: u64) -> Result<()> {
        let config = self.config.lock().await;
        let url = format!("{}/tr/reminders/", self.api_base_url);
        let headers = self.build_headers(&config)?;

        let body = serde_json::json!({
            "type": 1,
            "object_id": card_id,
            "card_type": 0,
        });

        let response = self
            .client
            .post(&url)
            .headers(headers)
            .header(CONTENT_TYPE, "application/json")
            .json(&body)
            .send()
            .await
            .context("Error al crear reminder")?;

        let code = response.status().as_u16();
        if code >= 400 {
            anyhow::bail!("Error HTTP {}: {}", code, response.text().await?);
        }

        Ok(())
    }

    pub async fn cancel_reminder(&self, reminder_id: u64) -> Result<()> {
        let config = self.config.lock().await;
        let url = format!("{}/tr/reminders/{}/", self.api_base_url, reminder_id);
        let headers = self.build_headers(&config)?;
        let response = self
            .client
            .get(&url)
            .headers(headers.clone())
            .send()
            .await
            .context("Error al obtener reminder para cancelarlo")?;
        if !response.status().is_success() {
            anyhow::bail!(
                "Error HTTP {} al obtener reminder: {}",
                response.status(),
                response.text().await?
            );
        }

        let record: serde_json::Value = response
            .json()
            .await
            .context("Respuesta inválida al obtener reminder para cancelarlo")?;
        let fields = record
            .as_object()
            .context("La respuesta del reminder no es un objeto JSON")?;
        let object_id = fields
            .get("object_id")
            .and_then(serde_json::Value::as_u64)
            .context("La respuesta del reminder no incluye object_id válido")?;
        let content_type = fields
            .get("content_type")
            .and_then(serde_json::Value::as_u64)
            .context("La respuesta del reminder no incluye content_type válido")?;
        let body = serde_json::json!({
            "canceled": true,
            "object_id": object_id,
            "content_type": content_type,
        });

        let response = self
            .client
            .put(&url)
            .headers(headers)
            .header(CONTENT_TYPE, "application/json")
            .json(&body)
            .send()
            .await
            .context("Error al cancelar reminder")?;

        let code = response.status().as_u16();
        if code >= 400 {
            anyhow::bail!("Error HTTP {}: {}", code, response.text().await?);
        }

        Ok(())
    }

    pub async fn get_reminders(&self) -> Result<Vec<Reminder>> {
        let config = self.config.lock().await;
        let url = format!("{}/tr/reminders/user/", self.api_base_url);
        let headers = self.build_headers(&config)?;

        let response = self
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .context("Error al obtener reminders")?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Error HTTP {}: {}",
                response.status(),
                response.text().await?
            );
        }

        let text = response.text().await?;
        if text.trim().is_empty() || text.trim() == "null" {
            return Ok(vec![]);
        }

        if let Ok(r) = serde_json::from_str::<crate::models::RemindersResponse>(&text) {
            return Ok(r.reminders);
        }

        if let Ok(list) = serde_json::from_str::<Vec<Reminder>>(&text) {
            return Ok(list);
        }

        Ok(vec![])
    }

    pub async fn get_daily_work_history(
        &self,
        user: Option<&str>,
        activity_type: Option<&str>,
        time_range: Option<&str>,
    ) -> Result<Vec<DailyWorkEntry>> {
        let config = self.config.lock().await;

        let mut params = Vec::new();
        if let Some(u) = user {
            params.push(format!("user={}", urlencoding::encode(u)));
        }
        if let Some(t) = activity_type {
            params.push(format!("type={}", urlencoding::encode(t)));
        }
        if let Some(r) = time_range {
            params.push(format!("time_range={}", urlencoding::encode(r)));
        }
        let url = if params.is_empty() {
            format!("{}/history/daily-work", self.site_base_url)
        } else {
            format!(
                "{}/history/daily-work?{}",
                self.site_base_url,
                params.join("&")
            )
        };

        let headers = self.build_headers(&config)?;

        let response = self
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .context("Error al obtener historial de trabajo diario")?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Error HTTP {}: {}",
                response.status(),
                response.text().await?
            );
        }

        let html = response.text().await?;
        Self::parse_daily_work_history(&html)
    }

    fn parse_daily_work_history(html: &str) -> Result<Vec<DailyWorkEntry>> {
        use scraper::{Html, Selector};

        let timestamp_re = regex::Regex::new(
            r"[A-Z][a-z]{2,3}\.\s\d{1,2},\s\d{4},\s(?:\d{1,2}:\d{2}\s[ap]\.m\.|noon|midnight)",
        )
        .context("Regex de timestamp inválida")?;

        let document = Html::parse_document(html);
        let entry_sel = Selector::parse("#history p").map_err(|e| anyhow::anyhow!("{:?}", e))?;
        let link_sel = Selector::parse("a").map_err(|e| anyhow::anyhow!("{:?}", e))?;

        let mut entries = Vec::new();

        for p in document.select(&entry_sel) {
            let full_text = p.text().collect::<Vec<_>>().join(" ");
            let full_text = full_text.split_whitespace().collect::<Vec<_>>().join(" ");
            if full_text.is_empty() {
                continue;
            }

            let links: Vec<(String, String)> = p
                .select(&link_sel)
                .map(|a| {
                    let href = a.value().attr("href").unwrap_or("").to_string();
                    let text = a
                        .text()
                        .collect::<String>()
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ");
                    (href, text)
                })
                .collect();

            let user_link = links
                .iter()
                .find(|(href, _)| href.starts_with("/history/daily-work"));
            let user = user_link
                .map(|(_, text)| text.clone())
                .unwrap_or_else(|| "External source".to_string());

            let target_link = links.iter().find(|(href, _)| {
                !href.starts_with("/history/daily-work")
                    && !href.contains("/boards/general/labels/")
            });
            let target_url = target_link.map(|(href, _)| format!("{}{}", SITE_URL, href));

            let rest = full_text
                .strip_prefix(user.as_str())
                .unwrap_or(&full_text)
                .trim();

            let Some(ts_match) = timestamp_re.find_iter(rest).last() else {
                continue;
            };

            let timestamp = ts_match.as_str().to_string();
            let activity = rest[..ts_match.start()].trim().to_string();
            let relative = rest[ts_match.end()..]
                .trim()
                .trim_start_matches(',')
                .trim()
                .to_string();

            entries.push(DailyWorkEntry {
                user,
                activity,
                target_url,
                timestamp,
                relative,
            });
        }

        Ok(entries)
    }

    pub async fn search_users(&self, query: &str) -> Result<Vec<UserSuggestion>> {
        let config = self.config.lock().await;
        let url = format!(
            "{}/users/username-autocomplete/?q={}",
            self.api_base_url, query
        );
        let headers = self.build_headers(&config)?;

        let response = self
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .context("Error al buscar usuarios")?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Error HTTP {}: {}",
                response.status(),
                response.text().await?
            );
        }

        let text = response
            .text()
            .await
            .context("Error leyendo respuesta de búsqueda de usuarios")?;

        if text.trim().is_empty() || text.trim() == "null" {
            return Ok(vec![]);
        }

        // The endpoint's shape isn't consistent between an empty and a
        // non-empty query (mirrors the defensive parsing already used in
        // get_working_on/get_reminders below): try the documented
        // `{"results": [...]}` wrapper first, then a bare array.
        if let Ok(resp) = serde_json::from_str::<crate::models::UserAutocompleteResponse>(&text) {
            return Ok(resp.results);
        }
        if let Ok(list) = serde_json::from_str::<Vec<UserSuggestion>>(&text) {
            return Ok(list);
        }

        anyhow::bail!(
            "Respuesta inesperada al buscar usuarios: {:?}",
            truncate_for_error(&text, 200)
        )
    }

    pub async fn check_in(&self) -> Result<CheckInResult> {
        let config = self.config.lock().await;
        let url = format!("{}/tr/checkworkshifs/check_in/", self.api_base_url);
        let headers = self.build_headers(&config)?;

        let response = self
            .client
            .post(&url)
            .headers(headers)
            .header(CONTENT_TYPE, "application/json")
            .body("{}")
            .send()
            .await
            .context("Error al hacer check in")?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Error HTTP {}: {}",
                response.status(),
                response.text().await?
            );
        }

        let result: CheckInResult = response.json().await?;
        Ok(result)
    }

    pub async fn get_workshift_resume(&self) -> Result<WorkShiftResume> {
        let config = self.config.lock().await;
        let url = format!("{}/tr/checkworkshifs/resume/", self.api_base_url);
        let headers = self.build_headers(&config)?;

        let response = self
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .context("Error al obtener el resumen del turno")?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Error HTTP {}: {}",
                response.status(),
                response.text().await?
            );
        }

        let result: WorkShiftResume = response.json().await?;
        Ok(result)
    }

    async fn fetch_boards_response(&self) -> Result<reqwest::Response> {
        let config = self.config.lock().await;
        let headers = self.build_headers(&config)?;
        drop(config);

        let url = format!("{}/operation/care/operations/", self.api_base_url);
        self.client
            .get(&url)
            .headers(headers)
            .query(&[
                ("format", "datatables"),
                ("status", "Execution"),
                ("draw", "1"),
                ("start", "0"),
                ("length", "500"),
                ("columns[0][data]", "name"),
                ("columns[0][orderable]", "true"),
                ("columns[1][data]", "service"),
                ("columns[2][data]", "responsible"),
                ("columns[3][data]", "status"),
                ("columns[4][data]", "total_tasks"),
                ("columns[5][data]", "total_tasks_closed"),
                ("columns[6][data]", "id"),
                ("columns[6][orderable]", "true"),
                ("order[0][column]", "0"),
                ("order[0][dir]", "asc"),
            ])
            .send()
            .await
            .context("Error de red al obtener boards")
    }

    /// Obtiene todos los boards en estado "Execution" desde el endpoint datatables.
    pub async fn get_boards(&self) -> Result<Vec<BoardEntry>> {
        match self.probe_auth().await? {
            AuthProbe::Authorized(boards) => Ok(boards),
            AuthProbe::Unauthorized => {
                anyhow::bail!("No autorizado al listar boards (sesión inválida o sin permiso)")
            }
        }
    }

    /// Obtiene los proyectos en `status` (default "Execution", igual que el
    /// filtro por defecto de `/projects/` en el sitio) desde el endpoint
    /// datatables — descubierto en el JS embebido de esa página
    /// (`ajax_api()` arma `/api/project-new/care/projects/?format=datatables&status=...`).
    pub async fn get_projects(&self, status: Option<&str>) -> Result<Vec<ProjectSummary>> {
        let config = self.config.lock().await;
        let headers = self.build_headers(&config)?;
        drop(config);

        let url = format!("{}/project-new/care/projects/", self.api_base_url);
        let response = self
            .client
            .get(&url)
            .headers(headers)
            .query(&[
                ("format", "datatables"),
                ("status", status.unwrap_or("Execution")),
            ])
            .send()
            .await
            .context("Error al obtener proyectos")?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Error HTTP {}: {}",
                response.status(),
                response.text().await?
            );
        }

        let data: ProjectsResponse = response.json().await?;
        Ok(data.data)
    }

    fn extract_comment_tokens(&self, html: &str) -> Option<(String, String, String)> {
        let extract = |name: &str| -> Option<String> {
            let needle = format!("name=\"{}\"", name);
            let pos = html.find(&needle)?;
            let after = &html[pos..];
            let val_start = after.find("value=\"")? + 7;
            let val_end = after[val_start..].find('"')?;
            Some(after[val_start..val_start + val_end].to_string())
        };

        let timestamp = extract("timestamp")?;
        let security_hash = extract("security_hash")?;
        let csrf = extract("csrfmiddlewaretoken")?;

        Some((timestamp, security_hash, csrf))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AuthConfig;
    use mockito::Matcher;

    fn authenticated_client(server_url: &str) -> HteamClient {
        let config = Config {
            auth: AuthConfig {
                session_id: Some("session".to_string()),
                csrf_token: Some("csrf".to_string()),
                access_token: Some("access".to_string()),
                board_number: Some(483),
                ..AuthConfig::default()
            },
            ..Config::default()
        };
        HteamClient::new_for_test(config, server_url.to_string(), server_url.to_string())
            .expect("test client")
    }

    #[tokio::test]
    async fn test_post_comment_sends_reply_to_id() {
        let mut server = mockito::Server::new_async().await;
        let _page = server
            .mock("GET", "/operations/483/tasks/42/")
            .with_status(200)
            .with_body(r#"<input name="timestamp" value="123"><input name="security_hash" value="hash"><input name="csrfmiddlewaretoken" value="csrf">"#)
            .create_async()
            .await;
        let _post = server
            .mock("POST", "/comments/post/")
            .match_body(Matcher::UrlEncoded("reply_to".into(), "17".into()))
            .match_body(Matcher::UrlEncoded("comment".into(), "respuesta".into()))
            .with_status(302)
            .create_async()
            .await;
        let client = authenticated_client(&server.url());

        client
            .post_comment(
                42,
                "respuesta",
                Some(483),
                false,
                chrono::NaiveDate::from_ymd_opt(2026, 10, 23)
                    .unwrap()
                    .and_hms_opt(12, 0, 0)
                    .unwrap(),
                Some(17),
            )
            .await
            .expect("reply comment");
    }

    #[tokio::test]
    async fn test_new_for_test_uses_mock_base_url() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/operation/care/operations/483/lists/")
            .match_query(Matcher::UrlEncoded(
                "format".to_string(),
                "json".to_string(),
            ))
            .with_status(200)
            .with_body(r#"[{"id":1,"name":"Open","total_board_cards":0}]"#)
            .create_async()
            .await;
        let client = HteamClient::new_for_test(
            Config::default(),
            server.url(),
            "http://example.test".to_string(),
        )
        .expect("test client");

        let lists = client.get_lists(Some(483)).await.expect("lists");

        assert_eq!(lists[0].name, "Open");
    }

    #[tokio::test]
    async fn test_cancel_reminder_puts_full_record_with_canceled_true() {
        let mut server = mockito::Server::new_async().await;
        let _detail = server
            .mock("GET", "/tr/reminders/123/")
            .with_status(200)
            .with_body(r#"{"id":123,"user":{"username":"ana"},"customer":21,"object_id":42,"content_type":99,"canceled":null,"type":1}"#)
            .create_async()
            .await;
        let _update = server
            .mock("PUT", "/tr/reminders/123/")
            .match_body(Matcher::Json(serde_json::json!({
                "canceled": true,
                "object_id": 42,
                "content_type": 99
            })))
            .with_status(200)
            .create_async()
            .await;
        let client = HteamClient::new_for_test(Config::default(), server.url(), server.url())
            .expect("client");

        client.cancel_reminder(123).await.expect("cancel reminder");
    }

    #[tokio::test]
    async fn test_get_card_labels_parses_wrapped_response() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/boards/care/boards/483/card/42/labels/")
            .match_query(Matcher::UrlEncoded("format".into(), "json".into()))
            .with_status(200)
            .with_body(r##"{"label":"Labels","labels":[{"id":473,"name":"Bug","color":"#fff"}]}"##)
            .create_async()
            .await;
        let client = HteamClient::new_for_test(Config::default(), server.url(), server.url())
            .expect("client");

        let labels = client.get_card_labels(42, Some(483)).await.expect("labels");

        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].id, 473);
        assert_eq!(labels[0].name, "Bug");
    }

    #[tokio::test]
    async fn test_get_card_labels_accepts_array_response() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/boards/care/boards/483/card/43/labels/")
            .match_query(Matcher::UrlEncoded("format".into(), "json".into()))
            .with_status(200)
            .with_body(r##"[{"id":315,"name":"Feature","color":"#abc"}]"##)
            .create_async()
            .await;
        let client = HteamClient::new_for_test(Config::default(), server.url(), server.url())
            .expect("client");

        let labels = client.get_card_labels(43, Some(483)).await.expect("labels");

        assert_eq!(labels[0].id, 315);
    }

    #[tokio::test]
    async fn test_get_label_catalog_follows_pages() {
        let mut server = mockito::Server::new_async().await;
        let _page_one = server
            .mock("GET", "/boards/care/labels/")
            .match_query(Matcher::UrlEncoded("page".into(), "1".into()))
            .with_status(200)
            .with_body(r##"{"count":2,"next":"?page=2","results":[{"id":473,"name":"Proceso","color":"#f00"}]}"##)
            .create_async()
            .await;
        let _page_two = server
            .mock("GET", "/boards/care/labels/")
            .match_query(Matcher::UrlEncoded("page".into(), "2".into()))
            .with_status(200)
            .with_body(
                r##"{"count":2,"next":null,"results":[{"id":315,"name":"DOING","color":"#0f0"}]}"##,
            )
            .create_async()
            .await;
        let client = HteamClient::new_for_test(Config::default(), server.url(), server.url())
            .expect("client");

        let labels = client.get_label_catalog().await.expect("catalog");

        assert_eq!(
            labels.iter().map(|label| label.id).collect::<Vec<_>>(),
            vec![473, 315]
        );
    }

    #[tokio::test]
    async fn test_set_card_labels_posts_repeated_label_ids() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("POST", "/boards/care/boards/483/card/42/labels/")
            .match_header(
                "content-type",
                "application/x-www-form-urlencoded; charset=UTF-8",
            )
            .match_body("labels=473&labels=315")
            .with_status(200)
            .create_async()
            .await;
        let client = HteamClient::new_for_test(Config::default(), server.url(), server.url())
            .expect("client");

        client
            .set_card_labels(483, 42, &[473, 315])
            .await
            .expect("set labels");
    }

    #[tokio::test]
    async fn test_get_lists_sends_auth_headers_and_parses_response() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/operation/care/operations/483/lists/")
            .match_query(Matcher::UrlEncoded(
                "format".to_string(),
                "json".to_string(),
            ))
            .match_header("cookie", "sessionid=session; csrftoken=csrf")
            .match_header("x-csrftoken", "csrf")
            .match_header("authorization", "Bearer access")
            .match_header("x-requested-with", "XMLHttpRequest")
            .with_status(200)
            .with_body(r#"[{"id":1,"name":"Open","total_board_cards":3}]"#)
            .create_async()
            .await;
        let client = authenticated_client(&server.url());

        let lists = client.get_lists(None).await.expect("lists");

        assert_eq!(lists.len(), 1);
        assert_eq!(lists[0].id, 1);
        assert_eq!(lists[0].name, "Open");
        assert_eq!(lists[0].card_count, Some(3));
    }

    #[tokio::test]
    async fn test_get_lists_returns_http_error_body() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/operation/care/operations/483/lists/")
            .match_query(Matcher::UrlEncoded(
                "format".to_string(),
                "json".to_string(),
            ))
            .with_status(500)
            .with_body("boom")
            .create_async()
            .await;
        let client = authenticated_client(&server.url());

        let err = client.get_lists(None).await.expect_err("http error");

        assert!(err
            .to_string()
            .contains("Error HTTP 500 Internal Server Error: boom"));
    }

    #[tokio::test]
    async fn test_get_cards_fails_on_invalid_json_response() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/operation/care/operations/483/lists/1/cards/")
            .match_query(Matcher::UrlEncoded(
                "format".to_string(),
                "json".to_string(),
            ))
            .with_status(200)
            .with_body(r#"{"unexpected":true}"#)
            .create_async()
            .await;
        let client = authenticated_client(&server.url());

        let err = client.get_cards(1, None).await.expect_err("json error");
        let err_text = format!("{err:#}");

        assert!(err_text.contains("missing field"), "{err_text}");
    }

    #[tokio::test]
    async fn test_get_cards_copies_time_status_only_for_follow_up_cards() {
        let mut server = mockito::Server::new_async().await;
        let body = r##"{
            "id": 315,
            "name": "DOING",
            "cardlist_list": [
                {
                    "id": 1,
                    "board_id": 447,
                    "is_closed": false,
                    "card_type": 0,
                    "time_status": 5,
                    "get_time_status": "Expired",
                    "position": 0,
                    "card": {
                        "id": 10716,
                        "title": "ITAM - Revision Intencion y politicas",
                        "labels_list": [
                            {"id": 315, "name": "DOING", "color": "#53FF1F"},
                            {"id": 387, "name": "Follow-up", "color": "#AEED39"}
                        ]
                    }
                },
                {
                    "id": 2,
                    "board_id": 447,
                    "is_closed": false,
                    "card_type": 0,
                    "time_status": 3,
                    "get_time_status": "On time",
                    "position": 1,
                    "card": {
                        "id": 10717,
                        "title": "Card sin seguimiento",
                        "labels_list": []
                    }
                }
            ]
        }"##;
        let _m = server
            .mock("GET", "/operation/care/operations/483/lists/315/cards/")
            .match_query(Matcher::UrlEncoded(
                "format".to_string(),
                "json".to_string(),
            ))
            .with_status(200)
            .with_body(body)
            .create_async()
            .await;
        let client = authenticated_client(&server.url());

        let (cards, _) = client.get_cards(315, None).await.expect("cards");

        assert_eq!(cards[0].time_status.as_deref(), Some("Expired"));
        assert_eq!(cards[1].time_status, None);
    }

    #[tokio::test]
    async fn test_get_card_follow_up_parses_id_comment_and_date_from_html() {
        let mut server = mockito::Server::new_async().await;
        let html = r##"
            <div class="media">
              <a name="c7709"></a>
              <div class="media-body">
                <div class="comment pb-2">
                  <div class="content">Se pospone</div>
                </div>
                <div class="row d-flex align-items-center pb-3 ml-0">
                  <form action="/followup/13310/cancel" method="post" class="d-inline-block">
                    <button type="submit"><i class="fa fa-trash"></i></button>
                  </form>
                  <form action="/followup/13310/complete" method="post" class="d-inline-block ml-1">
                    <button type="submit"><i class="fa fa-check"></i></button>
                  </form>
                  <spam class="text-5">
                    <i class="far fa-calendar"></i>&nbsp;Aug. 25, 2026, 1 p.m.
                  </spam>
                </div>
              </div>
            </div>
        "##;
        let _m = server
            .mock("GET", "/operations/447/tasks/10716/")
            .with_status(200)
            .with_body(html)
            .create_async()
            .await;
        let client = authenticated_client(&server.url());

        let follow_up = client
            .get_card_follow_up(10716, Some(447))
            .await
            .expect("follow up")
            .expect("some follow up");

        assert_eq!(follow_up.id, 13310);
        assert_eq!(follow_up.comment_id, 7709);
        assert_eq!(follow_up.date, "Aug. 25, 2026, 1 p.m.");
    }

    #[tokio::test]
    async fn test_get_card_follow_up_ignores_date_locale_and_wrapper_typo() {
        // El sitio localiza esta fecha en español ("7 de Septiembre de 2026 a
        // las 08:00", visto en la card real) a diferencia del AP style en
        // inglés de daily-work, y a veces envuelve el ícono en un tag mal
        // escrito ("<spam>"). El parseo no debe asumir ni el formato de fecha
        // ni el nombre del tag — solo el ícono `i.fa-calendar`.
        let mut server = mockito::Server::new_async().await;
        let html = r##"
            <div class="media">
              <a name="c9001"></a>
              <div class="row d-flex align-items-center pb-3 ml-0">
                <form action="/followup/20/cancel" method="post"></form>
                <form action="/followup/20/complete" method="post"></form>
                <span class="text-5"><i class="far fa-calendar"></i>&nbsp;7 de Septiembre de 2026 a las 08:00</span>
              </div>
            </div>
        "##;
        let _m = server
            .mock("GET", "/operations/447/tasks/10716/")
            .with_status(200)
            .with_body(html)
            .create_async()
            .await;
        let client = authenticated_client(&server.url());

        let follow_up = client
            .get_card_follow_up(10716, Some(447))
            .await
            .expect("follow up")
            .expect("some follow up");

        assert_eq!(follow_up.date, "7 de Septiembre de 2026 a las 08:00");
    }

    #[tokio::test]
    async fn test_get_card_follow_up_returns_none_without_followup_form() {
        let mut server = mockito::Server::new_async().await;
        let html = r#"<div class="media"><a name="c1"></a><div class="comment pb-2"><div class="content">hola</div></div></div>"#;
        let _m = server
            .mock("GET", "/operations/447/tasks/10716/")
            .with_status(200)
            .with_body(html)
            .create_async()
            .await;
        let client = authenticated_client(&server.url());

        let follow_up = client
            .get_card_follow_up(10716, Some(447))
            .await
            .expect("follow up");

        assert!(follow_up.is_none());
    }

    #[tokio::test]
    async fn test_complete_follow_up_sends_csrf_form_post() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("POST", "/followup/13310/complete")
            .match_body(Matcher::Regex("csrfmiddlewaretoken=csrf".to_string()))
            .with_status(200)
            .with_body("ok")
            .create_async()
            .await;
        let client = authenticated_client(&server.url());

        client.complete_follow_up(13310).await.expect("complete");
    }

    #[tokio::test]
    async fn test_cancel_follow_up_sends_csrf_form_post() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("POST", "/followup/13310/cancel")
            .match_body(Matcher::Regex("csrfmiddlewaretoken=csrf".to_string()))
            .with_status(200)
            .with_body("ok")
            .create_async()
            .await;
        let client = authenticated_client(&server.url());

        client.cancel_follow_up(13310).await.expect("cancel");
    }

    #[test]
    fn task_family_from_title_url_and_parse() {
        assert_eq!(
            TaskFamily::from_title_url("/operations/447/tasks/1/"),
            Some(TaskFamily::Operation)
        );
        assert_eq!(
            TaskFamily::from_title_url("/projects/335/tasks/6005/"),
            Some(TaskFamily::Project)
        );
        assert_eq!(TaskFamily::from_title_url("/other/"), None);
        assert_eq!(TaskFamily::parse("op").unwrap(), TaskFamily::Operation);
        assert_eq!(TaskFamily::parse("project").unwrap(), TaskFamily::Project);
        assert!(TaskFamily::parse("board").is_err());
    }

    #[tokio::test]
    async fn test_resolve_task_family_from_title_url() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/operation/care/tasks/6005/")
            .match_query(Matcher::UrlEncoded(
                "format".to_string(),
                "json".to_string(),
            ))
            .with_status(200)
            .with_body(
                r#"{"id":6005,"name":"t","labels":[],"list":null,"title_url":"/projects/335/tasks/6005/"}"#,
            )
            .create_async()
            .await;
        let client = authenticated_client(&server.url());

        assert_eq!(
            client.resolve_task_family(6005).await.expect("family"),
            TaskFamily::Project
        );
    }

    #[tokio::test]
    async fn test_close_task_posts_operation_family() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("POST", "/operation/care/tasks/42/close/")
            .with_status(200)
            .with_body(r#"{"status":"The task has been closed"}"#)
            .create_async()
            .await;
        let client = authenticated_client(&server.url());

        let msg = client
            .close_task(42, TaskFamily::Operation)
            .await
            .expect("close");
        assert_eq!(msg, "The task has been closed");
    }

    #[tokio::test]
    async fn test_reopen_task_posts_project_family() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("POST", "/project-new/care/tasks/6005/reopen/")
            .with_status(200)
            .with_body(r#"{"status":"The task has been reopened"}"#)
            .create_async()
            .await;
        let client = authenticated_client(&server.url());

        let msg = client
            .reopen_task(6005, TaskFamily::Project)
            .await
            .expect("reopen");
        assert_eq!(msg, "The task has been reopened");
    }

    #[tokio::test]
    async fn test_probe_auth_returns_unauthorized_for_forbidden_status() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/operation/care/operations/")
            .match_query(Matcher::AllOf(vec![
                Matcher::UrlEncoded("format".to_string(), "datatables".to_string()),
                Matcher::UrlEncoded("status".to_string(), "Execution".to_string()),
            ]))
            .with_status(403)
            .create_async()
            .await;
        let client = authenticated_client(&server.url());

        match client.probe_auth().await.expect("auth check") {
            AuthProbe::Unauthorized => {}
            AuthProbe::Authorized(_) => panic!("expected unauthorized"),
        }
    }

    #[tokio::test]
    async fn test_probe_auth_returns_boards_when_authorized() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/operation/care/operations/")
            .match_query(Matcher::AllOf(vec![
                Matcher::UrlEncoded("format".to_string(), "datatables".to_string()),
                Matcher::UrlEncoded("status".to_string(), "Execution".to_string()),
            ]))
            .with_status(200)
            .with_body(
                r#"{"data":[{"id":12,"name":"Solo","service":"Managed","total_tasks":1,"total_tasks_closed":0}]}"#,
            )
            .create_async()
            .await;
        let client = authenticated_client(&server.url());

        match client.probe_auth().await.expect("auth check") {
            AuthProbe::Authorized(boards) => {
                assert_eq!(boards.len(), 1);
                assert_eq!(boards[0].id, 12);
                assert_eq!(boards[0].name, "Solo");
            }
            AuthProbe::Unauthorized => panic!("expected authorized"),
        }
    }

    #[tokio::test]
    async fn test_check_in_posts_json_and_parses_response() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("POST", "/tr/checkworkshifs/check_in/")
            .match_body("{}")
            .match_header("content-type", "application/json")
            .with_status(200)
            .with_body(r#"{"check_in":"2026-08-28T09:00:00","shift_id":7}"#)
            .create_async()
            .await;
        let client = authenticated_client(&server.url());

        let result = client.check_in().await.expect("check in");

        assert_eq!(result.check_in.as_deref(), Some("2026-08-28T09:00:00"));
        assert_eq!(result.shift_id, Some(7));
        assert_eq!(result.check_out, None);
    }

    #[test]
    fn test_extract_comment_tokens_requires_all_fields() {
        let client = HteamClient::new(Config::default()).expect("client");
        let html = r#"
            <input name="timestamp" value="123">
            <input name="security_hash" value="abc">
            <input name="csrfmiddlewaretoken" value="csrf">
        "#;

        assert_eq!(
            client.extract_comment_tokens(html),
            Some(("123".to_string(), "abc".to_string(), "csrf".to_string()))
        );
        assert_eq!(
            client.extract_comment_tokens("<input name=\"timestamp\" value=\"123\">"),
            None
        );
    }

    #[test]
    fn parse_task_edit_form_reads_selected_fields() {
        let html = r#"<html><body><form>
<input type="hidden" name="csrfmiddlewaretoken" value="abc">
<input name="name" value="T">
<textarea name="description">hello</textarea>
<input name="time_estimated" value="1h 30m">
<input name="start_date" value="2026-10-13">
<select name="dependence"><option value="">-</option><option value="8965" selected>X</option></select>
<select name="responsible"><option value="140" selected>u</option></select>
<select name="priority"><option value="4" selected>High</option></select>
</form></body></html>"#;
        let b = parse_task_edit_form(html);
        assert_eq!(b.csrf.as_deref(), Some("abc"));
        assert_eq!(b.time_estimated, "1h 30m");
        assert_eq!(b.dependence, "8965");
        assert_eq!(b.responsible, "140");
        assert_eq!(b.priority, "4");
        assert_eq!(b.start_date, "2026-10-13");
        assert_eq!(b.description, "hello");
    }

    #[test]
    fn dependence_id_from_detail_ignores_label_string() {
        assert_eq!(
            dependence_id_from_detail(&Some(serde_json::json!(8965))).as_deref(),
            Some("8965")
        );
        assert_eq!(
            dependence_id_from_detail(&Some(serde_json::json!("#5 - Lectura"))),
            None
        );
    }
    #[test]
    fn form_date_yyyy_mm_dd_truncates_iso() {
        assert_eq!(
            form_date_yyyy_mm_dd(Some("2026-10-13T00:00:00-06:00")),
            "2026-10-13"
        );
        assert_eq!(form_date_yyyy_mm_dd(Some("2026-10-13")), "2026-10-13");
        assert_eq!(form_date_yyyy_mm_dd(None), "");
    }

    #[test]
    fn form_user_id_from_number_or_object() {
        assert_eq!(form_user_id(Some(&serde_json::json!(140))), "140");
        assert_eq!(
            form_user_id(Some(&serde_json::json!({"id": 140, "username": "a"}))),
            "140"
        );
        assert_eq!(form_user_id(None), "");
    }
}
