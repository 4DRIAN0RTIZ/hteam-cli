use anyhow::Result;

use crate::client::HteamClient;
use crate::models::DailyWorkEntry;

pub async fn history(
    client: &HteamClient,
    user: Option<&str>,
    activity_type: Option<&str>,
    range: Option<&str>,
) -> Result<Vec<DailyWorkEntry>> {
    client
        .get_daily_work_history(user, activity_type, range)
        .await
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
    async fn test_history_parses_daily_work_html() {
        let mut server = mockito::Server::new_async().await;
        let html = r#"
            <div id="history">
                <p><a href="/history/daily-work?user=ana">ana</a> updated
                <a href="/operations/483/tasks/42/">Fix bug</a>
                Aug. 27, 2026, 9:30 a.m., hace 1 hora</p>
            </div>
        "#;
        let _m = server
            .mock("GET", "/history/daily-work")
            .with_status(200)
            .with_body(html)
            .create_async()
            .await;

        let entries = history(&client(&server.url()), None, None, None)
            .await
            .expect("history");

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].user, "ana");
        assert!(entries[0].activity.contains("updated"));
        assert_eq!(entries[0].timestamp, "Aug. 27, 2026, 9:30 a.m.");
        assert_eq!(entries[0].relative, "hace 1 hora");
    }

    #[tokio::test]
    async fn test_history_parses_september_and_noon_timestamps() {
        let mut server = mockito::Server::new_async().await;
        let html = r#"
            <div id="history">
                <p><a href="/history/daily-work?user=adrian.ortiz">adrian.ortiz</a> closed the task
                <a href="/operations/447/tasks/10720/">#240 - Revision PR #231</a>
                Sept. 4, 2026, noon, hace 8 minutos</p>
            </div>
        "#;
        let _m = server
            .mock("GET", "/history/daily-work")
            .with_status(200)
            .with_body(html)
            .create_async()
            .await;

        let entries = history(&client(&server.url()), None, None, None)
            .await
            .expect("history");

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].timestamp, "Sept. 4, 2026, noon");
        assert_eq!(entries[0].relative, "hace 8 minutos");
    }
}
