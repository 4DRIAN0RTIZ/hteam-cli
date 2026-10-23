use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::sync::Arc;

use tokio::sync::{mpsc, Semaphore};
use tokio::task::JoinSet;

use crate::client::HteamClient;
use crate::models::{
    BoardEntry, Card, CardDetail, Comment, FollowUp, List, ProjectMilestone, ProjectSummary,
    ProjectTask, Reminder, UserSuggestion, WeeklyObjectivesSet, WorkShiftRecord, WorkingOnStatus,
};
use crate::operations;

use super::app::App;

/// Handle used by key handlers to push network work off the input loop.
#[derive(Clone)]
pub struct Jobs {
    tx: mpsc::UnboundedSender<JobOutcome>,
}

impl Jobs {
    pub fn channel() -> (Self, mpsc::UnboundedReceiver<JobOutcome>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (Self { tx }, rx)
    }

    pub fn spawn<F>(&self, fut: F)
    where
        F: Future<Output = JobOutcome> + Send + 'static,
    {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let outcome = fut.await;
            let _ = tx.send(outcome);
        });
    }
}

/// Result of a background network job. Applied on the TUI thread so draws and
/// key handling never await HTTP.
pub enum JobOutcome {
    BoardRefresh {
        id: u64,
        board_number: u64,
        lists: Result<Vec<List>, String>,
        cards: HashMap<u64, Result<Vec<Card>, String>>,
        working_on: Option<Vec<WorkingOnStatus>>,
        user_shift: Option<Option<WorkShiftRecord>>,
    },
    MoveCard {
        id: u64,
        board_number: u64,
        card: Card,
        from_list_id: u64,
        to_list_id: u64,
        from_idx: usize,
        target_list_pos: usize,
        to_list_name: String,
        result: Result<(), String>,
    },
    Reminders {
        id: u64,
        result: Result<Vec<Reminder>, String>,
    },
    ReminderCreated {
        id: u64,
        card_name: String,
        result: Result<Vec<Reminder>, String>,
    },
    ReminderCanceled {
        id: u64,
        reminder_id: u64,
        result: Result<Vec<Reminder>, String>,
    },
    WeeklyObjectives {
        id: u64,
        result: Result<WeeklyObjectivesLoaded, String>,
    },
    Comments {
        id: u64,
        card_id: u64,
        comments: Result<Vec<Comment>, String>,
        follow_up: Result<Option<FollowUp>, String>,
    },
    FollowUp {
        id: u64,
        board_number: u64,
        list_id: u64,
        complete: bool,
        result: Result<(), String>,
        cards: Option<Result<Vec<Card>, String>>,
    },
    Mentions {
        id: u64,
        /// Query that was searched — applied only if it still matches the
        /// trailing `@token` in the comment input.
        query: String,
        result: Result<Vec<UserSuggestion>, String>,
    },
    CommentPosted {
        id: u64,
        card_id: u64,
        result: Result<Vec<Comment>, String>,
    },
    DescriptionSaved {
        id: u64,
        board_number: u64,
        list_id: u64,
        result: Result<(), String>,
        cards: Option<Result<Vec<Card>, String>>,
    },
    ProjectLoaded {
        id: u64,
        project_id: u64,
        milestones: Result<Vec<ProjectMilestone>, String>,
        tasks: Result<Vec<ProjectTask>, String>,
    },
    LiveProjects {
        id: u64,
        status_idx: usize,
        result: Result<Vec<ProjectSummary>, String>,
        open: bool,
    },
    CardCreated {
        id: u64,
        board_number: u64,
        list_id: u64,
        result: Result<(String, Vec<Card>), String>,
    },
    WorkingToggled {
        id: u64,
        message: String,
        working_on: Option<Vec<WorkingOnStatus>>,
        error: Option<String>,
    },
    CardLink {
        id: u64,
        result: Result<String, String>,
    },
    TaskDetailLoaded {
        id: u64,
        card_id: u64,
        result: Result<CardDetail, String>,
        dependence_label: Option<String>,
        dependence_id: Option<u64>,
    },
    TaskDetailSaved {
        id: u64,
        card_id: u64,
        list_id: Option<u64>,
        board_number: u64,
        result: Result<CardDetail, String>,
        dependence_label: Option<String>,
        dependence_id: Option<u64>,
        cards: Option<Result<Vec<Card>, String>>,
    },
    BoardsLoaded {
        id: u64,
        current_board: u64,
        result: Result<Vec<BoardEntry>, String>,
    },
}

