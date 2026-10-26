use std::collections::{HashMap, HashSet};
use std::fs::OpenOptions;
use std::io::Write;
use std::time::{Duration, Instant};

use crate::config::WorkingHoursConfig;
use crate::models::{
    BoardEntry, Card, CardDetail, Comment, FollowUp, Label, List, ProjectMilestone, ProjectSummary,
    ProjectTask, Reminder, UserSuggestion, WeeklyObjectivesSet, WorkShiftRecord, WorkingOnStatus,
};
use crate::operations::cards::CardSearchFilters;

use super::theme::Theme;
use super::widgets::{Scroll, TextInput};

/// Current vs latest version for the startup "Update Available" popup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateAvailableInfo {
    pub current_version: String,
    pub latest_version: String,
}

/// Qué sección muestra el popup de ayuda ('?') — cada popup con muchos
/// keybindings propios (que ya no entran en una sola línea de título sin
/// truncarse) tiene el suyo, en vez de forzar al usuario a buscarlo dentro
/// del listado completo. `Board` es el listado completo de siempre, mostrado
/// cuando '?' se presiona sin ningún popup abierto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HelpContext {
    #[default]
    Board,
    Reminders,
    WeeklyObjectives,
    Comments,
    Projects,
    BoardSwitch,
    TaskDetail,
}

/// Which list is active inside the projects detail panel (right side).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProjectDetailSection {
    #[default]
    Milestones,
    Tasks,
}

/// Editable rows in the task-detail popup (`d` / `e`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailField {
    Name,
    Description,
    Responsible,
    Priority,
    Dependence,
    StartDate,
    DueDate,
    TimeEstimated,
    TimeSpent,
}

