use serde::{Deserialize, Serialize};
use tabled::Tabled;

#[derive(Debug, Clone, Serialize, Deserialize, Tabled)]
pub struct List {
    pub id: u64,
    pub name: String,
    #[serde(rename = "total_board_cards")]
    #[tabled(display_with = "display_option", rename = "Cards")]
    pub card_count: Option<usize>,
    #[expect(
        dead_code,
        reason = "kept for API compatibility with Hteam list payloads"
    )]
    #[serde(skip)]
    #[tabled(skip)]
    pub color: Option<String>,
    #[expect(
        dead_code,
        reason = "kept for API compatibility with Hteam list payloads"
    )]
    #[serde(skip)]
    #[tabled(skip)]
    pub position: Option<u64>,
    #[expect(
        dead_code,
        reason = "kept for API compatibility with Hteam list payloads"
    )]
    #[serde(skip)]
    #[tabled(skip)]
    pub show_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardListResponse {
    pub id: u64,
    pub name: String,
    pub cardlist_list: Vec<CardListEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardListEntry {
    pub id: u64,
    pub board_id: u64,
    pub card: Card,
    pub is_closed: bool,
    pub card_type: u64,
    pub time_status: u64,
    pub get_time_status: String,
    pub position: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Tabled)]
pub struct Card {
    pub id: u64,
    #[serde(rename = "title")]
    pub name: String,
    #[serde(rename = "subtitle")]
    #[tabled(skip)]
    pub description: Option<String>,
    #[expect(
        dead_code,
        reason = "kept for API compatibility with Hteam card payloads"
    )]
    #[serde(skip)]
    #[tabled(skip)]
    pub status: Option<String>,
    #[serde(rename = "labels_list")]
    #[tabled(display_with = "display_labels")]
    pub labels: Vec<Label>,
    #[expect(
        dead_code,
        reason = "kept for API compatibility with Hteam card payloads"
    )]
    #[serde(skip)]
    #[tabled(skip)]
    pub number: Option<u64>,
    #[expect(
        dead_code,
        reason = "kept for API compatibility with Hteam card payloads"
    )]
    #[serde(skip)]
    #[tabled(skip)]
    pub title_url: Option<String>,
    #[expect(
        dead_code,
        reason = "kept for API compatibility with Hteam card payloads"
    )]
    #[serde(skip)]
    #[tabled(skip)]
    pub subtitle_url: Option<String>,
    #[expect(
        dead_code,
        reason = "kept for API compatibility with Hteam card payloads"
    )]
    #[serde(skip)]
    #[tabled(skip)]
    pub responsible: Option<serde_json::Value>,
    #[expect(
        dead_code,
        reason = "kept for API compatibility with Hteam card payloads"
    )]
    #[serde(skip)]
    #[tabled(skip)]
    pub worker: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Label {
    pub id: u64,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardDetail {
    pub id: u64,
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(deserialize_with = "deserialize_labels")]
    pub labels: Vec<Label>,
    pub list: Option<List>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub time_estimated: Option<String>,
    #[serde(default)]
    pub start_date: Option<String>,
    #[serde(default)]
    pub dependence: Option<serde_json::Value>,
    #[serde(default)]
    pub responsible: Option<serde_json::Value>,
    #[serde(default)]
    pub priority: Option<serde_json::Value>,
}

fn deserialize_labels<'de, D>(deserializer: D) -> Result<Vec<Label>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;

    // Try to deserialize as various types
    let value: serde_json::Value = Deserialize::deserialize(deserializer)?;

    match value {
        serde_json::Value::Array(arr) => {
            // Try to parse as array of Label objects
            let mut labels = Vec::new();
            for item in arr {
                if let Ok(label) = serde_json::from_value::<Label>(item.clone()) {
                    labels.push(label);
                }
                // If parsing fails, skip this item
            }
            Ok(labels)
        }
        serde_json::Value::Number(_) | serde_json::Value::String(_) => {
            // If it's a number or string, return empty vector
            Ok(Vec::new())
        }
        _ => Ok(Vec::new()),
    }
}

