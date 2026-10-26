use anyhow::{Context, Result};

use crate::client::HteamClient;
use crate::config::Config;
use crate::models::{
    ProjectDueTask, ProjectMilestone, ProjectStatusSummary, ProjectSummary, ProjectTaskTreeNode,
    ProjectTasksResponse,
};

/// Los mismos radios de estado que `/projects/` en el sitio, en su mismo
/// orden — única fuente de verdad para el filtro tanto en `hteam project
/// list --status` como en el ciclo `h`/`l` del listado en vivo del TUI.
pub const PROJECT_STATUSES: [&str; 5] = ["All", "Planning", "Execution", "Finished", "Cancelled"];

/// Proyectos en `status` (default "Execution" — mismo default que `/projects/`
/// en el sitio). Ver `PROJECT_STATUSES` para los valores válidos.
pub async fn list(client: &HteamClient, status: Option<&str>) -> Result<Vec<ProjectSummary>> {
    client.get_projects(status).await
}

pub async fn milestones(client: &HteamClient, project_id: u64) -> Result<Vec<ProjectMilestone>> {
    client.get_project_milestones(project_id).await
}

pub async fn tasks(client: &HteamClient, project_id: u64) -> Result<ProjectTasksResponse> {
    client.get_project_tasks(project_id).await
}

pub async fn task_tree(client: &HteamClient, project_id: u64) -> Result<Vec<ProjectTaskTreeNode>> {
    client.get_project_task_tree(project_id).await
}

pub async fn set_members(client: &HteamClient, project_id: u64, member_ids: &[u64]) -> Result<()> {
    client.update_project_members(project_id, member_ids).await
}

pub async fn status(client: &HteamClient, project_id: u64) -> Result<ProjectStatusSummary> {
    let project = list(client, Some("All"))
        .await?
        .into_iter()
        .find(|project| project.id == project_id)
        .with_context(|| format!("proyecto {project_id} no encontrado"))?;
    let milestones = milestones(client, project_id).await?;
    let tasks = tasks(client, project_id).await?;
    let today = chrono::Local::now().date_naive();
    let upcoming_limit = today + chrono::Days::new(7);
    let mut overdue_tasks = Vec::new();
    let mut upcoming_tasks = Vec::new();

    for task in &tasks.results {
        if task.closed {
            continue;
        }
        let detail = client.get_project_task_detail(task.id).await?;
        let Some(due_date) = detail.due_date else {
            continue;
        };
        let date =
            chrono::NaiveDate::parse_from_str(&due_date[..due_date.len().min(10)], "%Y-%m-%d").ok();
        let due_task = ProjectDueTask {
            id: task.id,
            number: task.number,
            name: task.name.clone(),
            due_date,
        };
        match date {
            Some(date) if date < today => overdue_tasks.push(due_task),
            Some(date) if date <= upcoming_limit => upcoming_tasks.push(due_task),
            _ => {}
        }
    }

    Ok(ProjectStatusSummary {
        project,
        milestones,
        total_tasks: tasks.count,
        closed_tasks: tasks.results.iter().filter(|task| task.closed).count() as u64,
        overdue_tasks,
        upcoming_tasks,
        times: client.get_project_times(project_id).await?,
    })
}

