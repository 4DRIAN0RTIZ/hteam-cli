use anyhow::Result;

use crate::client::HteamClient;
use crate::models::DailyWorkEntry;

pub async fn history(
    client: &HteamClient,
    user: Option<&str>,
    activity_type: Option<&str>,
    range: Option<&str>,
) -> Result<Vec<DailyWorkEntry>> {
    client.get_daily_work_history(user, activity_type, range).await
}
