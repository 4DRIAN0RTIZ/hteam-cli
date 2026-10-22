use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::client::HteamClient;
use crate::config::Config;
use crate::models::{Reminder, WeeklyObjectivesSet, WorkingOnStatus};

use super::{objectives, reminders, working};

/// One dashboard section: either the data or the error that blocked it.
/// A failed section never hides the others.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum Section<T> {
    Ok(T),
    Err { error: String },
}

impl<T> Section<T> {
    fn from_result(result: Result<T>) -> Self {
        match result {
            Ok(value) => Section::Ok(value),
            Err(err) => Section::Err {
                error: format!("{err:#}"),
            },
        }
    }

    pub fn is_ok(&self) -> bool {
        matches!(self, Section::Ok(_))
    }

    pub fn as_ok(&self) -> Option<&T> {
        match self {
            Section::Ok(value) => Some(value),
            Section::Err { .. } => None,
        }
    }

    pub fn error(&self) -> Option<&str> {
        match self {
            Section::Err { error } => Some(error.as_str()),
            Section::Ok(_) => None,
        }
    }
}

/// Weekly objectives as they appear on the dashboard: the set plus whether
/// it came from the local 24h cache (same metadata as `objectives show --json`).
#[derive(Debug, Clone, Serialize)]
pub struct DashboardObjectives {
    #[serde(flatten)]
    pub set: WeeklyObjectivesSet,
    pub from_cache: bool,
    pub synced_at: DateTime<Utc>,
}

/// Personal snapshot: working-on-it, reminders and weekly objectives.
/// Each section is independent so one failure still returns the rest.
#[derive(Debug, Clone, Serialize)]
pub struct Dashboard {
    pub working: Section<Vec<WorkingOnStatus>>,
    pub reminders: Section<Vec<Reminder>>,
    pub objectives: Section<DashboardObjectives>,
}

/// Collect the three personal sources. Working and reminders share
/// `HteamClient`; objectives reuse `operations::objectives::show` (cache +
/// SharePad). Callers persist config when objectives were refreshed.
///
/// Working and reminders run concurrently. Objectives stay sequential
/// because they need `&mut Config` for the local cache.
pub async fn load(client: &HteamClient, config: &mut Config) -> Dashboard {
    let (working, reminders) = tokio::join!(working::list_working(client), reminders::list(client));
    let objectives = load_objectives(config).await;

    Dashboard {
        working: Section::from_result(working),
        reminders: Section::from_result(reminders),
        objectives: Section::from_result(objectives),
    }
}