#[expect(dead_code, reason = "kept for API compatibility with board payloads")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Board {
    pub id: u64,
    pub name: String,
    pub lists: Vec<List>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkingOnResponse {
    pub id: u64,
    pub user: u64,
    pub object_id: u64,
    pub content_type: u64,
    pub created_at: String,
    pub updated_at: String,
    pub content_object: ContentObject,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentObject {
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Tabled)]
pub struct WorkingOnStatus {
    pub id: u64,
    #[tabled(rename = "Card ID")]
    pub card_id: u64,
    #[tabled(rename = "Card Name")]
    pub card_name: String,
    #[tabled(rename = "Started At")]
    pub started_at: String,
}

impl From<WorkingOnResponse> for WorkingOnStatus {
    fn from(resp: WorkingOnResponse) -> Self {
        Self {
            id: resp.id,
            card_id: resp.object_id,
            card_name: resp.content_object.title,
            started_at: resp.created_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Tabled)]
pub struct ProjectMilestone {
    #[tabled(rename = "Milestone")]
    pub name: String,
    #[tabled(display_with = "display_progress", rename = "Progreso")]
    pub progress: f64,
    #[tabled(skip)]
    pub url: String,
}

impl<'de> serde::Deserialize<'de> for ProjectMilestone {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let (name, progress, url): (String, f64, String) =
            serde::Deserialize::deserialize(deserializer)?;
        Ok(Self {
            name,
            progress,
            url,
        })
    }
}

fn display_progress(v: &f64) -> String {
    // milestones usan escala 0.0-1.0, tasks usan 0.0-100.0
    let pct = if *v <= 1.0 {
        (v * 100.0).round() as usize
    } else {
        v.round() as usize
    };
    let pct = pct.min(100);
    let filled = pct / 10;
    let bar = format!("{}{}", "█".repeat(filled), "░".repeat(10 - filled));
    format!("{} {}%", bar, pct)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectTasksResponse {
    pub count: u64,
    pub results: Vec<ProjectTask>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Tabled)]
