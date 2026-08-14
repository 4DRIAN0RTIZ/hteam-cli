use std::collections::{HashMap, HashSet};
use std::fs::OpenOptions;
use std::io::Write;
use std::time::{Duration, Instant};

use crate::config::WorkingHoursConfig;
use crate::models::{
    Card, Comment, List, ProjectMilestone, ProjectTask, Reminder, UserSuggestion, WorkShiftRecord,
    WorkingOnStatus,
};

pub struct App {
    pub board_number: u64,
    /// Último registro de turno del usuario actual (check_in/check_out),
    /// mostrado en el header — `None` mientras no se ha podido cargar.
    pub user_shift: Option<WorkShiftRecord>,
    /// Every list returned by the API, unfiltered — cards are fetched for all
    /// of them regardless of visibility so toggling a hidden list back on
    /// doesn't require another round trip.
    pub all_lists: Vec<List>,
    /// Lowercased list names hidden from view/navigation (persisted to
    /// config.toml's `[tui] hidden_lists`).
    pub hidden_lists: HashSet<String>,
    pub cards_by_list: HashMap<u64, Vec<Card>>,
    /// The current user's active "working on it" records — keeps the record
    /// id around (not just the card id) since `stop_working` needs it, not
    /// the card id.
    pub working_on: Vec<WorkingOnStatus>,
    pub selected_list: usize,
    pub selected_card: HashMap<u64, usize>,
    pub status: Option<String>,
    status_set_at: Option<Instant>,
    /// Pending reminders, loaded when the 'R' popup is opened.
    pub reminders: Vec<Reminder>,
    pub show_reminders: bool,
    /// True while the description popup ('Enter') is open, editing the
    /// currently selected card's description inline.
    pub show_description: bool,
    pub description_input: String,
    /// Comments of the card that was selected when 'C' was pressed.
    pub comments: Vec<Comment>,
    pub show_comments: bool,
    /// True while actively typing a new comment inside the comments popup.
    pub composing_comment: bool,
    pub comment_input: String,
    pub mention_suggestions: Vec<UserSuggestion>,
    pub mention_selected: usize,
    /// Project ids looked up before, most-recently-used first — mirrors
    /// `config.toml`'s `[tui] known_projects`.
    pub known_projects: Vec<u64>,
    pub selected_project_idx: usize,
    pub active_project: Option<u64>,
    pub project_milestones: Vec<ProjectMilestone>,
    pub project_tasks: Vec<ProjectTask>,
    pub show_projects: bool,
    /// True while typing a new project id inside the projects popup.
    pub composing_project: bool,
    pub project_input: String,
    /// True while typing a new card's name for `new_card_list_id`.
    pub composing_card: bool,
    pub new_card_input: String,
    pub new_card_list_id: Option<u64>,
    pub show_help: bool,
    /// Scroll offset (rows) para el popup de ayuda.
    pub help_scroll: u16,
    /// Scroll offset (rows) para el popup de reminders.
    pub reminders_scroll: u16,
    /// Scroll offset (rows) para la lista de comentarios (solo en modo lectura, no al componer).
    pub comments_scroll: u16,
    /// True cuando el foco del popup de proyectos está en el panel de detalle (derecha).
    /// Tab alterna entre la lista de proyectos guardados y el panel de detalle.
    pub projects_detail_focused: bool,
    /// Scroll offset (rows) para el panel de detalle del popup de proyectos.
    pub projects_scroll: u16,
    /// Horario laboral configurado en `config.toml`'s `[working_hours]`.
    pub working_hours: WorkingHoursConfig,
}