impl DetailField {
    pub const ALL: &'static [DetailField] = &[
        DetailField::Name,
        DetailField::Description,
        DetailField::Responsible,
        DetailField::Priority,
        DetailField::Dependence,
        DetailField::StartDate,
        DetailField::DueDate,
        DetailField::TimeEstimated,
        DetailField::TimeSpent,
    ];

    pub fn label(self) -> &'static str {
        match self {
            DetailField::Name => "Nombre",
            DetailField::Description => "Descripción",
            DetailField::Responsible => "Responsable",
            DetailField::Priority => "Prioridad",
            DetailField::Dependence => "Depende de",
            DetailField::StartDate => "Inicio",
            DetailField::DueDate => "Vencimiento",
            DetailField::TimeEstimated => "Estimado",
            DetailField::TimeSpent => "Gastado",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            DetailField::Name => "texto",
            DetailField::Description => "texto (Ctrl+S guarda)",
            DetailField::Responsible => "username",
            DetailField::Priority => "1 muy baja … 5 muy alta",
            DetailField::Dependence => "j/k elegir · Enter confirmar · / filtrar · Esc",

            DetailField::StartDate | DetailField::DueDate => "YYYY-MM-DD",
            DetailField::TimeEstimated | DetailField::TimeSpent => "ej. 1h · 45m · +30m",
        }
    }
}

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
    /// Active board card filters (`/` search). Empty = show all cards.
    pub card_filters: CardSearchFilters,
    /// True while typing a `/` search query in the footer bar.
    pub composing_search: bool,
    pub search_input: TextInput,
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
    pub selected_reminder: Option<usize>,
    pub show_reminders: bool,
    /// True while the description popup ('Enter') is open, editing the
    /// currently selected card's description inline.
    pub show_description: bool,
    pub description_input: TextInput,
    /// Task detail popup (`d`) — full card sheet with field edit (`e`).
    pub show_task_detail: bool,
    pub task_detail: Option<CardDetail>,
    pub task_detail_card_id: Option<u64>,
    pub task_detail_list_id: Option<u64>,
    pub task_detail_error: Option<String>,
    /// Human label for depends-on (`#n name`) when resolved.
    pub task_detail_dependence_label: Option<String>,
    /// Internal task id for depends-on (jump), when known.
    pub task_detail_dependence_id: Option<u64>,
    pub task_detail_field_idx: usize,
    /// Picker of board tasks while editing Depende de.
    pub picking_dependence: bool,
    pub dependence_picker_idx: usize,
    pub dependence_picker_filter: TextInput,
    /// Cached board tasks for the picker: (id, number, name).
    pub dependence_picker_items: Vec<(u64, Option<u64>, String)>,
    /// Multi-select label picker inside the task-detail popup.
    pub picking_labels: bool,
    pub label_picker_loading: bool,
    pub label_catalog_loaded: bool,
    pub label_picker_idx: usize,
    pub label_picker_filter: TextInput,
    pub label_picker_items: Vec<Label>,
    pub label_picker_selected: HashSet<u64>,
    pub task_detail_scroll: Scroll,
    /// Editing one field inside the detail popup.
    pub editing_detail_field: bool,
    pub detail_edit_input: TextInput,
    /// Comments of the card that was selected when 'C' was pressed.
    pub comments: Vec<Comment>,
    pub comments_selected: Option<usize>,
    pub comment_reply_to: Option<u64>,
    pub show_comments: bool,
    /// Seguimiento agendado de la card que está detrás del popup de
    /// comentarios (`None` si no tiene uno agendado). Se scrapea aparte
    /// porque no viene en la API JSON de comentarios — ver
    /// `HteamClient::get_card_follow_up`.
    pub current_follow_up: Option<FollowUp>,
    /// True while actively typing a new comment inside the comments popup.
    pub composing_comment: bool,
    pub comment_input: TextInput,
    /// True cuando el comentario en curso se marcará para seguimiento (Ctrl+F togglea).
    pub comment_follow: bool,
    /// Fecha del comentario en curso; vacío = "ahora" en hora local. Tab alterna
    /// el foco entre este campo y `comment_input`.
    pub comment_date_input: TextInput,
    pub comment_date_focused: bool,
    pub mention_suggestions: Vec<UserSuggestion>,
    pub mention_selected: usize,
    /// When set, a mention search will fire after this instant (debounce).
    pub mention_debounce_at: Option<Instant>,
    /// Project ids looked up before, most-recently-used first — mirrors
    /// `config.toml`'s `[tui] known_projects`.
    pub known_projects: Vec<u64>,
    pub selected_project_idx: usize,
    pub active_project: Option<u64>,
    pub project_milestones: Vec<ProjectMilestone>,
    /// Highlighted milestone inside the projects detail panel (create/edit).
    pub selected_milestone_idx: usize,
    pub project_tasks: Vec<ProjectTask>,
    /// Highlighted task inside the projects detail panel.
    pub selected_task_idx: usize,
    /// Milestones vs tasks list when the detail panel has focus.
    pub projects_detail_section: ProjectDetailSection,
    pub show_projects: bool,
    /// Proyectos en vivo (`hteam project list`, status "Execution"),
    /// cargados al presionar 'L' dentro del popup de proyectos — distinto de
    /// `known_projects`, que son solo IDs recordados localmente.
    pub live_projects: Vec<ProjectSummary>,
    /// True mientras el panel izquierdo del popup de proyectos muestra
    /// `live_projects` en vez de `known_projects`.
    pub show_live_projects: bool,
    pub selected_live_project_idx: usize,
    /// Índice en `operations::projects::PROJECT_STATUSES` del filtro activo
    /// del listado en vivo — 'h'/'l' lo cicla. Se conserva entre aperturas
    /// del popup en la misma sesión del TUI.
    pub live_projects_status_idx: usize,
    /// Weekly objectives loaded from SharePad/cache for the read-only popup.
    pub weekly_objectives: Option<WeeklyObjectivesSet>,
    pub weekly_objectives_from_cache: bool,
    pub weekly_objectives_synced_at: Option<chrono::DateTime<chrono::Utc>>,
    pub weekly_objectives_error: Option<String>,
    pub show_weekly_objectives: bool,
    /// Changelog text shown once after a binary update (startup popup).
    pub startup_changelog: Option<String>,
    pub show_changelog: bool,
    /// Newer GitHub release noticed at startup (popup, not only footer).
    pub update_available: Option<UpdateAvailableInfo>,
    pub show_update_available: bool,
    /// True while typing a new project id inside the projects popup.
    pub composing_project: bool,
    pub project_input: TextInput,
    /// True while creating/editing a project milestone (name + description).
    pub composing_milestone: bool,
    /// `None` = create; `Some(id)` = edit that milestone.
    pub milestone_form_edit_id: Option<u64>,
    pub milestone_name_input: TextInput,
    pub milestone_desc_input: TextInput,
    /// `false` = name field focused; `true` = description.
    pub milestone_desc_focused: bool,
    /// Create/edit a project task (under the selected milestone, or edit fields).
    pub composing_project_task: bool,
    /// `None` = create under selected milestone; `Some(id)` = edit that task.
    pub project_task_form_edit_id: Option<u64>,
    pub project_task_name_input: TextInput,
    pub project_task_desc_input: TextInput,
    pub project_task_milestone_input: TextInput,
    pub project_task_dependence_input: TextInput,
    pub project_task_start_date_input: TextInput,
    /// Field index in the active project-task form (create or edit).
    pub project_task_form_field: usize,
    /// True while typing a new card's name for `new_card_list_id`.
    pub composing_card: bool,
    pub new_card_input: TextInput,
    pub new_card_list_id: Option<u64>,
    pub show_help: bool,
    /// Qué sección mostrar en el popup de ayuda — ver `HelpContext`.
    pub help_context: HelpContext,
    /// True mientras el popup de cambio de board ('B') está abierto.
    pub show_board_switch: bool,
    /// Boards obtenidos de la API al abrir el popup.
    pub available_boards: Vec<BoardEntry>,
    /// Índice del board resaltado en `available_boards`.
    pub selected_board_idx: usize,
    /// Scroll offset para el popup de ayuda.
    pub help_scroll: Scroll,
    /// Scroll offset para el popup de reminders.
    pub reminders_scroll: Scroll,
    /// Scroll offset para el popup de objetivos semanales.
    pub weekly_objectives_scroll: Scroll,
    /// Scroll offset for the post-update changelog popup.
    pub changelog_scroll: Scroll,
    /// Scroll offset para la lista de comentarios (solo en modo lectura, no al componer).
    pub comments_scroll: Scroll,
    /// True cuando el foco del popup de proyectos está en el panel de detalle (derecha).
    /// Tab alterna entre la lista de proyectos guardados y el panel de detalle.
    pub projects_detail_focused: bool,
    /// Scroll offset para el panel de detalle del popup de proyectos.
    pub projects_scroll: Scroll,
    /// Horario laboral configurado en `config.toml`'s `[working_hours]`.
    pub working_hours: WorkingHoursConfig,
    /// Tema de color activo, resuelto desde `config.toml`'s `[theme] active`
    /// al iniciar el TUI. Todo el renderizado en `ui.rs` resuelve sus colores
    /// desde acá en vez de literales `Color::` sueltos.
    pub theme: Theme,
    /// Monotonic ids so a late HTTP response cannot clobber a newer board or
    /// popup request (see `tui::net`).
    pub requests: RequestIds,
    /// Non-expiring loading label, nested by concurrent background jobs.
    pub busy_message: Option<String>,
    pub busy_depth: u32,
}