pub struct WeeklyObjectivesLoaded {
    pub set: WeeklyObjectivesSet,
    pub from_cache: bool,
    pub synced_at: chrono::DateTime<chrono::Utc>,
}

/// Snapshot used by board refresh / board-switch jobs.
/// Max in-flight card-list HTTP requests during a board refresh.
const LIST_CARD_CONCURRENCY: usize = 4;

pub async fn fetch_board_snapshot(
    client: &HteamClient,
    board_number: u64,
) -> (
    Result<Vec<List>, String>,
    HashMap<u64, Result<Vec<Card>, String>>,
    Option<Vec<WorkingOnStatus>>,
    Option<Option<WorkShiftRecord>>,
) {
    let (user_shift, lists_result) = tokio::join!(
        async {
            match operations::checkin::workshift_resume(client).await {
                Ok(resume) => Some(resume.last),
                Err(_) => None,
            }
        },
        async {
            operations::cards::list_lists(client, Some(board_number))
                .await
                .map_err(|e| format!("Error cargando listas: {}", e))
        },
    );

    let lists = match lists_result {
        Ok(lists) => lists,
        Err(msg) => {
            return (Err(msg), HashMap::new(), None, user_shift);
        }
    };

    let (cards, working_on) = tokio::join!(
        fetch_list_cards_parallel(client, board_number, &lists),
        async { operations::working::list_working(client).await.ok() },
    );

    (Ok(lists), cards, working_on, user_shift)
}

async fn fetch_list_cards_parallel(
    client: &HteamClient,
    board_number: u64,
    lists: &[List],
) -> HashMap<u64, Result<Vec<Card>, String>> {
    if lists.is_empty() {
        return HashMap::new();
    }

    let semaphore = Arc::new(Semaphore::new(LIST_CARD_CONCURRENCY));
    let mut set = JoinSet::new();

    for list in lists {
        let client = client.clone();
        let list_id = list.id;
        let list_name = list.name.clone();
        let semaphore = semaphore.clone();
        set.spawn(async move {
            let _permit = semaphore
                .acquire()
                .await
                .expect("list-card semaphore stays open");
            let entry =
                match operations::cards::list_cards(&client, list_id, Some(board_number)).await {
                    Ok((list_cards, _)) => Ok(list_cards),
                    Err(e) => Err(format!("Error cargando '{}': {}", list_name, e)),
                };
            (list_id, entry)
        });
    }

    let mut cards = HashMap::with_capacity(lists.len());
    while let Some(joined) = set.join_next().await {
        match joined {
            Ok((list_id, entry)) => {
                cards.insert(list_id, entry);
            }
            Err(e) => {
                // JoinError has no list id; surface a generic failure so the
                // refresh still completes for the remaining lists.
                let _ = e;
            }
        }
    }
    cards
}

fn enrich_detail_responsible(app: &App, card_id: u64, mut detail: CardDetail) -> CardDetail {
    let bare_id = matches!(
        detail.responsible.as_ref(),
        Some(serde_json::Value::Number(_)) | Some(serde_json::Value::String(_))
    ) || detail.responsible.is_none();
    if !bare_id {
        return detail;
    }
    for cards in app.cards_by_list.values() {
        if let Some(card) = cards.iter().find(|c| c.id == card_id) {
            if let Some(ref resp) = card.responsible {
                if matches!(resp, serde_json::Value::Object(_)) {
                    detail.responsible = Some(resp.clone());
                }
            }
            break;
        }
    }
    detail
}

