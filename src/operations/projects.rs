use anyhow::Result;

use crate::client::HteamClient;
use crate::config::Config;
use crate::models::{ProjectMilestone, ProjectTasksResponse};

pub async fn milestones(client: &HteamClient, project_id: u64) -> Result<Vec<ProjectMilestone>> {
    client.get_project_milestones(project_id).await
}

pub async fn tasks(client: &HteamClient, project_id: u64) -> Result<ProjectTasksResponse> {
    client.get_project_tasks(project_id).await
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

    fn client(server_url: &str) -> HteamClient {
        HteamClient::new_for_test(
            Config::default(),
            server_url.to_string(),
            server_url.to_string(),
        )
        .expect("test client")
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
