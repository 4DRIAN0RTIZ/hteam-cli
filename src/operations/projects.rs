use anyhow::Result;

use crate::client::HteamClient;
use crate::config::Config;
use crate::models::{ProjectMilestone, ProjectSummary, ProjectTasksResponse};

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

/// URL pública de la página de detalle de un proyecto (`/projects/{id}/`).
pub fn project_url(client: &HteamClient, project_id: u64) -> String {
    client.project_url(project_id)
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
        assert_eq!(projects[0].due_date.as_deref(), Some("2025-06-06T00:00:00-06:00"));
        assert_eq!(projects[0].members, vec![114, 118]);
    }

    #[tokio::test]
    async fn test_list_forwards_explicit_status() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/project-new/care/projects/")
            .match_query(Matcher::UrlEncoded("status".to_string(), "Planning".to_string()))
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
                r#"{"count":1,"results":[{"number":42,"name":"Fix bug","milestone":{"id":1,"name":"M1"},"progress":50.0,"closed":false,"responsible":{"email":"ana@example.com","username":"ana"}}]}"#,
            )
            .create_async()
            .await;

        let response = tasks(&client(&server.url()), 7).await.expect("tasks");

        assert_eq!(response.count, 1);
        assert_eq!(response.results[0].name, "Fix bug");
    }
}
