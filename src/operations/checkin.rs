use anyhow::Result;

use crate::client::HteamClient;
use crate::models::{CheckInResult, WorkShiftResume};

pub async fn check_in(client: &HteamClient) -> Result<CheckInResult> {
    client.check_in().await
}

pub async fn workshift_resume(client: &HteamClient) -> Result<WorkShiftResume> {
    client.get_workshift_resume().await
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
    async fn test_check_in_returns_result() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("POST", "/tr/checkworkshifs/check_in/")
            .with_status(200)
            .with_body(r#"{"check_in":"09:00","check_out":null,"shift_id":5}"#)
            .create_async()
            .await;

        let result = check_in(&client(&server.url())).await.expect("check in");

        assert_eq!(result.check_in.as_deref(), Some("09:00"));
        assert_eq!(result.shift_id, Some(5));
    }

    #[tokio::test]
    async fn test_workshift_resume_returns_last_record() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/tr/checkworkshifs/resume/")
            .with_status(200)
            .with_body(
                r#"{"last":{"id":5,"user":{"id":3,"username":"ana"},"check_in":"09:00","check_out":null}}"#,
            )
            .create_async()
            .await;

        let resume = workshift_resume(&client(&server.url()))
            .await
            .expect("resume");

        assert_eq!(resume.last.expect("last").user.username, "ana");
    }
}