pub fn apply_outcome(app: &mut App, outcome: JobOutcome) {
    match outcome {
        JobOutcome::BoardRefresh {
            id,
            board_number,
            lists,
            cards,
            working_on,
            user_shift,
        } => {
            if !app
                .requests
                .is_current_board_refresh(id, board_number, app.board_number)
            {
                app.end_busy();
                return;
            }
            app.end_busy();
            if let Some(shift) = user_shift {
                app.user_shift = shift;
            }
            match lists {
                Ok(lists) => {
                    let list_ids: HashSet<u64> = lists.iter().map(|l| l.id).collect();
                    app.all_lists = lists;
                    // Drop cards for lists that no longer exist, but keep prior
                    // data for lists whose fetch failed this round.
                    app.cards_by_list.retain(|id, _| list_ids.contains(id));
                    let mut failed = Vec::new();
                    for (list_id, entry) in cards {
                        match entry {
                            Ok(list_cards) => {
                                app.cards_by_list.insert(list_id, list_cards);
                            }
                            Err(msg) => failed.push(msg),
                        }
                    }
                    if let Some(items) = working_on {
                        app.working_on = items;
                    }
                    app.clamp_card_selection();
                    if failed.is_empty() {
                        if app.all_lists.is_empty() {
                            app.set_status("Este board no tiene listas.");
                        } else if let Some(summary) = app.card_filters_summary() {
                            app.set_status(format!("Filtro: {} · X limpiar", summary));
                        } else {
                            app.clear_status();
                        }
                    } else {
                        app.set_status(failed.join(" · "));
                    }
                }
                // Keep whatever is already on screen if the lists call fails.
                Err(msg) => app.set_status(msg),
            }
        }
        JobOutcome::MoveCard {
            id,
            board_number,
            card,
            from_list_id,
            to_list_id,
            from_idx,
            target_list_pos,
            to_list_name,
            result,
        } => {
            if app.requests.move_card != id || app.board_number != board_number {
                return;
            }
            match result {
                Ok(()) => {
                    let new_len = app
                        .cards_by_list
                        .get(&to_list_id)
                        .map(|c| c.len())
                        .unwrap_or(1);
                    app.selected_card
                        .insert(to_list_id, new_len.saturating_sub(1));
                    app.selected_list = target_list_pos;
                    app.set_status(format!("Movida: {} → {}", card.name, to_list_name));
                }
                Err(e) => {
                    if let Some(cards) = app.cards_by_list.get_mut(&to_list_id) {
                        if let Some(pos) = cards.iter().position(|c| c.id == card.id) {
                            cards.remove(pos);
                        }
                    }
                    let insert_at = from_idx.min(
                        app.cards_by_list
                            .get(&from_list_id)
                            .map(|c| c.len())
                            .unwrap_or(0),
                    );
                    app.cards_by_list
                        .entry(from_list_id)
                        .or_default()
                        .insert(insert_at, card);
                    app.set_status(format!("Error moviendo card: {}", e));
                }
            }
        }
        JobOutcome::Reminders { id, result } => {
            if app.requests.reminders != id {
                app.end_busy();
                return;
            }
            app.end_busy();
            match result {
                Ok(list) => {
                    app.reminders = list;
                    app.selected_reminder = None;
                }
                Err(e) => app.set_status(format!("Error cargando reminders: {}", e)),
            }
        }
        JobOutcome::ReminderCreated {
            id,
            card_name,
            result,
        } => {
            if app.requests.reminders != id {
                app.end_busy();
                return;
            }
            app.end_busy();
            match result {
                Ok(list) => {
                    app.reminders = list;
                    app.set_status(format!("Reminder creado: {}", card_name));
                }
                Err(e) => app.set_status(format!("Error creando reminder: {}", e)),
            }
        }
        JobOutcome::ReminderCanceled {
            id,
            reminder_id,
            result,
        } => {
            if app.requests.reminders != id {
                app.end_busy();
                return;
            }
            app.end_busy();
            match result {
                Ok(list) => {
                    app.reminders = list;
                    app.selected_reminder = None;
                    app.set_status(format!("Reminder {reminder_id} cancelado."));
                }
                Err(e) => app.set_status(format!("Error cancelando reminder: {}", e)),
            }
        }
        JobOutcome::WeeklyObjectives { id, result } => {
            if app.requests.objectives != id {
                app.end_busy();
                return;
            }
            app.end_busy();
            match result {
                Ok(view) => {
                    app.weekly_objectives = Some(view.set);
                    app.weekly_objectives_from_cache = view.from_cache;
                    app.weekly_objectives_synced_at = Some(view.synced_at);
                    app.weekly_objectives_error = None;
                    app.clear_status();
                }
                Err(e) => {
                    let msg = format!("Error cargando objetivos semanales: {}", e);
                    app.weekly_objectives_error = Some(msg.clone());
                    app.weekly_objectives = None;
                    app.set_status(msg);
                }
            }
        }
        JobOutcome::Comments {
            id,
            card_id,
            comments,
            follow_up,
        } => {
            if app.requests.comments != id
                || !app.show_comments
                || app.current_card().map(|c| c.id) != Some(card_id)
            {
                app.end_busy();
                return;
            }
            app.end_busy();
            match comments {
                Ok(list) => app.comments = list,
                Err(e) => app.set_status(format!("Error cargando comentarios: {}", e)),
            }
            match follow_up {
                Ok(fu) => app.current_follow_up = fu,
                Err(e) => app.set_status(format!("Error cargando seguimiento: {}", e)),
            }
        }
        JobOutcome::FollowUp {
            id,
            board_number,
            list_id,
            complete,
            result,
            cards,
        } => {
            if app.requests.follow_up != id || app.board_number != board_number {
                app.end_busy();
                return;
            }
            app.end_busy();
            match result {
                Ok(()) => {
                    app.current_follow_up = None;
                    let verb = if complete { "completado" } else { "cancelado" };
                    app.set_status(format!("Seguimiento {}.", verb));
                    if let Some(Ok(list_cards)) = cards {
                        app.cards_by_list.insert(list_id, list_cards);
                    }
                }
                Err(e) => {
                    let verb = if complete {
                        "completando"
                    } else {
                        "cancelando"
                    };
                    app.set_status(format!("Error {} seguimiento: {}", verb, e));
                }
            }
        }
        JobOutcome::Mentions { id, query, result } => {
            if app.requests.mentions != id || !app.composing_comment {
                return;
            }
            // Drop results that no longer match what the user is typing, even
            // if the request id somehow lined up (e.g. race after accept).
            let current = operations::comments::mention_query(app.comment_input.as_str());
            if current != Some(query.as_str()) {
                return;
            }
            match result {
                Ok(list) => {
                    app.mention_suggestions = list;
                    app.mention_selected = 0;
                }
                Err(e) => {
                    app.mention_suggestions.clear();
                    app.set_status(format!("Error buscando usuarios: {}", e));
                }
            }
        }
        JobOutcome::CommentPosted {
            id,
            card_id,
            result,
        } => {
            if app.requests.comment_post != id {
                app.end_busy();
                return;
            }
            app.end_busy();
            match result {
                Ok(list) => {
                    app.set_status("Comentario publicado.");
                    super::events::cancel_composing_comment(app);
                    if app.show_comments && app.current_card().map(|c| c.id) == Some(card_id) {
                        app.comments = list;
                    }
                }
                Err(e) => app.set_status(format!("Error publicando comentario: {}", e)),
            }
        }
        JobOutcome::DescriptionSaved {
            id,
            board_number,
            list_id,
            result,
            cards,
        } => {
            if app.requests.description != id || app.board_number != board_number {
                app.end_busy();
                return;
            }
            app.end_busy();
            match result {
                Ok(()) => {
                    app.set_status("Descripción actualizada.");
                    if let Some(Ok(list_cards)) = cards {
                        app.cards_by_list.insert(list_id, list_cards);
                    }
                }
                Err(e) => app.set_status(format!("Error guardando descripción: {}", e)),
            }
        }
        JobOutcome::ProjectLoaded {
            id,
            project_id,
            milestones,
            tasks,
        } => {
            if app.requests.projects != id || app.active_project != Some(project_id) {
                app.end_busy();
                return;
            }
            app.end_busy();
            match milestones {
                Ok(list) => app.project_milestones = list,
                Err(e) => app.set_status(format!("Error cargando milestones: {}", e)),
            }
            match tasks {
                Ok(list) => app.project_tasks = list,
                Err(e) => app.set_status(format!("Error cargando tasks: {}", e)),
            }
        }
        JobOutcome::LiveProjects {
            id,
            status_idx,
            result,
            open,
        } => {
            if app.requests.live_projects != id || app.live_projects_status_idx != status_idx {
                app.end_busy();
                return;
            }
            app.end_busy();
            match result {
                Ok(projects) => {
                    app.live_projects = projects;
                    app.selected_live_project_idx = 0;
                    if open {
                        app.show_live_projects = true;
                    }
                }
                Err(e) => app.set_status(format!("Error cargando proyectos: {}", e)),
            }
        }
        JobOutcome::CardCreated {
            id,
            board_number,
            list_id,
            result,
        } => {
            if app.requests.new_card != id || app.board_number != board_number {
                app.end_busy();
                return;
            }
            app.end_busy();
            match result {
                Ok((name, list_cards)) => {
                    app.set_status(format!("Card creada: {}", name));
                    super::events::cancel_composing_card(app);
                    app.cards_by_list.insert(list_id, list_cards);
                }
                Err(e) => app.set_status(format!("Error creando card: {}", e)),
            }
        }
        JobOutcome::WorkingToggled {
            id,
            message,
            working_on,
            error,
        } => {
            if app.requests.working != id {
                app.end_busy();
                return;
            }
            app.end_busy();
            if let Some(e) = error {
                app.set_status(e);
            } else {
                if let Some(items) = working_on {
                    app.working_on = items;
                }
                app.set_status(message);
            }
        }
        JobOutcome::CardLink { id, result } => {
            if app.requests.card_link != id {
                app.end_busy();
                return;
            }
            app.end_busy();
            match result {
                Ok(url) => match operations::clipboard::copy(&url) {
                    Ok(()) => {
                        app.set_status(format!("{} Link copiado: {}", operations::COPY_ICON, url))
                    }
                    Err(e) => app.set_status(format!("Error copiando el link: {}", e)),
                },
                Err(e) => app.set_status(format!("Error obteniendo el link: {}", e)),
            }
        }
        JobOutcome::TaskDetailLoaded {
            id,
            card_id,
            result,
            dependence_label,
            dependence_id,
        } => {
            if app.requests.task_detail != id || app.task_detail_card_id != Some(card_id) {
                app.end_busy();
                return;
            }
            app.end_busy();
            match result {
                Ok(detail) => {
                    // Prefer list-card responsible object when detail only has a bare id.
                    let detail = enrich_detail_responsible(app, card_id, detail);
                    app.task_detail = Some(detail);
                    app.task_detail_dependence_label = dependence_label;
                    app.task_detail_dependence_id = dependence_id;
                    app.task_detail_error = None;
                    app.clear_status();
                }
                Err(e) => {
                    app.task_detail = None;
                    app.task_detail_dependence_label = None;
                    app.task_detail_dependence_id = None;
                    app.task_detail_error = Some(e.clone());
                    app.set_status(format!("Error cargando detalle: {}", e));
                }
            }
        }
        JobOutcome::TaskDetailSaved {
            id,
            card_id,
            list_id,
            board_number,
            result,
            dependence_label,
            dependence_id,
            cards,
        } => {
            if app.requests.task_detail != id {
                app.end_busy();
                return;
            }
            app.end_busy();
            match result {
                Ok(detail) => {
                    let detail = enrich_detail_responsible(app, card_id, detail);
                    if app.show_task_detail && app.task_detail_card_id == Some(card_id) {
                        app.task_detail = Some(detail);
                        app.task_detail_dependence_label = dependence_label;
                        app.task_detail_dependence_id = dependence_id;
                        app.task_detail_error = None;
                    }
                    if board_number == app.board_number {
                        if let (Some(lid), Some(Ok(list_cards))) = (list_id, cards) {
                            app.cards_by_list.insert(lid, list_cards);
                            app.clamp_card_selection();
                        }
                    }
                    app.editing_detail_field = false;
                    app.detail_edit_input.clear();
                    app.set_status("Detalle actualizado.");
                }
                Err(e) => app.set_status(format!("Error guardando: {}", e)),
            }
        }
        JobOutcome::BoardsLoaded {
            id,
            current_board,
            result,
        } => {
            if app.requests.boards_popup != id || !app.show_board_switch {
                app.end_busy();
                return;
            }
            app.end_busy();
            match result {
                Ok(boards) => {
                    if let Some(idx) = boards.iter().position(|b| b.id == current_board) {
                        app.selected_board_idx = idx;
                    }
                    app.available_boards = boards;
                    app.clear_status();
                }
                Err(e) => app.set_status(format!("Error cargando boards: {}", e)),
            }
        }
    }
}