pub struct ProjectTask {
    #[tabled(rename = "#")]
    pub number: u64,
    #[tabled(rename = "Task")]
    pub name: String,
    #[tabled(display_with = "display_milestone", rename = "Milestone")]
    pub milestone: Option<ProjectMilestoneRef>,
    #[tabled(display_with = "display_progress", rename = "Progreso")]
    pub progress: f64,
    #[tabled(display_with = "display_closed", rename = "Estado")]
    pub closed: bool,
    #[tabled(display_with = "display_responsible", rename = "Resp.")]
    pub responsible: Option<ProjectResponsible>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMilestoneRef {
    pub id: u64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectResponsible {
    pub email: String,
    pub username: String,
}

fn display_milestone(opt: &Option<ProjectMilestoneRef>) -> String {
    opt.as_ref()
        .map(|m| m.name.clone())
        .unwrap_or_else(|| "-".to_string())
}

fn display_responsible(opt: &Option<ProjectResponsible>) -> String {
    opt.as_ref()
        .map(|r| r.username.clone())
        .unwrap_or_else(|| "-".to_string())
}

fn display_closed(v: &bool) -> String {
    if *v {
        "✅".to_string()
    } else {
        "🔲".to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Tabled)]
pub struct Reminder {
    pub id: u64,
    #[tabled(rename = "Card ID")]
    pub object_id: u64,
    #[serde(rename = "type", default)]
    #[tabled(skip)]
    pub reminder_type: Option<u64>,
    #[serde(default)]
    #[tabled(display_with = "display_option", rename = "Date")]
    pub reminder_date: Option<String>,
    #[serde(default)]
    #[tabled(display_with = "display_content_title", rename = "Card")]
    pub content_object: Option<ContentObject>,
}

fn display_content_title(opt: &Option<ContentObject>) -> String {
    match opt {
        Some(o) => o.title.clone(),
        None => "-".to_string(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Tabled)]
pub struct UserSuggestion {
    /// This select2-style endpoint returns `id` as a string (e.g. `"8"`),
    /// not a number.
    pub id: String,
    /// The plain username to insert for a mention (e.g. `"oscarc"`).
    #[serde(default)]
    #[tabled(rename = "Usuario")]
    pub selected_text: String,
    /// Friendly display label the endpoint already formats for us (e.g.
    /// `"oscarc (oscarc@ditra.mx)"`).
    #[serde(default)]
    #[tabled(rename = "Detalle")]
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserAutocompleteResponse {
    pub results: Vec<UserSuggestion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemindersResponse {
    pub reminders: Vec<Reminder>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckInResult {
    pub check_in: Option<String>,
    #[serde(default)]
    pub check_out: Option<String>,
    #[serde(default)]
    pub shift_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkShiftUser {
    pub id: u64,
    pub username: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkShiftRecord {
    pub id: u64,
    pub user: WorkShiftUser,
    pub check_in: Option<String>,
    pub check_out: Option<String>,
}

/// Respuesta de `/tr/checkworkshifs/resume/` — sólo se usa `last` para
/// mostrar el estado del turno actual en el header del TUI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkShiftResume {
    pub last: Option<WorkShiftRecord>,
}

fn truncate_comment(s: &str) -> String {
    let s = s.replace('\n', " ");
    if s.chars().count() > 80 {
        format!("{}…", s.chars().take(79).collect::<String>())
    } else {
        s
    }
}

fn display_option<T: ToString>(opt: &Option<T>) -> String {
    match opt {
        Some(v) => v.to_string(),
        None => "-".to_string(),
    }
}

fn display_labels(labels: &[Label]) -> String {
    if labels.is_empty() {
        return "-".to_string();
    }
    labels
        .iter()
        .map(|l| l.name.clone())
        .collect::<Vec<_>>()
        .join(", ")
}

#[derive(Debug, Clone, Serialize, Deserialize, Tabled)]
pub struct Comment {
    pub id: u64,
    #[tabled(rename = "Usuario")]
    pub user_name: String,
    #[serde(default)]
    #[tabled(rename = "Fecha")]
    pub submit_date: String,
    #[tabled(display_with = "truncate_comment", rename = "Comentario")]
    pub comment: String,
    #[serde(default)]
    #[tabled(skip)]
    pub level: u64,
    #[serde(default)]
    #[tabled(skip)]
    pub parent_id: Option<u64>,
    #[serde(default)]
    #[tabled(skip)]
    pub is_removed: bool,
    #[serde(default)]
    #[tabled(skip)]
    pub permalink: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CommentsResponse {
    pub results: Vec<Comment>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Tabled)]
pub struct DailyWorkEntry {
    #[tabled(rename = "Usuario")]
    pub user: String,
    #[tabled(display_with = "truncate_comment", rename = "Actividad")]
    pub activity: String,
    #[serde(default)]
    #[tabled(skip)]
    pub target_url: Option<String>,
    #[tabled(rename = "Fecha")]
    pub timestamp: String,
    #[tabled(rename = "Hace")]
    pub relative: String,
}

/// Un board listado por el endpoint datatables de operaciones.
#[derive(Debug, Clone, Deserialize)]
pub struct BoardEntry {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub service: String,
    #[expect(
        dead_code,
        reason = "kept for API compatibility with board datatables payloads"
    )]
    #[serde(default)]
    pub responsible: String,
    #[serde(default)]
    pub total_tasks: u64,
    #[serde(default)]
    pub total_tasks_closed: u64,
}

/// Respuesta de `GET /api/operation/care/operations/?format=datatables`.
#[derive(Debug, Deserialize)]
pub struct BoardsResponse {
    pub data: Vec<BoardEntry>,
}

impl std::fmt::Display for Label {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn list_deserializes_and_renames_card_count() {
        let list: List = serde_json::from_value(json!({
            "id": 1,
            "name": "Backlog",
            "total_board_cards": 3
        }))
        .unwrap();

        assert_eq!(list.id, 1);
        assert_eq!(list.name, "Backlog");
        assert_eq!(list.card_count, Some(3));
    }

    #[test]
    fn card_deserializes_title_and_subtitle_renames() {
        let card: Card = serde_json::from_value(json!({
            "id": 42,
            "title": "Fix bug",
            "subtitle": "Details here",
            "labels_list": [{"id": 1, "name": "bug", "color": "#fff"}]
        }))
        .unwrap();

        assert_eq!(card.name, "Fix bug");
        assert_eq!(card.description.as_deref(), Some("Details here"));
        assert_eq!(card.labels.len(), 1);
        assert_eq!(card.labels[0].name, "bug");
    }

    #[test]
    fn deserialize_labels_parses_array_of_valid_labels() {
        let detail: CardDetail = serde_json::from_value(json!({
            "id": 1,
            "name": "Card",
            "description": null,
            "labels": [{"id": 1, "name": "bug", "color": "#fff"}]
        }))
        .unwrap();

        assert_eq!(detail.labels.len(), 1);
        assert_eq!(detail.labels[0].name, "bug");
    }

    #[test]
    fn deserialize_labels_skips_invalid_array_items() {
        let detail: CardDetail = serde_json::from_value(json!({
            "id": 1,
            "name": "Card",
            "description": null,
            "labels": [{"id": 1, "name": "bug", "color": "#fff"}, "not-a-label"]
        }))
        .unwrap();

        assert_eq!(detail.labels.len(), 1);
    }

    #[test]
    fn deserialize_labels_returns_empty_for_number_or_string() {
        let from_number: CardDetail = serde_json::from_value(json!({
            "id": 1,
            "name": "Card",
            "description": null,
            "labels": 0
        }))
        .unwrap();
        let from_string: CardDetail = serde_json::from_value(json!({
            "id": 1,
            "name": "Card",
            "description": null,
            "labels": "none"
        }))
        .unwrap();

        assert!(from_number.labels.is_empty());
        assert!(from_string.labels.is_empty());
    }

    #[test]
    fn working_on_response_converts_into_status() {
        let resp = WorkingOnResponse {
            id: 1,
            user: 2,
            object_id: 99,
            content_type: 3,
            created_at: "2026-08-28T00:00:00Z".to_string(),
            updated_at: "2026-08-28T00:00:00Z".to_string(),
            content_object: ContentObject {
                title: "My Card".to_string(),
                url: "/cards/99".to_string(),
            },
        };

        let status: WorkingOnStatus = resp.into();

        assert_eq!(status.card_id, 99);
        assert_eq!(status.card_name, "My Card");
        assert_eq!(status.started_at, "2026-08-28T00:00:00Z");
    }

    #[test]
    fn project_milestone_deserializes_from_tuple() {
        let milestone: ProjectMilestone =
            serde_json::from_value(json!(["Sprint 1", 0.5, "/milestones/1"])).unwrap();

        assert_eq!(milestone.name, "Sprint 1");
        assert_eq!(milestone.progress, 0.5);
        assert_eq!(milestone.url, "/milestones/1");
    }

    #[test]
    fn display_progress_handles_fraction_and_percentage_scales() {
        assert_eq!(display_progress(&0.5), "█████░░░░░ 50%");
        assert_eq!(display_progress(&50.0), "█████░░░░░ 50%");
    }

    #[test]
    fn display_progress_clamps_over_100() {
        assert_eq!(display_progress(&150.0), "██████████ 100%");
    }

    #[test]
    fn display_closed_renders_check_or_box() {
        assert_eq!(display_closed(&true), "✅");
        assert_eq!(display_closed(&false), "🔲");
    }

    #[test]
    fn display_milestone_and_responsible_handle_none() {
        assert_eq!(display_milestone(&None), "-");
        assert_eq!(display_responsible(&None), "-");
    }

    #[test]
    fn display_milestone_and_responsible_handle_some() {
        let milestone = Some(ProjectMilestoneRef {
            id: 1,
            name: "Sprint 1".to_string(),
        });
        let responsible = Some(ProjectResponsible {
            email: "a@b.com".to_string(),
            username: "abrown".to_string(),
        });

        assert_eq!(display_milestone(&milestone), "Sprint 1");
        assert_eq!(display_responsible(&responsible), "abrown");
    }

    #[test]
    fn reminder_deserializes_with_defaults_when_optional_fields_missing() {
        let reminder: Reminder = serde_json::from_value(json!({
            "id": 1,
            "object_id": 99
        }))
        .unwrap();

        assert_eq!(reminder.reminder_type, None);
        assert_eq!(reminder.reminder_date, None);
        assert!(reminder.content_object.is_none());
    }

    #[test]
    fn display_content_title_handles_some_and_none() {
        let some = Some(ContentObject {
            title: "Card title".to_string(),
            url: "/x".to_string(),
        });

        assert_eq!(display_content_title(&some), "Card title");
        assert_eq!(display_content_title(&None), "-");
    }

    #[test]
    fn user_suggestion_deserializes_id_as_string() {
        let suggestion: UserSuggestion = serde_json::from_value(json!({
            "id": "8",
            "selected_text": "oscarc",
            "text": "oscarc (oscarc@ditra.mx)"
        }))
        .unwrap();

        assert_eq!(suggestion.id, "8");
        assert_eq!(suggestion.selected_text, "oscarc");
    }

    #[test]
    fn check_in_result_deserializes_with_defaults() {
        let result: CheckInResult = serde_json::from_value(json!({
            "check_in": "2026-08-28T08:00:00Z"
        }))
        .unwrap();

        assert_eq!(result.check_in.as_deref(), Some("2026-08-28T08:00:00Z"));
        assert_eq!(result.check_out, None);
        assert_eq!(result.shift_id, None);
    }

    #[test]
    fn work_shift_resume_deserializes_last_record() {
        let resume: WorkShiftResume = serde_json::from_value(json!({
            "last": {
                "id": 1,
                "user": {"id": 2, "username": "abrown"},
                "check_in": "2026-08-28T08:00:00Z",
                "check_out": null
            }
        }))
        .unwrap();

        let last = resume.last.expect("expected last record");
        assert_eq!(last.user.username, "abrown");
        assert_eq!(last.check_out, None);
    }

    #[test]
    fn truncate_comment_keeps_short_strings_untouched() {
        assert_eq!(truncate_comment("short comment"), "short comment");
    }

    #[test]
    fn truncate_comment_truncates_long_strings_with_ellipsis() {
        let long = "a".repeat(90);

        let truncated = truncate_comment(&long);

        assert_eq!(truncated.chars().count(), 80);
        assert!(truncated.ends_with('…'));
    }

    #[test]
    fn truncate_comment_replaces_newlines_with_spaces() {
        assert_eq!(truncate_comment("line one\nline two"), "line one line two");
    }

    #[test]
    fn display_option_handles_some_and_none() {
        assert_eq!(display_option(&Some(42)), "42");
        assert_eq!(display_option::<u64>(&None), "-");
    }

    #[test]
    fn display_labels_handles_empty_and_non_empty() {
        assert_eq!(display_labels(&[]), "-");

        let labels = vec![
            Label {
                id: 1,
                name: "bug".to_string(),
                color: "#fff".to_string(),
            },
            Label {
                id: 2,
                name: "urgent".to_string(),
                color: "#000".to_string(),
            },
        ];
        assert_eq!(display_labels(&labels), "bug, urgent");
    }

    #[test]
    fn comment_deserializes_with_defaults() {
        let comment: Comment = serde_json::from_value(json!({
            "id": 1,
            "user_name": "abrown",
            "comment": "Looks good"
        }))
        .unwrap();

        assert_eq!(comment.submit_date, "");
        assert_eq!(comment.level, 0);
        assert!(!comment.is_removed);
        assert_eq!(comment.parent_id, None);
    }

    #[test]
    fn daily_work_entry_deserializes() {
        let entry: DailyWorkEntry = serde_json::from_value(json!({
            "user": "abrown",
            "activity": "Commented on card",
            "timestamp": "2026-08-28T08:00:00Z",
            "relative": "hace 2 horas"
        }))
        .unwrap();

        assert_eq!(entry.user, "abrown");
        assert_eq!(entry.target_url, None);
    }

    #[test]
    fn board_entry_and_response_deserialize() {
        let boards: BoardsResponse = serde_json::from_value(json!({
            "data": [{
                "id": 1,
                "name": "Ops",
                "service": "care",
                "responsible": "abrown",
                "total_tasks": 10,
                "total_tasks_closed": 4
            }]
        }))
        .unwrap();

        assert_eq!(boards.data.len(), 1);
        assert_eq!(boards.data[0].name, "Ops");
        assert_eq!(boards.data[0].total_tasks_closed, 4);
    }

    #[test]
    fn label_display_renders_name() {
        let label = Label {
            id: 1,
            name: "bug".to_string(),
            color: "#fff".to_string(),
        };

        assert_eq!(label.to_string(), "bug");
    }
}