/// Per-surface request tokens. Each bump invalidates in-flight responses for
/// that surface when `net::apply_outcome` checks the id.
#[derive(Debug, Default, Clone)]
pub struct RequestIds {
    pub board_refresh: u64,
    pub comments: u64,
    pub mentions: u64,
    pub reminders: u64,
    pub objectives: u64,
    pub projects: u64,
    pub project_milestone_write: u64,
    pub project_milestone_form: u64,
    pub project_task_write: u64,
    pub project_task_form: u64,
    pub live_projects: u64,
    pub boards_popup: u64,
    pub move_card: u64,
    pub working: u64,
    pub description: u64,
    pub new_card: u64,
    pub comment_post: u64,
    pub follow_up: u64,
    pub card_link: u64,
    pub task_detail: u64,
    pub label_picker: u64,
}

impl RequestIds {
    fn bump(slot: &mut u64) -> u64 {
        *slot = slot.wrapping_add(1);
        *slot
    }

    pub fn bump_board_refresh(&mut self) -> u64 {
        Self::bump(&mut self.board_refresh)
    }
    pub fn bump_comments(&mut self) -> u64 {
        Self::bump(&mut self.comments)
    }
    pub fn bump_mentions(&mut self) -> u64 {
        Self::bump(&mut self.mentions)
    }
    pub fn bump_reminders(&mut self) -> u64 {
        Self::bump(&mut self.reminders)
    }
    pub fn bump_objectives(&mut self) -> u64 {
        Self::bump(&mut self.objectives)
    }
    pub fn bump_projects(&mut self) -> u64 {
        Self::bump(&mut self.projects)
    }
    pub fn bump_project_milestone_write(&mut self) -> u64 {
        Self::bump(&mut self.project_milestone_write)
    }
    pub fn bump_project_milestone_form(&mut self) -> u64 {
        Self::bump(&mut self.project_milestone_form)
    }
    pub fn bump_project_task_write(&mut self) -> u64 {
        Self::bump(&mut self.project_task_write)
    }
    pub fn bump_project_task_form(&mut self) -> u64 {
        Self::bump(&mut self.project_task_form)
    }
    pub fn bump_live_projects(&mut self) -> u64 {
        Self::bump(&mut self.live_projects)
    }
    pub fn bump_boards_popup(&mut self) -> u64 {
        Self::bump(&mut self.boards_popup)
    }
    pub fn bump_move_card(&mut self) -> u64 {
        Self::bump(&mut self.move_card)
    }
    pub fn bump_working(&mut self) -> u64 {
        Self::bump(&mut self.working)
    }
    pub fn bump_description(&mut self) -> u64 {
        Self::bump(&mut self.description)
    }
    pub fn bump_new_card(&mut self) -> u64 {
        Self::bump(&mut self.new_card)
    }
    pub fn bump_comment_post(&mut self) -> u64 {
        Self::bump(&mut self.comment_post)
    }
    pub fn bump_follow_up(&mut self) -> u64 {
        Self::bump(&mut self.follow_up)
    }
    pub fn bump_card_link(&mut self) -> u64 {
        Self::bump(&mut self.card_link)
    }
    pub fn bump_task_detail(&mut self) -> u64 {
        Self::bump(&mut self.task_detail)
    }
    pub fn bump_label_picker(&mut self) -> u64 {
        Self::bump(&mut self.label_picker)
    }

