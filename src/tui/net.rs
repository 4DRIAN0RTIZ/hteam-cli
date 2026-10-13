use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

use tokio::sync::mpsc;

use crate::client::HteamClient;
use crate::models::{
    BoardEntry, Card, Comment, FollowUp, List, ProjectMilestone, ProjectSummary, ProjectTask,
    Reminder, UserSuggestion, WeeklyObjectivesSet, WorkShiftRecord, WorkingOnStatus,
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
pub async fn fetch_board_snapshot(
    client: &HteamClient,
    board_number: u64,
) -> (
    Result<Vec<List>, String>,
    HashMap<u64, Result<Vec<Card>, String>>,
    Option<Vec<WorkingOnStatus>>,
    Option<Option<WorkShiftRecord>>,
) {
    let user_shift = match operations::checkin::workshift_resume(client).await {
        Ok(resume) => Some(resume.last),
        Err(_) => None,
    };

    let lists = match operations::cards::list_lists(client, Some(board_number)).await {
        Ok(lists) => lists,
        Err(e) => {
            return (
                Err(format!("Error cargando listas: {}", e)),
                HashMap::new(),
                None,
                user_shift,
            );
        }
    };

    let mut cards = HashMap::new();
    for list in &lists {
        let entry = match operations::cards::list_cards(client, list.id, Some(board_number)).await {
            Ok((list_cards, _)) => Ok(list_cards),
            Err(e) => Err(format!("Error cargando '{}': {}", list.name, e)),
        };
        cards.insert(list.id, entry);
    }

    let working_on = operations::working::list_working(client).await.ok();

    (Ok(lists), cards, working_on, user_shift)
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
                    app.all_lists = lists;
                    app.cards_by_list.clear();
                    let mut error_msg = None;
                    for (list_id, entry) in cards {
                        match entry {
                            Ok(list_cards) => {
                                app.cards_by_list.insert(list_id, list_cards);
                            }
                            Err(msg) => error_msg = Some(msg),
                        }
                    }
                    if let Some(items) = working_on {
                        app.working_on = items;
                    }
                    let visible_len = app.visible_lists().len();
                    app.selected_list = if visible_len > 0 {
                        app.selected_list.min(visible_len - 1)
                    } else {
                        0
                    };
                    match error_msg {
                        Some(msg) => app.set_status(msg),
                        None if app.all_lists.is_empty() => {
                            app.set_status("Este board no tiene listas.")
                        }
                        None => app.clear_status(),
                    }
                }
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
                Ok(list) => app.reminders = list,
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
        JobOutcome::Mentions { id, result } => {
            if app.requests.mentions != id || !app.composing_comment {
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
}
