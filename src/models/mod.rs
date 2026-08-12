use serde::{Deserialize, Serialize};
use tabled::Tabled;

#[derive(Debug, Clone, Serialize, Deserialize, Tabled)]
pub struct List {
    pub id: u64,
    pub name: String,
    #[serde(rename = "total_board_cards")]
    #[tabled(display_with = "display_option", rename = "Cards")]
    pub card_count: Option<usize>,
    #[serde(skip)]
    #[tabled(skip)]
    pub color: Option<String>,
    #[serde(skip)]
    #[tabled(skip)]
    pub position: Option<u64>,
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
    #[serde(skip)]
    #[tabled(skip)]
    pub status: Option<String>,
    #[serde(rename = "labels_list")]
    #[tabled(display_with = "display_labels")]
    pub labels: Vec<Label>,
    #[serde(skip)]
    #[tabled(skip)]
    pub number: Option<u64>,
    #[serde(skip)]
    #[tabled(skip)]
    pub title_url: Option<String>,
    #[serde(skip)]
    #[tabled(skip)]
    pub subtitle_url: Option<String>,
    #[serde(skip)]
    #[tabled(skip)]
    pub responsible: Option<serde_json::Value>,
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
        Ok(Self { name, progress, url })
    }
}

fn display_progress(v: &f64) -> String {
    // milestones usan escala 0.0-1.0, tasks usan 0.0-100.0
    let pct = if *v <= 1.0 { (v * 100.0).round() as usize } else { v.round() as usize };
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
    opt.as_ref().map(|m| m.name.clone()).unwrap_or_else(|| "-".to_string())
}

fn display_responsible(opt: &Option<ProjectResponsible>) -> String {
    opt.as_ref().map(|r| r.username.clone()).unwrap_or_else(|| "-".to_string())
}

fn display_closed(v: &bool) -> String {
    if *v { "✅".to_string() } else { "🔲".to_string() }
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

fn display_option_f64(opt: &Option<f64>) -> String {
    match opt {
        Some(v) => format!("{:.0}%", v),
        None => "-".to_string(),
    }
}

fn truncate_comment(s: &String) -> String {
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

fn display_labels(labels: &Vec<Label>) -> String {
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

impl std::fmt::Display for Label {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}
