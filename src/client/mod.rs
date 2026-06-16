use anyhow::{Context, Result};
use reqwest::Client;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE, COOKIE, REFERER};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::config::Config;
use crate::models::{
    Card, CardDetail, CheckInResult, Comment, CommentsResponse, Label, List, ProjectMilestone,
    ProjectTasksResponse, Reminder, UserSuggestion, WorkingOnStatus,
};

const BASE_URL: &str = "https://hteam.mx/api";
const SITE_URL: &str = "https://hteam.mx";

pub struct HteamClient {
    client: Client,
    config: Arc<Mutex<Config>>,
}

impl HteamClient {
    pub fn new(config: Config) -> Result<Self> {
        let client = Client::builder()
            // Don't use cookie store, we'll handle cookies manually
            .user_agent("curl/7.81.0")  // Match curl's user-agent
            .build()
            .context("Error al crear el cliente HTTP")?;

        Ok(Self {
            client,
            config: Arc::new(Mutex::new(config)),
        })
    }

    pub async fn with_auth(config: Config) -> Result<Self> {
        if !config.is_authenticated() {
            anyhow::bail!("No autenticado. Ejecuta 'hteam login' primero.");
        }
        Self::new(config)
    }

    fn build_headers(&self, config: &Config) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();

        headers.insert(REFERER, HeaderValue::from_static("https://hteam.mx/"));

        if let (Some(session_id), Some(csrf_token)) = (&config.auth.session_id, &config.auth.csrf_token) {
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
        headers.insert("X-Requested-With", HeaderValue::from_static("XMLHttpRequest"));
        
        // Add Accept header like curl does
        headers.insert(reqwest::header::ACCEPT, HeaderValue::from_static("*/*"));

        Ok(headers)
    }

    async fn get_board_number(&self) -> Result<u64> {
        let config = self.config.lock().await;
        config.get_board_number()
            .or_else(|| config.load_last_ticket().ok().flatten())
            .context("No se especificó el board number. Usa --board o configura uno por defecto.")
    }

    pub async fn test_auth(&self) -> Result<bool> {
        // Try to get lists from a default board (483 is common)
        // This endpoint should work with cookie authentication
        let config = self.config.lock().await;
        let board = config.get_board_number().unwrap_or(483);
        let url = format!("{}/operation/care/operations/{}/lists/?format=json", BASE_URL, board);
        let headers = self.build_headers(&config)?;
        
        let response = self.client
            .get(&url)
            .headers(headers)
            .send()
            .await?;

        Ok(response.status().is_success())
    }

    pub async fn get_lists(&self, board_number: Option<u64>) -> Result<Vec<List>> {
        let board = match board_number {
            Some(b) => b,
            None => self.get_board_number().await?,
        };

        let config = self.config.lock().await;
        let url = format!("{}/operation/care/operations/{}/lists/?format=json", BASE_URL, board);
        let headers = self.build_headers(&config)?;

        let response = self.client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .context("Error al obtener listas")?;

        if !response.status().is_success() {
            anyhow::bail!("Error HTTP {}: {}", response.status(), response.text().await?);
        }

        let lists: Vec<List> = response.json().await?;
        Ok(lists)
    }

    pub async fn get_cards(&self, list_id: u64, board_number: Option<u64>) -> Result<(Vec<Card>, Option<u64>)> {
        let board = match board_number {
            Some(b) => b,
            None => self.get_board_number().await?,
        };

        let config = self.config.lock().await;
        let url = format!("{}/operation/care/operations/{}/lists/{}/cards/?format=json", BASE_URL, board, list_id);
        let headers = self.build_headers(&config)?;

        let response = self.client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .context("Error al obtener cards")?;

        if !response.status().is_success() {
            anyhow::bail!("Error HTTP {}: {}", response.status(), response.text().await?);
        }

        let card_response: crate::models::CardListResponse = response.json().await?;
        
        // Extract board_id from first entry if available
        let board_id = card_response.cardlist_list
            .first()
            .map(|entry| entry.board_id);
        
        let cards: Vec<Card> = card_response.cardlist_list
            .into_iter()
            .map(|entry| entry.card)
            .collect();
            
        Ok((cards, board_id))
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
        let url = format!("{}/operation/care/tasks/{}/?format=json", BASE_URL, card_id);
        let headers = self.build_headers(&config)?;

        let response = self.client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .context("Error al obtener detalle de card")?;

        if !response.status().is_success() {
            anyhow::bail!("Error HTTP {}: {}", response.status(), response.text().await?);
        }

        let detail: CardDetail = response.json().await?;
        Ok(detail)
    }