async fn load_objectives(config: &mut Config) -> Result<DashboardObjectives> {
    let view = objectives::show(config, false).await?;
    Ok(DashboardObjectives {
        set: view.set,
        from_cache: view.from_cache,
        synced_at: view.synced_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WeeklyObjectivesConfig;
    use crate::models::WeeklyObjective;
    use chrono::Duration;
    use mockito::Matcher;

    fn client(server_url: &str) -> HteamClient {
        HteamClient::new_for_test(
            Config::default(),
            server_url.to_string(),
            server_url.to_string(),
        )
        .expect("test client")
    }

    fn fresh_objectives_config() -> WeeklyObjectivesConfig {
        WeeklyObjectivesConfig {
            source_url: "http://127.0.0.1:1/unreachable".to_string(),
            last_synced_at: Some((Utc::now() - Duration::hours(1)).to_rfc3339()),
            cached_title: Some("Objetivos semanales".to_string()),
            cached_objectives: vec![WeeklyObjective {
                name: "Demo CCL".to_string(),
                tasks: vec![],
            }],
        }
    }

    fn working_body() -> &'static str {
        r#"[{"id":7,"user":3,"object_id":42,"content_type":99,"created_at":"2026-08-27T10:00:00Z","updated_at":"2026-08-27T10:00:00Z","content_object":{"title":"Fix bug","url":"/tasks/42/"}}]"#
    }

    fn reminders_body() -> &'static str {
        r#"{"reminders":[{"id":1,"object_id":42,"type":1,"content_object":{"title":"Fix bug","url":"/tasks/42/"}}]}"#
    }

    async fn mock_working(
        server: &mut mockito::Server,
        status: usize,
        body: &str,
    ) -> mockito::Mock {
        server
            .mock("GET", "/tr/workingonit/user/")
            .with_status(status)
            .with_body(body)
            .create_async()
            .await
    }

    async fn mock_reminders(
        server: &mut mockito::Server,
        status: usize,
        body: &str,
    ) -> mockito::Mock {
        server
            .mock("GET", "/tr/reminders/user/")
            .with_status(status)
            .with_body(body)
            .create_async()
            .await
    }

    #[tokio::test]
    async fn load_returns_all_sections_when_sources_succeed() {
        let mut server = mockito::Server::new_async().await;
        let _w = mock_working(&mut server, 200, working_body()).await;
        let _r = mock_reminders(&mut server, 200, reminders_body()).await;

        let mut config = Config {
            weekly_objectives: fresh_objectives_config(),
            ..Config::default()
        };

        let dashboard = load(&client(&server.url()), &mut config).await;

        let working = dashboard.working.as_ok().expect("working");
        assert_eq!(working.len(), 1);
        assert_eq!(working[0].card_id, 42);

        let reminders = dashboard.reminders.as_ok().expect("reminders");
        assert_eq!(reminders.len(), 1);
        assert_eq!(reminders[0].object_id, 42);

        let objectives = dashboard.objectives.as_ok().expect("objectives");
        assert!(objectives.from_cache);
        assert_eq!(objectives.set.title, "Objetivos semanales");
    }

    #[tokio::test]
    async fn load_keeps_other_sections_when_working_fails() {
        let mut server = mockito::Server::new_async().await;
        let _w = mock_working(&mut server, 500, "boom").await;
        let _r = mock_reminders(&mut server, 200, reminders_body()).await;

        let mut config = Config {
            weekly_objectives: fresh_objectives_config(),
            ..Config::default()
        };

        let dashboard = load(&client(&server.url()), &mut config).await;

        assert!(dashboard.working.error().is_some());
        assert!(dashboard.reminders.is_ok());
        assert!(dashboard.objectives.is_ok());
    }

    #[tokio::test]
    async fn load_keeps_other_sections_when_reminders_fail() {
        let mut server = mockito::Server::new_async().await;
        let _w = mock_working(&mut server, 200, working_body()).await;
        let _r = mock_reminders(&mut server, 403, "nope").await;

        let mut config = Config {
            weekly_objectives: fresh_objectives_config(),
            ..Config::default()
        };

        let dashboard = load(&client(&server.url()), &mut config).await;

        assert!(dashboard.working.is_ok());
        assert!(dashboard.reminders.error().unwrap().contains("403"));
        assert!(dashboard.objectives.is_ok());
    }

    #[tokio::test]
    async fn load_keeps_other_sections_when_objectives_fail() {
        let mut server = mockito::Server::new_async().await;
        let _w = mock_working(&mut server, 200, working_body()).await;
        let _r = mock_reminders(&mut server, 200, reminders_body()).await;
        let _o = server
            .mock("GET", "/n/okr-semanales")
            .match_query(Matcher::Any)
            .with_status(500)
            .create_async()
            .await;

        let mut config = Config {
            weekly_objectives: WeeklyObjectivesConfig {
                source_url: format!("{}/n/okr-semanales", server.url()),
                last_synced_at: Some((Utc::now() - Duration::hours(48)).to_rfc3339()),
                cached_title: Some("Anterior".to_string()),
                cached_objectives: vec![],
            },
            ..Config::default()
        };

        let dashboard = load(&client(&server.url()), &mut config).await;

        assert!(dashboard.working.is_ok());
        assert!(dashboard.reminders.is_ok());
        let err = dashboard.objectives.error().expect("objectives error");
        assert!(
            err.contains("Error al obtener los objetivos semanales de SharePad"),
            "{err}"
        );
        assert_eq!(
            config.weekly_objectives.cached_title.as_deref(),
            Some("Anterior")
        );
    }

    #[test]
    fn json_includes_error_without_hiding_ok_sections() {
        let dashboard = Dashboard {
            working: Section::Err {
                error: "working down".into(),
            },
            reminders: Section::Ok(vec![]),
            objectives: Section::Ok(DashboardObjectives {
                set: WeeklyObjectivesSet {
                    title: "OKR".into(),
                    objectives: vec![],
                },
                from_cache: true,
                synced_at: DateTime::parse_from_rfc3339("2026-10-22T12:00:00Z")
                    .unwrap()
                    .with_timezone(&Utc),
            }),
        };

        let json = serde_json::to_value(&dashboard).unwrap();
        assert_eq!(json["working"]["error"], "working down");
        assert!(json["reminders"].is_array());
        assert_eq!(json["objectives"]["from_cache"], true);
        assert_eq!(json["objectives"]["title"], "OKR");
    }
}