impl App {
    pub fn new(
        board_number: u64,
        hidden_lists: HashSet<String>,
        known_projects: Vec<u64>,
        working_hours: WorkingHoursConfig,
    ) -> Self {
        Self {
            board_number,
            user_shift: None,
            all_lists: Vec::new(),
            hidden_lists,
            cards_by_list: HashMap::new(),
            working_on: Vec::new(),
            selected_list: 0,
            selected_card: HashMap::new(),
            status: None,
            status_set_at: None,
            reminders: Vec::new(),
            show_reminders: false,
            show_description: false,
            description_input: String::new(),
            comments: Vec::new(),
            show_comments: false,
            composing_comment: false,
            comment_input: String::new(),
            mention_suggestions: Vec::new(),
            mention_selected: 0,
            known_projects,
            selected_project_idx: 0,
            active_project: None,
            project_milestones: Vec::new(),
            project_tasks: Vec::new(),
            show_projects: false,
            composing_project: false,
            project_input: String::new(),
            composing_card: false,
            new_card_input: String::new(),
            new_card_list_id: None,
            show_help: false,
            help_scroll: 0,
            reminders_scroll: 0,
            comments_scroll: 0,
            projects_detail_focused: false,
            projects_scroll: 0,
            working_hours,
        }
    }

    /// Tiempo restante de la jornada, evaluado contra la hora local actual.
    pub fn working_hours_remaining(&self) -> Option<String> {
        self.working_hours.remaining_display(chrono::Local::now().time())
    }

    /// Sets a footer status message, starts its expiry timer, and appends it
    /// to `~/.config/hteam/tui.log` — the footer clears itself after
    /// `STATUS_TTL`, so this is the only place the message survives to be
    /// inspected/reported after the fact.
    pub fn set_status(&mut self, msg: impl Into<String>) {
        let msg = msg.into();
        log_line(&msg);
        self.status = Some(msg);
        self.status_set_at = Some(Instant::now());
    }

    pub fn clear_status(&mut self) {
        self.status = None;
        self.status_set_at = None;
    }

    /// Clears the status message once it's older than `ttl`, so the footer
    /// falls back to the keybindings hint instead of sticking forever.
    pub fn expire_status(&mut self, ttl: Duration) {
        if let Some(set_at) = self.status_set_at {
            if set_at.elapsed() >= ttl {
                self.clear_status();
            }
        }
    }

    pub fn visible_lists(&self) -> Vec<&List> {
        self.all_lists
            .iter()
            .filter(|l| !self.hidden_lists.contains(&l.name.to_lowercase()))
            .collect()
    }

    pub fn current_list(&self) -> Option<&List> {
        self.visible_lists().get(self.selected_list).copied()
    }

    pub fn current_card_index(&self) -> usize {
        match self.current_list() {
            Some(list) => *self.selected_card.get(&list.id).unwrap_or(&0),
            None => 0,
        }
    }

    pub fn current_card(&self) -> Option<&Card> {
        let list = self.current_list()?;
        self.cards_by_list.get(&list.id)?.get(self.current_card_index())
    }

    pub fn is_working_on(&self, card_id: u64) -> bool {
        self.working_on.iter().any(|w| w.card_id == card_id)
    }

    /// The "working on it" record id for `card_id`, if any — this is what
    /// `stop_working` needs, not the card id itself.
    pub fn working_on_id_for(&self, card_id: u64) -> Option<u64> {
        self.working_on.iter().find(|w| w.card_id == card_id).map(|w| w.id)
    }

    pub fn move_selection(&mut self, delta: i32) {
        let Some(list) = self.current_list() else { return };
        let list_id = list.id;
        let len = self.cards_by_list.get(&list_id).map(|c| c.len()).unwrap_or(0);
        if len == 0 {
            return;
        }
        let current = self.current_card_index() as i32;
        let next = (current + delta).clamp(0, len as i32 - 1) as usize;
        self.selected_card.insert(list_id, next);
    }

    pub fn move_list(&mut self, delta: i32) {
        let len = self.visible_lists().len();
        if len == 0 {
            return;
        }
        let next = (self.selected_list as i32 + delta).clamp(0, len as i32 - 1);
        self.selected_list = next as usize;
    }
}

/// Best-effort append to the TUI's debug log — never fails the caller, since
/// this is only a debugging aid, not something the app depends on.
fn log_line(msg: &str) {
    let Ok(dir) = crate::config::Config::config_dir() else {
        return;
    };
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(dir.join("tui.log")) else {
        return;
    };
    let _ = writeln!(
        file,
        "[{}] {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        msg
    );
}