    pub fn is_current_board_refresh(&self, id: u64, job_board: u64, app_board: u64) -> bool {
        self.board_refresh == id && job_board == app_board
    }
}

impl App {
    pub fn new(
        board_number: u64,
        hidden_lists: HashSet<String>,
        known_projects: Vec<u64>,
        working_hours: WorkingHoursConfig,
        theme: Theme,
    ) -> Self {
        Self {
            board_number,
            user_shift: None,
            all_lists: Vec::new(),
            hidden_lists,
            cards_by_list: HashMap::new(),
            card_filters: CardSearchFilters::default(),
            composing_search: false,
            search_input: TextInput::default(),
            working_on: Vec::new(),
            selected_list: 0,
            selected_card: HashMap::new(),
            status: None,
            status_set_at: None,
            reminders: Vec::new(),
            selected_reminder: None,
            show_reminders: false,
            show_description: false,
            description_input: TextInput::default(),
            show_task_detail: false,
            task_detail: None,
            task_detail_card_id: None,
            task_detail_list_id: None,
            task_detail_error: None,
            task_detail_dependence_label: None,
            task_detail_dependence_id: None,
            task_detail_field_idx: 0,
            picking_dependence: false,
            dependence_picker_idx: 0,
            dependence_picker_filter: TextInput::default(),
            dependence_picker_items: Vec::new(),
            picking_labels: false,
            label_picker_loading: false,
            label_catalog_loaded: false,
            label_picker_idx: 0,
            label_picker_filter: TextInput::default(),
            label_picker_items: Vec::new(),
            label_picker_selected: HashSet::new(),
            task_detail_scroll: Scroll::default(),
            editing_detail_field: false,
            detail_edit_input: TextInput::default(),
            comments: Vec::new(),
            comments_selected: None,
            comment_reply_to: None,
            show_comments: false,
            current_follow_up: None,
            composing_comment: false,
            comment_input: TextInput::default(),
            comment_follow: false,
            comment_date_input: TextInput::default(),
            comment_date_focused: false,
            mention_suggestions: Vec::new(),
            mention_selected: 0,
            mention_debounce_at: None,
            known_projects,
            selected_project_idx: 0,
            active_project: None,
            project_milestones: Vec::new(),
            selected_milestone_idx: 0,
            project_tasks: Vec::new(),
            selected_task_idx: 0,
            projects_detail_section: ProjectDetailSection::default(),
            show_projects: false,
            live_projects: Vec::new(),
            show_live_projects: false,
            selected_live_project_idx: 0,
            live_projects_status_idx: crate::operations::projects::PROJECT_STATUSES
                .iter()
                .position(|&s| s == "Execution")
                .unwrap_or(0),
            weekly_objectives: None,
            weekly_objectives_from_cache: false,
            weekly_objectives_synced_at: None,
            weekly_objectives_error: None,
            show_weekly_objectives: false,
            startup_changelog: None,
            show_changelog: false,
            update_available: None,
            show_update_available: false,
            composing_project: false,
            project_input: TextInput::default(),
            composing_milestone: false,
            milestone_form_edit_id: None,
            milestone_name_input: TextInput::default(),
            milestone_desc_input: TextInput::default(),
            milestone_desc_focused: false,
            composing_project_task: false,
            project_task_form_edit_id: None,
            project_task_name_input: TextInput::default(),
            project_task_desc_input: TextInput::default(),
            project_task_milestone_input: TextInput::default(),
            project_task_dependence_input: TextInput::default(),
            project_task_start_date_input: TextInput::default(),
            project_task_form_field: 0,
            composing_card: false,
            new_card_input: TextInput::default(),
            new_card_list_id: None,
            show_help: false,
            help_context: HelpContext::default(),
            show_board_switch: false,
            available_boards: Vec::new(),
            selected_board_idx: 0,
            help_scroll: Scroll::default(),
            reminders_scroll: Scroll::default(),
            weekly_objectives_scroll: Scroll::default(),
            changelog_scroll: Scroll::default(),
            comments_scroll: Scroll::default(),
            projects_detail_focused: false,
            projects_scroll: Scroll::default(),
            working_hours,
            theme,
            requests: RequestIds::default(),
            busy_message: None,
            busy_depth: 0,
        }
    }

