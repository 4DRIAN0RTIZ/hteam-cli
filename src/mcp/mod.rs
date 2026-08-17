use std::sync::Arc;

use anyhow::Result;
use rmcp::{
    ServiceExt,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content},
    schemars::{self, JsonSchema},
    tool, tool_router,
    transport::stdio,
    ErrorData as McpError,
};
use serde::Deserialize;

use crate::client::HteamClient;
use crate::operations::{self, Session};

// ── helpers ────────────────────────────────────────────────────────────────

fn ok(value: impl serde::Serialize) -> Result<CallToolResult, McpError> {
    let json = serde_json::to_string_pretty(&value)
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;
    Ok(CallToolResult::success(vec![Content::text(json)]))
}

fn err(e: anyhow::Error) -> McpError {
    McpError::internal_error(e.to_string(), None)
}

// ── parameter structs ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
struct BoardParam {
    #[schemars(description = "ID del board (usa el configurado por defecto si se omite)")]
    board: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ListCardsParams {
    #[schemars(description = "ID de la lista")]
    list_id: u64,
    #[schemars(description = "ID del board (opcional)")]
    board: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct CardParams {
    #[schemars(description = "ID del card / task")]
    card_id: u64,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct CardLabelsParams {
    #[schemars(description = "ID del card")]
    card_id: u64,
    #[schemars(description = "ID del board (opcional)")]
    board: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct CreateCardParams {
    #[schemars(description = "Nombre del card")]
    name: String,
    #[schemars(description = "ID de la lista donde crear el card (opcional)")]
    list_id: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct MoveCardParams {
    #[schemars(description = "ID del card")]
    card_id: u64,
    #[schemars(description = "ID de la lista destino")]
    to_list: u64,
    #[schemars(description = "ID de la lista origen (default: 1 = Open)")]
    from_list: Option<u64>,
    #[schemars(description = "ID del board (opcional)")]
    board: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct UpdateDescParams {
    #[schemars(description = "ID del card")]
    card_id: u64,
    #[schemars(description = "Nueva descripción")]
    description: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct UpdateCardParams {
    #[schemars(description = "ID del card")]
    card_id: u64,
    #[schemars(description = "ID del board")]
    board: u64,
    #[schemars(description = "Nuevo nombre (opcional)")]
    name: Option<String>,
    #[schemars(description = "Nueva descripción (opcional)")]
    description: Option<String>,
    #[schemars(description = "Prioridad: 1=alta 2=media 3=baja (opcional)")]
    priority: Option<String>,
    #[schemars(description = "Username del responsable (opcional)")]
    responsible: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct CommentParams {
    #[schemars(description = "ID del card")]
    card_id: u64,
    #[schemars(description = "Texto del comentario")]
    comment: String,
    #[schemars(description = "ID del board (opcional)")]
    board: Option<u64>,
    #[schemars(description = "Marcar el comentario para seguimiento (recibir notificaciones de respuestas); false por defecto")]
    follow: Option<bool>,
    #[schemars(description = "Fecha del comentario, formato \"YYYY-MM-DD HH:MM\" en hora local (opcional; por defecto ahora)")]
    date: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct WorkingStopParams {
    #[schemars(description = "ID del registro working-on-it")]
    working_id: u64,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ProjectParams {
    #[schemars(description = "ID del proyecto")]
    project_id: u64,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct UsersSearchParams {
    #[schemars(description = "Texto a buscar en usernames")]
    query: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct DailyWorkParams {
    #[schemars(description = "Filtrar por username (opcional)")]
    user: Option<String>,
    #[schemars(description = "Tipo de actividad: time, tracking, objectives (opcional)")]
    activity_type: Option<String>,
    #[schemars(description = "Rango de tiempo: today, this_week, this_month (default: today)")]
    range: Option<String>,
}

// ── server ─────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct HteamMcpServer {
    client: Arc<HteamClient>,
}

impl HteamMcpServer {
    pub fn new(client: HteamClient) -> Self {
        Self { client: Arc::new(client) }
    }
}

#[tool_router(server_handler)]
impl HteamMcpServer {
    // ── boards / listas ──────────────────────────────────────────────────

    #[tool(description = "Obtener las listas del board activo")]
    async fn lists(
        &self,
        Parameters(BoardParam { board }): Parameters<BoardParam>,
    ) -> Result<CallToolResult, McpError> {
        let lists = operations::cards::list_lists(&self.client, board).await.map_err(err)?;
        ok(lists)
    }

    // ── cards ────────────────────────────────────────────────────────────

    #[tool(description = "Obtener cards de una lista específica")]
    async fn cards_list(
        &self,
        Parameters(ListCardsParams { list_id, board }): Parameters<ListCardsParams>,
    ) -> Result<CallToolResult, McpError> {
        let (cards, _) = operations::cards::list_cards(&self.client, list_id, board).await.map_err(err)?;
        ok(cards)
    }

    #[tool(description = "Obtener cards abiertos (lista 1)")]
    async fn cards_open(
        &self,
        Parameters(BoardParam { board }): Parameters<BoardParam>,
    ) -> Result<CallToolResult, McpError> {
        let cards = operations::cards::open_cards(&self.client, board).await.map_err(err)?;
        ok(cards)
    }

    #[tool(description = "Obtener cards cerrados (lista 2)")]
    async fn cards_closed(
        &self,
        Parameters(BoardParam { board }): Parameters<BoardParam>,
    ) -> Result<CallToolResult, McpError> {
        let cards = operations::cards::closed_cards(&self.client, board).await.map_err(err)?;
        ok(cards)
    }

    #[tool(description = "Obtener el detalle completo de un card por su ID")]
    async fn card_detail(
        &self,
        Parameters(CardParams { card_id }): Parameters<CardParams>,
    ) -> Result<CallToolResult, McpError> {
        let detail = operations::cards::card_detail(&self.client, card_id).await.map_err(err)?;
        ok(detail)
    }

    #[tool(description = "Obtener los labels disponibles de un card")]
    async fn card_labels(
        &self,
        Parameters(CardLabelsParams { card_id, board }): Parameters<CardLabelsParams>,
    ) -> Result<CallToolResult, McpError> {
        let labels = operations::cards::card_labels(&self.client, card_id, board).await.map_err(err)?;
        ok(labels)
    }

    #[tool(description = "Crear un nuevo card en el board")]
    async fn card_create(
        &self,
        Parameters(CreateCardParams { name, list_id }): Parameters<CreateCardParams>,
    ) -> Result<CallToolResult, McpError> {
        let card = operations::cards::create_card(&self.client, &name, list_id)
            .await
            .map_err(err)?;
        ok(card)
    }

    #[tool(description = "Mover un card de una lista a otra")]
    async fn card_move(
        &self,
        Parameters(MoveCardParams { card_id, to_list, from_list, board }): Parameters<MoveCardParams>,
    ) -> Result<CallToolResult, McpError> {
        operations::cards::move_card(&self.client, card_id, from_list, to_list, board)
            .await
            .map_err(err)?;
        ok(serde_json::json!({"success": true, "card_id": card_id, "to_list": to_list}))
    }

    #[tool(description = "Actualizar la descripción de un card (PATCH rápido)")]
    async fn card_update_desc(
        &self,
        Parameters(UpdateDescParams { card_id, description }): Parameters<UpdateDescParams>,
    ) -> Result<CallToolResult, McpError> {
        operations::cards::update_description(&self.client, card_id, &description)
            .await
            .map_err(err)?;
        ok(serde_json::json!({"success": true, "card_id": card_id}))
    }

    #[tool(description = "Actualizar nombre, descripción, prioridad o responsable de un card")]
    async fn card_update(
        &self,
        Parameters(UpdateCardParams { card_id, board, name, description, priority, responsible }): Parameters<UpdateCardParams>,
    ) -> Result<CallToolResult, McpError> {
        operations::cards::update_card(
            &self.client, card_id, board,
            name.as_deref(), description.as_deref(),
            priority.as_deref(), responsible.as_deref(),
        )
            .await
            .map_err(err)?;
        ok(serde_json::json!({"success": true, "card_id": card_id}))
    }

    #[tool(description = "Obtener los comentarios de un card")]
    async fn card_comments(
        &self,
        Parameters(CardParams { card_id }): Parameters<CardParams>,
    ) -> Result<CallToolResult, McpError> {
        let comments = operations::comments::list_comments(&self.client, card_id).await.map_err(err)?;
        ok(comments)
    }

    #[tool(description = "Agregar un comentario a un card")]
    async fn card_comment(
        &self,
        Parameters(CommentParams { card_id, comment, board, follow, date }): Parameters<CommentParams>,
    ) -> Result<CallToolResult, McpError> {
        operations::comments::post_comment(&self.client, card_id, &comment, board, follow.unwrap_or(false), date.as_deref())
            .await
            .map_err(err)?;
        ok(serde_json::json!({"success": true, "card_id": card_id}))
    }

    #[tool(description = "Crear un recordatorio para un card")]
    async fn card_remind(
        &self,
        Parameters(CardParams { card_id }): Parameters<CardParams>,
    ) -> Result<CallToolResult, McpError> {
        operations::reminders::create(&self.client, card_id).await.map_err(err)?;
        ok(serde_json::json!({"success": true, "card_id": card_id}))
    }

    // ── working on it ────────────────────────────────────────────────────

    #[tool(description = "Listar los cards en los que estás trabajando actualmente")]
    async fn working_list(&self) -> Result<CallToolResult, McpError> {
        let working = operations::working::list_working(&self.client).await.map_err(err)?;
        ok(working)
    }

    #[tool(description = "Marcar que estás trabajando en un card")]
    async fn working_start(
        &self,
        Parameters(CardParams { card_id }): Parameters<CardParams>,
    ) -> Result<CallToolResult, McpError> {
        operations::working::start_working(&self.client, card_id).await.map_err(err)?;
        ok(serde_json::json!({"success": true, "card_id": card_id}))
    }

    #[tool(description = "Dejar de trabajar en un card (requiere el ID del registro working-on-it, no del card)")]
    async fn working_stop(
        &self,
        Parameters(WorkingStopParams { working_id }): Parameters<WorkingStopParams>,
    ) -> Result<CallToolResult, McpError> {
        operations::working::stop_working(&self.client, working_id).await.map_err(err)?;
        ok(serde_json::json!({"success": true, "working_id": working_id}))
    }

    // ── proyectos ────────────────────────────────────────────────────────

    #[tool(description = "Ver el progreso de milestones de un proyecto")]
    async fn project_milestones(
        &self,
        Parameters(ProjectParams { project_id }): Parameters<ProjectParams>,
    ) -> Result<CallToolResult, McpError> {
        let milestones = operations::projects::milestones(&self.client, project_id).await.map_err(err)?;
        ok(milestones)
    }

    #[tool(description = "Ver las tasks de un proyecto con su estado y responsable")]
    async fn project_tasks(
        &self,
        Parameters(ProjectParams { project_id }): Parameters<ProjectParams>,
    ) -> Result<CallToolResult, McpError> {
        let resp = operations::projects::tasks(&self.client, project_id).await.map_err(err)?;
        ok(resp)
    }

    // ── usuarios / misc ──────────────────────────────────────────────────

    #[tool(description = "Buscar usuarios por username o nombre")]
    async fn users_search(
        &self,
        Parameters(UsersSearchParams { query }): Parameters<UsersSearchParams>,
    ) -> Result<CallToolResult, McpError> {
        let users = operations::users::search(&self.client, &query).await.map_err(err)?;
        ok(users)
    }

    #[tool(description = "Ver recordatorios pendientes")]
    async fn reminders(&self) -> Result<CallToolResult, McpError> {
        let items = operations::reminders::list(&self.client).await.map_err(err)?;
        ok(items)
    }

    #[tool(description = "Registrar entrada del día (check in)")]
    async fn check_in(&self) -> Result<CallToolResult, McpError> {
        let result = operations::checkin::check_in(&self.client).await.map_err(err)?;
        ok(result)
    }

    #[tool(description = "Ver el historial de trabajo diario del equipo (daily work log)")]
    async fn daily_work(
        &self,
        Parameters(DailyWorkParams { user, activity_type, range }): Parameters<DailyWorkParams>,
    ) -> Result<CallToolResult, McpError> {
        let entries = operations::daily_work::history(
            &self.client, user.as_deref(), activity_type.as_deref(), range.as_deref(),
        )
            .await
            .map_err(err)?;
        ok(entries)
    }
}

// ── entrypoint ─────────────────────────────────────────────────────────────

pub async fn run() -> Result<()> {
    let session = Session::open().await?;
    let server = HteamMcpServer::new(session.client);

    let service = server.serve(stdio()).await
        .inspect_err(|e| eprintln!("MCP server error: {e:?}"))?;

    service.waiting().await
        .inspect_err(|e| eprintln!("MCP service error: {e:?}"))?;

    Ok(())
}
