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