/// Shared startup path (before the alternate screen): still blocking so the
/// first frame already has data. Runtime refreshes go through [`Jobs`].
pub async fn refresh_all_blocking(client: &HteamClient, app: &mut App) {
    app.begin_busy("Cargando...");
    let id = app.requests.bump_board_refresh();
    let board_number = app.board_number;
    let (lists, cards, working_on, user_shift) = fetch_board_snapshot(client, board_number).await;
    apply_outcome(
        app,
        JobOutcome::BoardRefresh {
            id,
            board_number,
            lists,
            cards,
            working_on,
            user_shift,
        },
    );
}

pub fn request_board_refresh(client: Arc<HteamClient>, app: &mut App, jobs: &Jobs) {
    let id = app.requests.bump_board_refresh();
    let board_number = app.board_number;
    app.begin_busy("Cargando...");
    jobs.spawn(async move {
        let (lists, cards, working_on, user_shift) =
            fetch_board_snapshot(&client, board_number).await;
        JobOutcome::BoardRefresh {
            id,
            board_number,
            lists,
            cards,
            working_on,
            user_shift,
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WorkingHoursConfig;
    use crate::models::List;
    use crate::tui::theme::Theme;
    use std::collections::HashSet;

    fn test_app(board: u64) -> App {
        App::new(
            board,
            HashSet::new(),
            Vec::new(),
            WorkingHoursConfig::default(),
            Theme::default(),
        )
    }

    fn sample_list(id: u64, name: &str) -> List {
        List {
            id,
            name: name.to_string(),
            card_count: Some(0),
            color: None,
            position: None,
            show_url: None,
        }
    }

    #[test]
    fn stale_board_refresh_is_ignored() {
        let mut app = test_app(1);
        let current_id = app.requests.bump_board_refresh();
        app.begin_busy("Cargando...");

        apply_outcome(
            &mut app,
            JobOutcome::BoardRefresh {
                id: current_id.wrapping_sub(1),
                board_number: 1,
                lists: Ok(vec![sample_list(10, "Todo")]),
                cards: HashMap::new(),
                working_on: None,
                user_shift: None,
            },
        );

        assert!(app.all_lists.is_empty());
        assert!(app.busy_message.is_none());
    }

    #[test]
    fn board_refresh_for_other_board_is_ignored() {
        let mut app = test_app(2);
        let id = app.requests.bump_board_refresh();
        app.begin_busy("Cargando...");

        apply_outcome(
            &mut app,
            JobOutcome::BoardRefresh {
                id,
                board_number: 1,
                lists: Ok(vec![sample_list(10, "Todo")]),
                cards: HashMap::new(),
                working_on: None,
                user_shift: None,
            },
        );

        assert!(app.all_lists.is_empty());
    }

    #[test]
    fn current_board_refresh_applies_lists() {
        let mut app = test_app(1);
        let id = app.requests.bump_board_refresh();
        app.begin_busy("Cargando...");

        apply_outcome(
            &mut app,
            JobOutcome::BoardRefresh {
                id,
                board_number: 1,
                lists: Ok(vec![sample_list(10, "Todo")]),
                cards: HashMap::new(),
                working_on: Some(Vec::new()),
                user_shift: Some(None),
            },
        );

        assert_eq!(app.all_lists.len(), 1);
        assert_eq!(app.all_lists[0].name, "Todo");
        assert!(app.busy_message.is_none());
        assert!(app.status.is_none());
    }

    fn sample_card(id: u64, name: &str) -> Card {
        Card {
            id,
            name: name.to_string(),
            description: None,
            status: None,
            labels: Vec::new(),
            number: None,
            title_url: None,
            subtitle_url: None,
            responsible: None,
            priority: None,
            worker: None,
            time_status: None,
        }
    }

    #[test]
    fn board_refresh_keeps_prior_cards_when_a_list_fails() {
        let mut app = test_app(1);
        app.all_lists = vec![sample_list(10, "Todo"), sample_list(20, "Doing")];
        app.cards_by_list
            .insert(10, vec![sample_card(1, "old-todo")]);
        app.cards_by_list
            .insert(20, vec![sample_card(2, "old-doing")]);

        let id = app.requests.bump_board_refresh();
        let mut cards = HashMap::new();
        cards.insert(10, Ok(vec![sample_card(3, "new-todo")]));
        cards.insert(20, Err("Error cargando 'Doing': timeout".into()));

        apply_outcome(
            &mut app,
            JobOutcome::BoardRefresh {
                id,
                board_number: 1,
                lists: Ok(vec![sample_list(10, "Todo"), sample_list(20, "Doing")]),
                cards,
                working_on: None,
                user_shift: None,
            },
        );

        assert_eq!(app.cards_by_list[&10][0].name, "new-todo");
        assert_eq!(
            app.cards_by_list[&20][0].name, "old-doing",
            "failed list must keep previous cards"
        );
        assert_eq!(
            app.status.as_deref(),
            Some("Error cargando 'Doing': timeout")
        );
    }

    #[test]
    fn board_refresh_drops_cards_for_removed_lists() {
        let mut app = test_app(1);
        app.all_lists = vec![sample_list(10, "Todo"), sample_list(99, "Gone")];
        app.cards_by_list.insert(10, vec![sample_card(1, "keep")]);
        app.cards_by_list.insert(99, vec![sample_card(2, "drop")]);

        let id = app.requests.bump_board_refresh();
        let mut cards = HashMap::new();
        cards.insert(10, Ok(vec![sample_card(1, "keep")]));

        apply_outcome(
            &mut app,
            JobOutcome::BoardRefresh {
                id,
                board_number: 1,
                lists: Ok(vec![sample_list(10, "Todo")]),
                cards,
                working_on: None,
                user_shift: None,
            },
        );

        assert!(app.cards_by_list.contains_key(&10));
        assert!(
            !app.cards_by_list.contains_key(&99),
            "cards for lists no longer on the board must go"
        );
        assert!(app.status.is_none());
    }

    #[test]
    fn board_refresh_joins_multiple_list_errors_in_status() {
        let mut app = test_app(1);
        let id = app.requests.bump_board_refresh();
        let mut cards = HashMap::new();
        cards.insert(10, Err("Error cargando 'A': 500".into()));
        cards.insert(20, Err("Error cargando 'B': 503".into()));

        apply_outcome(
            &mut app,
            JobOutcome::BoardRefresh {
                id,
                board_number: 1,
                lists: Ok(vec![sample_list(10, "A"), sample_list(20, "B")]),
                cards,
                working_on: None,
                user_shift: None,
            },
        );

        let status = app.status.expect("status");
        assert!(status.contains("Error cargando 'A': 500"));
        assert!(status.contains("Error cargando 'B': 503"));
        assert!(status.contains('·'));
    }

    #[test]
    fn board_refresh_list_error_keeps_existing_board() {
        let mut app = test_app(1);
        app.all_lists = vec![sample_list(10, "Todo")];
        app.cards_by_list
            .insert(10, vec![sample_card(1, "still-here")]);

        let id = app.requests.bump_board_refresh();
        apply_outcome(
            &mut app,
            JobOutcome::BoardRefresh {
                id,
                board_number: 1,
                lists: Err("Error cargando listas: boom".into()),
                cards: HashMap::new(),
                working_on: None,
                user_shift: None,
            },
        );

        assert_eq!(app.all_lists.len(), 1);
        assert_eq!(app.cards_by_list[&10][0].name, "still-here");
        assert_eq!(app.status.as_deref(), Some("Error cargando listas: boom"));
    }

    #[test]
    fn stale_comments_do_not_overwrite_open_popup() {
        let mut app = test_app(1);
        app.show_comments = true;
        let id = app.requests.bump_comments();
        app.begin_busy("Cargando comentarios...");
        let _newer = app.requests.bump_comments();
        app.begin_busy("Cargando comentarios...");

        apply_outcome(
            &mut app,
            JobOutcome::Comments {
                id,
                card_id: 99,
                comments: Ok(vec![Comment {
                    id: 1,
                    user_name: "u".into(),
                    submit_date: String::new(),
                    comment: "viejo".into(),
                    level: 0,
                    parent_id: None,
                    is_removed: false,
                    permalink: None,
                }]),
                follow_up: Ok(None),
            },
        );

        assert!(app.comments.is_empty(), "stale payload must not apply");
        assert_eq!(app.busy_depth, 1, "only the stale job ends busy");
        assert_eq!(app.busy_message.as_deref(), Some("Cargando comentarios..."));
    }

    #[test]
    fn footer_prefers_busy_over_status() {
        let mut app = test_app(1);
        app.set_status("listo");
        app.begin_busy("Cargando...");
        assert_eq!(app.footer_message().as_deref(), Some("Cargando..."));
        app.end_busy();
        assert_eq!(app.footer_message().as_deref(), Some("listo"));
    }

    #[test]
    fn mentions_with_stale_query_are_ignored() {
        use crate::models::UserSuggestion;

        let mut app = test_app(1);
        app.composing_comment = true;
        app.comment_input.set("@ab");
        let id = app.requests.bump_mentions();

        apply_outcome(
            &mut app,
            JobOutcome::Mentions {
                id,
                query: "a".into(), // no longer matches "@ab"
                result: Ok(vec![UserSuggestion {
                    id: "1".into(),
                    text: "ana".into(),
                    selected_text: "ana".into(),
                }]),
            },
        );

        assert!(app.mention_suggestions.is_empty());
    }

    #[test]
    fn mentions_with_matching_query_apply() {
        use crate::models::UserSuggestion;

        let mut app = test_app(1);
        app.composing_comment = true;
        app.comment_input.set("hola @an");
        let id = app.requests.bump_mentions();

        apply_outcome(
            &mut app,
            JobOutcome::Mentions {
                id,
                query: "an".into(),
                result: Ok(vec![UserSuggestion {
                    id: "1".into(),
                    text: "ana".into(),
                    selected_text: "ana".into(),
                }]),
            },
        );

        assert_eq!(app.mention_suggestions.len(), 1);
        assert_eq!(app.mention_suggestions[0].selected_text, "ana");
    }

    fn test_client(server_url: &str) -> HteamClient {
        use crate::config::{AuthConfig, Config};

        let config = Config {
            auth: AuthConfig {
                board_number: Some(483),
                csrf_token: Some("csrf".into()),
                session_id: Some("sid".into()),
                ..AuthConfig::default()
            },
            ..Config::default()
        };
        HteamClient::new_for_test(config, server_url.to_string(), server_url.to_string())
            .expect("test client")
    }

    fn empty_card_list_body(list_id: u64) -> String {
        format!(r#"{{"id":{list_id},"name":"L","cardlist_list":[],"total_board_cards":0}}"#)
    }

    #[tokio::test]
    async fn fetch_board_snapshot_loads_each_list_and_keeps_partial_errors() {
        let mut server = mockito::Server::new_async().await;
        let _lists = server
            .mock("GET", "/operation/care/operations/483/lists/")
            .match_query(mockito::Matcher::UrlEncoded(
                "format".into(),
                "json".into(),
            ))
            .with_status(200)
            .with_body(
                r#"[{"id":10,"name":"Todo","total_board_cards":0},{"id":20,"name":"Doing","total_board_cards":0}]"#,
            )
            .create_async()
            .await;
        let _todo_cards = server
            .mock("GET", "/operation/care/operations/483/lists/10/cards/")
            .match_query(mockito::Matcher::UrlEncoded("format".into(), "json".into()))
            .with_status(200)
            .with_body(empty_card_list_body(10))
            .create_async()
            .await;
        let _doing_cards = server
            .mock("GET", "/operation/care/operations/483/lists/20/cards/")
            .match_query(mockito::Matcher::UrlEncoded("format".into(), "json".into()))
            .with_status(500)
            .with_body("boom")
            .create_async()
            .await;

        let client = test_client(&server.url());
        let (lists, cards, _, _) = fetch_board_snapshot(&client, 483).await;

        let lists = lists.expect("lists");
        assert_eq!(lists.len(), 2);
        assert!(cards.get(&10).unwrap().is_ok());
        assert!(
            cards
                .get(&20)
                .unwrap()
                .as_ref()
                .unwrap_err()
                .contains("Doing"),
            "failed list should be named in the error"
        );
    }
}