    pub fn begin_busy(&mut self, msg: impl Into<String>) {
        self.busy_depth = self.busy_depth.saturating_add(1);
        self.busy_message = Some(msg.into());
    }

    pub fn end_busy(&mut self) {
        self.busy_depth = self.busy_depth.saturating_sub(1);
        if self.busy_depth == 0 {
            self.busy_message = None;
        }
    }

    /// Footer text: non-expiring busy label wins over transient status.
    pub fn footer_message(&self) -> Option<String> {
        self.busy_message.clone().or_else(|| self.status.clone())
    }

    /// True while a board-list load is in flight (board-switch empty state).
    pub fn is_loading_boards(&self) -> bool {
        self.busy_message
            .as_deref()
            .map(|s| s.contains("Cargando"))
            .unwrap_or(false)
            || self
                .status
                .as_deref()
                .map(|s| s.contains("Cargando"))
                .unwrap_or(false)
    }

    /// Tiempo restante de la jornada, evaluado contra la hora local actual.
    pub fn working_hours_remaining(&self) -> Option<String> {
        self.working_hours
            .remaining_display(chrono::Local::now().time())
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

    /// Cards of `list_id` after applying [`Self::card_filters`].
    pub fn view_cards(&self, list_id: u64) -> Vec<&Card> {
        let Some(cards) = self.cards_by_list.get(&list_id) else {
            return Vec::new();
        };
        if self.card_filters.is_empty() {
            return cards.iter().collect();
        }
        cards
            .iter()
            .filter(|c| crate::operations::cards::matches_filters(c, &self.card_filters, &[]))
            .collect()
    }

    pub fn current_card(&self) -> Option<&Card> {
        let list = self.current_list()?;
        let cards = self.view_cards(list.id);
        cards.get(self.current_card_index()).copied()
    }

    pub fn is_working_on(&self, card_id: u64) -> bool {
        self.working_on.iter().any(|w| w.card_id == card_id)
    }

    /// The "working on it" record id for `card_id`, if any — this is what
    /// `stop_working` needs, not the card id itself.
    pub fn working_on_id_for(&self, card_id: u64) -> Option<u64> {
        crate::operations::working::working_id_for(&self.working_on, card_id)
    }

    pub fn move_selection(&mut self, delta: i32) {
        let Some(list) = self.current_list() else {
            return;
        };
        let list_id = list.id;
        let len = self.view_cards(list_id).len();
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
        self.clamp_card_selection();
    }

    /// Keep selected card indices inside the filtered view for every list.
    pub fn clamp_card_selection(&mut self) {
        let list_ids: Vec<u64> = self.all_lists.iter().map(|l| l.id).collect();
        for list_id in list_ids {
            let len = self.view_cards(list_id).len();
            if len == 0 {
                self.selected_card.insert(list_id, 0);
                continue;
            }
            let idx = self.selected_card.get(&list_id).copied().unwrap_or(0);
            if idx >= len {
                self.selected_card.insert(list_id, len - 1);
            }
        }
        let visible_len = self.visible_lists().len();
        if visible_len == 0 {
            self.selected_list = 0;
        } else if self.selected_list >= visible_len {
            self.selected_list = visible_len - 1;
        }
    }

    /// Short label for the footer when filters are active.
    pub fn card_filters_summary(&self) -> Option<String> {
        if self.card_filters.is_empty() {
            return None;
        }
        let mut parts = Vec::new();
        if let Some(t) = self
            .card_filters
            .text
            .as_ref()
            .filter(|s| !s.trim().is_empty())
        {
            parts.push(format!("q:{}", t));
        }
        if let Some(r) = self
            .card_filters
            .responsible
            .as_ref()
            .filter(|s| !s.trim().is_empty())
        {
            parts.push(format!("@{}", r));
        }
        if let Some(p) = self
            .card_filters
            .priority
            .as_ref()
            .filter(|s| !s.trim().is_empty())
        {
            parts.push(format!("p:{}", p));
        }
        for lab in &self.card_filters.labels {
            if !lab.trim().is_empty() {
                parts.push(format!("#{}", lab));
            }
        }
        if parts.is_empty() {
            None
        } else {
            Some(parts.join(" "))
        }
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
    let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("tui.log"))
    else {
        return;
    };
    let _ = writeln!(
        file,
        "[{}] {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        msg
    );
}
