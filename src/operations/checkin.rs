use anyhow::Result;

use crate::client::HteamClient;
use crate::models::{CheckInResult, WorkShiftResume};

pub async fn check_in(client: &HteamClient) -> Result<CheckInResult> {
    client.check_in().await
}

pub async fn workshift_resume(client: &HteamClient) -> Result<WorkShiftResume> {
    client.get_workshift_resume().await
}