/// URL pública de la página de detalle de un proyecto (`/projects/{id}/`).
pub fn project_url(client: &HteamClient, project_id: u64) -> String {
    client.project_url(project_id)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MilestoneWriteResult {
    pub project_id: u64,
    pub milestone_id: Option<u64>,
    pub name: String,
}

fn to_write_result(w: crate::client::MilestoneWrite) -> MilestoneWriteResult {
    MilestoneWriteResult {
        project_id: w.project_id,
        milestone_id: w.milestone_id,
        name: w.name,
    }
}

pub async fn create_milestone(
    client: &HteamClient,
    project_id: u64,
    name: &str,
    description: Option<&str>,
) -> Result<MilestoneWriteResult> {
    let written = client
        .create_project_milestone(project_id, name, description)
        .await?;
    Ok(to_write_result(written))
}

pub async fn delete_milestone(
    client: &HteamClient,
    project_id: u64,
    milestone_id: u64,
) -> Result<()> {
    client
        .delete_project_milestone(project_id, milestone_id)
        .await
}

pub async fn edit_milestone(
    client: &HteamClient,
    project_id: u64,
    milestone_id: u64,
    name: Option<&str>,
    description: Option<&str>,
) -> Result<MilestoneWriteResult> {
    let written = client
        .edit_project_milestone(project_id, milestone_id, name, description)
        .await?;
    Ok(to_write_result(written))
}

#[derive(Debug, Clone, Default)]
pub struct ProjectTaskCreate<'a> {
    pub name: &'a str,
    pub description: Option<&'a str>,
    pub time_estimated: Option<&'a str>,
    pub start_date: Option<&'a str>,
    pub dependence: Option<u64>,
    pub responsible: Option<u64>,
    pub priority: Option<u8>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProjectTaskWriteResult {
    pub project_id: u64,
    pub milestone_id: u64,
    pub task_id: Option<u64>,
    pub name: String,
}

pub async fn resolve_user_id(client: &HteamClient, reference: &str) -> Result<u64> {
    if let Ok(id) = reference.parse() {
        return Ok(id);
    }
    let users = crate::operations::users::search(client, reference).await?;
    users
        .into_iter()
        .find(|user| user.selected_text.eq_ignore_ascii_case(reference))
        .context("no se encontró un usuario exacto; usa `hteam users <query>` para buscarlo")?
        .id
        .parse()
        .context("Hteam devolvió un ID de usuario inválido")
}

#[derive(Debug, Clone, Default)]
pub struct ProjectTaskEdit<'a> {
    pub milestone_id: Option<u64>,
    pub dependence: Option<u64>,
    pub start_date: Option<&'a str>,
}

pub async fn edit_task(
    client: &HteamClient,
    project_id: u64,
    task_id: u64,
    task: ProjectTaskEdit<'_>,
) -> Result<ProjectTaskWriteResult> {
    if task.milestone_id.is_none() && task.dependence.is_none() && task.start_date.is_none() {
        anyhow::bail!("indica al menos --milestone, --dependence o --start-date");
    }
    if let Some(date) = task.start_date {
        if date.len() != 10
            || !date.as_bytes().get(4).is_some_and(|b| *b == b'-')
            || !date.as_bytes().get(7).is_some_and(|b| *b == b'-')
        {
            anyhow::bail!("--start-date debe usar el formato YYYY-MM-DD");
        }
    }
    let written = client
        .edit_project_task(
            project_id,
            task_id,
            crate::client::ProjectTaskEditRequest {
                milestone_id: task.milestone_id,
                dependence: task.dependence,
                start_date: task.start_date,
            },
        )
        .await?;
    Ok(ProjectTaskWriteResult {
        project_id: written.project_id,
        milestone_id: written.milestone_id,
        task_id: written.task_id,
        name: written.name,
    })
}

pub async fn create_task(
    client: &HteamClient,
    project_id: u64,
    milestone_id: u64,
    task: ProjectTaskCreate<'_>,
) -> Result<ProjectTaskWriteResult> {
    let written = client
        .create_project_task(
            project_id,
            milestone_id,
            crate::client::ProjectTaskCreateRequest {
                name: task.name,
                description: task.description,
                time_estimated: task.time_estimated,
                start_date: task.start_date,
                dependence: task.dependence,
                responsible: task.responsible,
                priority: task.priority,
            },
        )
        .await?;
    Ok(ProjectTaskWriteResult {
        project_id: written.project_id,
        milestone_id: written.milestone_id,
        task_id: written.task_id,
        name: written.name,
    })
}