    pub async fn get_card_comments(&self, card_id: u64) -> Result<Vec<Comment>> {
        let config = self.config.lock().await;
        let url = format!("{}/comments/api/processes-task/{}/", SITE_URL, card_id);
        let headers = self.build_headers(&config)?;

        let response = self.client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .context("Error al obtener comentarios")?;

        if !response.status().is_success() {
            anyhow::bail!("Error HTTP {}: {}", response.status(), response.text().await?);
        }

        let resp: CommentsResponse = response.json().await?;
        Ok(resp.results)
    }

    pub async fn get_board_id(&self) -> Result<u64> {
        {
            let config = self.config.lock().await;
            if let Some(bid) = config.auth.board_id {
                return Ok(bid);
            }
        }
        // Fetch from any list (list 1 = Open) to extract board_id
        let (_, board_id) = self.get_cards(1, None).await?;
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
        let url = format!("{}/tr/workingonit/user/", BASE_URL);
        let headers = self.build_headers(&config)?;
        drop(config);

        let response = self.client.get(&url).headers(headers).send().await?;
        let text = response.text().await?;

        if let Ok(array) = serde_json::from_str::<Vec<crate::models::WorkingOnResponse>>(&text) {
            if let Some(first) = array.first() {
                let uid = first.user;
                let mut config = self.config.lock().await;
                config.auth.user_id = Some(uid);
                return Ok(uid);
            }
        }
        if let Ok(single) = serde_json::from_str::<crate::models::WorkingOnResponse>(&text) {
            let uid = single.user;
            let mut config = self.config.lock().await;
            config.auth.user_id = Some(uid);
            return Ok(uid);
        }

        anyhow::bail!("No se pudo obtener el user_id del usuario autenticado")
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

        let config = self.config.lock().await;
        let url = format!("{}/boards/care/boards/{}/create_card/", BASE_URL, board_id);
        let headers = self.build_headers(&config)?;

        let labels: Vec<u64> = list_id.into_iter().collect();
        let body = serde_json::json!({
            "name": name,
            "complement": user_id,
            "process": board_number,
            "labels": labels,
        });
        


        let response = self.client
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
                worker: None,
            });
        }

        serde_json::from_str::<Card>(&response_text)
            .with_context(|| format!("Respuesta inesperada del API al crear card: {:?}", &response_text[..response_text.len().min(300)]))
    }

    pub async fn move_card(
        &self,
        card_id: u64,
        from_list: u64,
        to_list: u64,
        board_number: Option<u64>,
    ) -> Result<()> {
        // Use hardcoded board_id 483 for POST operations (like create_card)
        let board = board_number.unwrap_or(483);

        let config = self.config.lock().await;
        let url = format!("{}/boards/care/labels/update_card_position/", BASE_URL);
        let headers = self.build_headers(&config)?;

        let body = serde_json::json!({
            "board_id": board,
            "card_id": card_id,
            "from_list": from_list,
            "to_list": to_list,
            "card_type": 0,
        });

        let response = self.client
            .post(&url)
            .headers(headers)
            .header(CONTENT_TYPE, "application/json")
            .json(&body)
            .send()
            .await
            .context("Error al mover card")?;

        if !response.status().is_success() {
            anyhow::bail!("Error HTTP {}: {}", response.status(), response.text().await?);
        }

        Ok(())
    }

    pub async fn update_card_description(&self, card_id: u64, description: &str) -> Result<()> {
        let board_number = self.get_board_number().await?;
        let detail = self.get_card_detail(card_id).await?;
        self.update_card_full(card_id, board_number, &detail, None, Some(description), None, None).await
    }

    pub async fn get_working_on(&self) -> Result<Vec<WorkingOnStatus>> {
        let config = self.config.lock().await;
        let url = format!("{}/tr/workingonit/user/", BASE_URL);
        let headers = self.build_headers(&config)?;

        let response = self.client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .context("Error al obtener working on")?;

        if !response.status().is_success() {
            anyhow::bail!("Error HTTP {}: {}", response.status(), response.text().await?);
        }

        // Try to parse as array first, then as single object
        let response_text = response.text().await?;
        
        // Check if response is empty or null
        if response_text.trim().is_empty() || response_text.trim() == "null" {
            return Ok(vec![]);
        }
        
        // Try to parse as array
        if let Ok(array) = serde_json::from_str::<Vec<crate::models::WorkingOnResponse>>(&response_text) {
            return Ok(array.into_iter().map(|resp| resp.into()).collect());
        }
        
        // Try to parse as single object
        if let Ok(single) = serde_json::from_str::<crate::models::WorkingOnResponse>(&response_text) {
            return Ok(vec![single.into()]);
        }
        
        anyhow::bail!("No se pudo parsear la respuesta de working on: {}", &response_text[..response_text.len().min(200)])
    }

    pub async fn start_working(&self, card_id: u64) -> Result<()> {
        let config = self.config.lock().await;
        let url = format!("{}/tr/workingonit/", BASE_URL);
        let headers = self.build_headers(&config)?;

        let body = format!("object_id={}&content_type=99", card_id);

        let response = self.client
            .post(&url)
            .headers(headers)
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded; charset=UTF-8")
            .header("X-Requested-With", "XMLHttpRequest")
            .body(body)
            .send()
            .await
            .context("Error al iniciar working on")?;

        if !response.status().is_success() {
            anyhow::bail!("Error HTTP {}: {}", response.status(), response.text().await?);
        }

        Ok(())
    }

    pub async fn stop_working(&self, working_id: u64) -> Result<()> {
        let config = self.config.lock().await;
        let url = format!("{}/tr/workingonit/{}/content_type/99/", BASE_URL, working_id);
        let headers = self.build_headers(&config)?;

        let body = format!("object_id={}&content_type=99&state=4", working_id);

        let response = self.client
            .put(&url)
            .headers(headers)
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded; charset=UTF-8")
            .header("X-Requested-With", "XMLHttpRequest")
            .body(body)
            .send()
            .await
            .context("Error al detener working on")?;

        if !response.status().is_success() {
            anyhow::bail!("Error HTTP {}: {}", response.status(), response.text().await?);
        }

        Ok(())
    }

    pub async fn post_comment(&self, card_id: u64, comment: &str, board_number: Option<u64>) -> Result<()> {
        let board = board_number.unwrap_or(483);

        let config = self.config.lock().await;
        let task_url = format!("{}/operations/{}/tasks/{}/", SITE_URL, board, card_id);
        let headers = self.build_headers(&config)?;

        let html_response = self.client
            .get(&task_url)
            .headers(headers.clone())
            .send()
            .await
            .context("Error al obtener página de card")?;

        if !html_response.status().is_success() {
            anyhow::bail!("Error HTTP {} al obtener página de card", html_response.status());
        }

        let html = html_response.text().await?;

        let (timestamp, security_hash, csrf) = self.extract_comment_tokens(&html)
            .context("No se pudo extraer el formulario de comentarios de la página")?;

        let encode = |s: &str| {
            let mut out = String::new();
            for b in s.bytes() {
                match b {
                    b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
                    b' ' => out.push('+'),
                    _ => out.push_str(&format!("%{:02X}", b)),
                }
            }
            out
        };

        let now = chrono::Utc::now().format("%Y-%m-%d %H:%M").to_string();
        let body = [
            format!("csrfmiddlewaretoken={}", encode(&csrf)),
            format!("next=%2Fcomments%2Fsent%2F"),
            format!("content_type=processes.task"),
            format!("object_pk={}", card_id),
            format!("timestamp={}", timestamp),
            format!("security_hash={}", security_hash),
            format!("reply_to=0"),
            format!("honeypot="),
            format!("comment={}", encode(comment)),
            format!("date={}", encode(&now)),
        ].join("&");

        let comment_url = format!("{}/comments/post/", SITE_URL);

        let response = self.client
            .post(&comment_url)
            .headers(headers)
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header("Origin", SITE_URL)
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
        let board = board_id.unwrap_or_else(|| {
            self.config.try_lock().ok()
                .and_then(|c| c.get_board_number())
                .unwrap_or(483)
        });

        let config = self.config.lock().await;
        let url = format!("{}/boards/care/boards/{}/card/{}/labels/?format=json", BASE_URL, board, card_id);
        let headers = self.build_headers(&config)?;

        let response = self.client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .context("Error al obtener labels")?;

        if !response.status().is_success() {
            anyhow::bail!("Error HTTP {}: {}", response.status(), response.text().await?);
        }

        let labels: Vec<Label> = response.json().await?;
        Ok(labels)
    }

    pub async fn get_project_milestones(&self, project_id: u64) -> Result<Vec<ProjectMilestone>> {
        let config = self.config.lock().await;
        let url = format!("{}/project-new/care/projects/{}/milestones_progress/", BASE_URL, project_id);
        let headers = self.build_headers(&config)?;

        let response = self.client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .context("Error al obtener milestones")?;

        if !response.status().is_success() {
            anyhow::bail!("Error HTTP {}: {}", response.status(), response.text().await?);
        }

        let milestones: Vec<ProjectMilestone> = response.json().await?;
        Ok(milestones)
    }

    pub async fn get_project_tasks(&self, project_id: u64) -> Result<ProjectTasksResponse> {
        let config = self.config.lock().await;
        let url = format!("{}/project-new/care/{}/tasks-projects/", BASE_URL, project_id);
        let headers = self.build_headers(&config)?;

        let response = self.client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .context("Error al obtener tasks de proyecto")?;

        if !response.status().is_success() {
            anyhow::bail!("Error HTTP {}: {}", response.status(), response.text().await?);
        }

        let resp: ProjectTasksResponse = response.json().await?;
        Ok(resp)
    }

    pub async fn update_card_full(
        &self,
        card_id: u64,
        board_number: u64,
        detail: &CardDetail,
        name: Option<&str>,
        description: Option<&str>,
        priority: Option<&str>,
        responsible: Option<&str>,
    ) -> Result<()> {
        let config = self.config.lock().await;
        let url = format!("{}/operations/{}/tasks/{}/edit/", SITE_URL, board_number, card_id);
        let headers = self.build_headers(&config)?;
        let csrf = config.auth.csrf_token.as_deref().unwrap_or("");

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

        let fields = vec![
            format!("csrfmiddlewaretoken={}", encode(csrf)),
            format!("process_id={}", board_number),
            format!("name={}", encode(name.unwrap_or(&detail.name))),
            format!("description={}", encode(description.unwrap_or(detail.description.as_deref().unwrap_or("")))),
            format!("time_estimated={}", encode(detail.time_estimated.as_deref().unwrap_or(""))),
            format!("start_date={}", encode(detail.start_date.as_deref().unwrap_or(""))),
            format!("dependence={}", encode(&detail.dependence.as_ref().map(val_to_str).unwrap_or_default())),
            format!("responsible={}", encode(responsible.unwrap_or(&detail.responsible.as_ref().map(val_to_str).unwrap_or_default()))),
            format!("priority={}", encode(priority.unwrap_or(&detail.priority.as_ref().map(val_to_str).unwrap_or_else(|| "3".to_string())))),
            format!("id={}", card_id),
        ];
        let body = fields.join("&");

        let response = self.client
            .post(&url)
            .headers(headers)
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header("Origin", SITE_URL)
            .header(REFERER, url.as_str())
            .body(body)
            .send()
            .await
            .context("Error al actualizar card")?;

        let code = response.status().as_u16();
        if code >= 400 {
            anyhow::bail!("Error HTTP {}: {}", code, response.text().await?);
        }

        Ok(())
    }

    pub async fn set_reminder(&self, card_id: u64) -> Result<()> {
        let config = self.config.lock().await;
        let url = format!("{}/tr/reminders/", BASE_URL);
        let headers = self.build_headers(&config)?;

        let body = serde_json::json!({
            "type": 1,
            "object_id": card_id,
            "card_type": 0,
        });

        let response = self.client
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

    pub async fn get_reminders(&self) -> Result<Vec<Reminder>> {
        let config = self.config.lock().await;
        let url = format!("{}/tr/reminders/user/", BASE_URL);
        let headers = self.build_headers(&config)?;

        let response = self.client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .context("Error al obtener reminders")?;

        if !response.status().is_success() {
            anyhow::bail!("Error HTTP {}: {}", response.status(), response.text().await?);
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

    pub async fn search_users(&self, query: &str) -> Result<Vec<UserSuggestion>> {
        let config = self.config.lock().await;
        let url = format!("{}/users/username-autocomplete/?q={}", BASE_URL, query);
        let headers = self.build_headers(&config)?;

        let response = self.client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .context("Error al buscar usuarios")?;

        if !response.status().is_success() {
            anyhow::bail!("Error HTTP {}: {}", response.status(), response.text().await?);
        }

        let resp: crate::models::UserAutocompleteResponse = response.json().await?;
        Ok(resp.results)
    }

    pub async fn check_in(&self) -> Result<CheckInResult> {
        let config = self.config.lock().await;
        let url = format!("{}/tr/checkworkshifs/check_in/", BASE_URL);
        let headers = self.build_headers(&config)?;

        let response = self.client
            .post(&url)
            .headers(headers)
            .header(CONTENT_TYPE, "application/json")
            .body("{}")
            .send()
            .await
            .context("Error al hacer check in")?;

        if !response.status().is_success() {
            anyhow::bail!("Error HTTP {}: {}", response.status(), response.text().await?);
        }

        let result: CheckInResult = response.json().await?;
        Ok(result)
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