/// Moves `id` to the front of the known-projects MRU list (deduping) and
/// persists it to `config.toml`'s `[tui] known_projects`.
pub fn remember_project(config: &mut Config, id: u64) -> Result<()> {
    config.tui.known_projects.retain(|&p| p != id);
    config.tui.known_projects.insert(0, id);
    config.save()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use mockito::Matcher;

    fn client(server_url: &str) -> HteamClient {
        HteamClient::new_for_test(
            Config::default(),
            server_url.to_string(),
            server_url.to_string(),
        )
        .expect("test client")
    }

    #[tokio::test]
    async fn test_list_returns_projects_and_defaults_status_to_execution() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/project-new/care/projects/")
            .match_query(Matcher::AllOf(vec![
                Matcher::UrlEncoded("format".to_string(), "datatables".to_string()),
                Matcher::UrlEncoded("status".to_string(), "Execution".to_string()),
            ]))
            .with_status(200)
            .with_body(
                r#"{"data":[{"id":378,"name":"Proyecto X","service":"Plataforma::Apps","priority":3,"clc_project_progress":97.46,"clc_time_deviation":-190.57,"estimated_due_date":"2025-10-31T00:00:00-06:00","budget_exercised":0,"due_date":"2025-06-06T00:00:00-06:00","members":[114,118],"status":"Execution"}]}"#,
            )
            .create_async()
            .await;

        let projects = list(&client(&server.url()), None).await.expect("projects");

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].id, 378);
        assert_eq!(projects[0].name, "Proyecto X");
        assert_eq!(projects[0].progress, 97.46);
        assert_eq!(
            projects[0].due_date.as_deref(),
            Some("2025-06-06T00:00:00-06:00")
        );
        assert_eq!(projects[0].members, vec![114, 118]);
    }

    #[tokio::test]
    async fn test_list_forwards_explicit_status() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/project-new/care/projects/")
            .match_query(Matcher::UrlEncoded(
                "status".to_string(),
                "Planning".to_string(),
            ))
            .with_status(200)
            .with_body(r#"{"data":[]}"#)
            .create_async()
            .await;

        let projects = list(&client(&server.url()), Some("Planning"))
            .await
            .expect("projects");

        assert!(projects.is_empty());
    }

    #[tokio::test]
    async fn test_milestones_returns_project_progress() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/project-new/care/projects/7/milestones_progress/")
            .with_status(200)
            .with_body(r#"[["M1",0.5,"/m/1"]]"#)
            .create_async()
            .await;

        let milestones = milestones(&client(&server.url()), 7)
            .await
            .expect("milestones");

        assert_eq!(milestones[0].name, "M1");
        assert_eq!(milestones[0].progress, 0.5);
    }

    #[tokio::test]
    async fn resolve_user_id_accepts_an_exact_username() {
        let mut server = mockito::Server::new_async().await;
        let _users = server
            .mock("GET", "/users/username-autocomplete/")
            .match_query(Matcher::UrlEncoded("q".into(), "egaleana".into()))
            .with_status(200)
            .with_body(
                r#"{"results":[{"id":"114","selected_text":"egaleana","text":"egaleana (egaleana@example.com)"}]}"#,
            )
            .create_async()
            .await;

        assert_eq!(
            resolve_user_id(&client(&server.url()), "egaleana")
                .await
                .expect("user ID"),
            114
        );
    }

    #[tokio::test]
    async fn create_task_posts_project_milestone_form() {
        let mut server = mockito::Server::new_async().await;
        let _form = server
            .mock("GET", "/projects/469/milestones/835/task/create/")
            .with_status(200)
            .with_body(r#"<form><input name="csrfmiddlewaretoken" value="csrf"><input name="name"></form>"#)
            .create_async()
            .await;
        let _post = server
            .mock("POST", "/projects/469/milestones/835/task/create/")
            .match_body(Matcher::UrlEncoded(
                "csrfmiddlewaretoken".into(),
                "csrf".into(),
            ))
            .match_body(Matcher::UrlEncoded("process_id".into(), "469".into()))
            .match_body(Matcher::UrlEncoded("name".into(), "Plan+release".into()))
            .match_body(Matcher::UrlEncoded("priority".into(), "4".into()))
            .with_status(200)
            .with_body(r#"<div>Created task: Plan release</div><a href="/projects/469/tasks/996/">Plan release</a>"#)
            .create_async()
            .await;

        let result = create_task(
            &client(&server.url()),
            469,
            835,
            ProjectTaskCreate {
                name: "Plan release",
                priority: Some(4),
                ..Default::default()
            },
        )
        .await
        .expect("create task");
        assert_eq!(result.name, "Plan release");
        assert_eq!(result.milestone_id, 835);
        assert_eq!(result.task_id, Some(996));
    }

    #[test]
    fn test_project_url_builds_link() {
        let url = project_url(&client("https://hteam.mx"), 472);

        assert_eq!(url, "https://hteam.mx/projects/472/");
    }

    #[tokio::test]
    async fn test_tasks_returns_project_tasks() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/project-new/care/7/tasks-projects/")
            .with_status(200)
            .with_body(
                r#"{"count":1,"results":[{"id":42,"number":7,"name":"Fix bug","milestone":{"id":1,"name":"M1"},"progress":50.0,"closed":false,"responsible":{"email":"ana@example.com","username":"ana"}}]}"#,
            )
            .create_async()
            .await;

        let response = tasks(&client(&server.url()), 7).await.expect("tasks");

        assert_eq!(response.count, 1);
        assert_eq!(response.results[0].id, 42);
        assert_eq!(response.results[0].name, "Fix bug");
    }

    #[tokio::test]
    async fn set_members_replaces_the_complete_member_list() {
        let mut server = mockito::Server::new_async().await;
        let _update = server
            .mock("POST", "/project-new/care/projects/7/update_members/")
            .match_header(
                "content-type",
                "application/x-www-form-urlencoded; charset=UTF-8",
            )
            .match_body("members=140&members=114")
            .with_status(200)
            .create_async()
            .await;

        set_members(&client(&server.url()), 7, &[140, 114])
            .await
            .expect("members updated");
    }

    #[tokio::test]
    async fn task_tree_parses_nested_dependencies() {
        let mut server = mockito::Server::new_async().await;
        let _tree = server
            .mock("GET", "/projects/7/task-tree")
            .with_status(200)
            .with_body(
                r#"<ul><li><a href="/projects/7/tasks/42/">Parent task</a><ul><li><span><a href="/projects/7/tasks/43/">Child task</a></span></li></ul></li></ul>"#,
            )
            .create_async()
            .await;

        let tree = task_tree(&client(&server.url()), 7)
            .await
            .expect("task tree");

        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].id, 42);
        assert_eq!(tree[0].name, "Parent task");
        assert_eq!(tree[0].children[0].id, 43);
        assert_eq!(tree[0].children[0].name, "Child task");
    }

    #[tokio::test]
    async fn create_milestone_posts_scraped_csrf_and_process() {
        let mut server = mockito::Server::new_async().await;
        let _form = server
            .mock("GET", "/projects/469/milestones/create/")
            .with_status(200)
            .with_body(
                r#"<form method="post"><input name="csrfmiddlewaretoken" value="csrf"><input name="name" value=""><textarea name="description"></textarea><input name="process" value="469"></form>"#,
            )
            .create_async()
            .await;
        let _milestones = server
            .mock("GET", "/project-new/care/projects/469/milestones_progress/")
            .with_status(200)
            .with_body(r#"[["Release prep",0,"/projects/469/milestones/835/"]]"#)
            .create_async()
            .await;
        let _post = server
            .mock("POST", "/projects/469/milestones/create/")
            .match_body(Matcher::UrlEncoded(
                "csrfmiddlewaretoken".into(),
                "csrf".into(),
            ))
            .match_body(Matcher::UrlEncoded("name".into(), "Release+prep".into()))
            .match_body(Matcher::UrlEncoded("description".into(), "Notes".into()))
            .match_body(Matcher::UrlEncoded("process".into(), "469".into()))
            .with_status(200)
            .with_body("saved")
            .create_async()
            .await;

        let result = create_milestone(&client(&server.url()), 469, "Release prep", Some("Notes"))
            .await
            .expect("create milestone");

        assert_eq!(result.project_id, 469);
        assert_eq!(result.name, "Release prep");
        assert_eq!(result.milestone_id, Some(835));
    }

    #[tokio::test]
    async fn edit_milestone_preserves_unspecified_fields() {
        let mut server = mockito::Server::new_async().await;
        let _form = server
            .mock("GET", "/projects/469/milestones/835/edit/")
            .with_status(200)
            .with_body(
                r#"<form method="post"><input name="csrfmiddlewaretoken" value="csrf"><input name="name" value="GWTC"><textarea name="description">Current notes</textarea><input name="process" value="469"></form>"#,
            )
            .create_async()
            .await;
        let _post = server
            .mock("POST", "/projects/469/milestones/835/edit/")
            .match_body(Matcher::UrlEncoded("name".into(), "New+name".into()))
            .match_body(Matcher::UrlEncoded(
                "description".into(),
                "Current+notes".into(),
            ))
            .match_body(Matcher::UrlEncoded("process".into(), "469".into()))
            .with_status(200)
            .with_body("saved")
            .create_async()
            .await;

        let result = edit_milestone(&client(&server.url()), 469, 835, Some("New name"), None)
            .await
            .expect("edit milestone");

        assert_eq!(result.milestone_id, Some(835));
        assert_eq!(result.name, "New name");
    }

    #[tokio::test]
    async fn edit_task_preserves_unmodified_form_fields() {
        let mut server = mockito::Server::new_async().await;
        let form = r#"<form><input name="csrfmiddlewaretoken" value="csrf"><input name="name" value="Plan"><textarea name="description">Notes</textarea><input name="time_estimated" value="1h"><input name="start_date" value="2026-10-01"><select name="dependence"><option value="12" selected>Previous</option></select><select name="responsible"><option value="114" selected>egaleana</option></select><select name="priority"><option value="3" selected>Normal</option></select><select name="milestone"><option value="835" selected>M1</option></select></form>"#;
        let _form = server
            .mock("GET", "/projects/469/tasks/996/edit/")
            .with_status(200)
            .with_body(form)
            .create_async()
            .await;
        let _post = server
            .mock("POST", "/projects/469/tasks/996/edit/")
            .match_body(Matcher::UrlEncoded("milestone".into(), "836".into()))
            .match_body(Matcher::UrlEncoded("dependence".into(), "12".into()))
            .match_body(Matcher::UrlEncoded(
                "start_date".into(),
                "2026-10-02".into(),
            ))
            .match_body(Matcher::UrlEncoded("time_estimated".into(), "1h".into()))
            .match_body(Matcher::UrlEncoded("responsible".into(), "114".into()))
            .with_status(200)
            .with_body("saved")
            .create_async()
            .await;

        let result = edit_task(
            &client(&server.url()),
            469,
            996,
            ProjectTaskEdit {
                milestone_id: Some(836),
                start_date: Some("2026-10-02"),
                ..Default::default()
            },
        )
        .await
        .expect("edit task");

        assert_eq!(result.milestone_id, 836);
        assert_eq!(result.task_id, Some(996));
        assert_eq!(result.name, "Plan");
    }

    #[tokio::test]
    async fn edit_milestone_requires_a_change() {
        let err = edit_milestone(&client("https://example.test"), 469, 835, None, None)
            .await
            .expect_err("must reject empty edit");
        assert!(err.to_string().contains("--name o --description"));
    }
}
