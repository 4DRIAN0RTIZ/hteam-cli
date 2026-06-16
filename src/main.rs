mod cli;
mod client;
mod config;
mod mcp;
mod models;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    cli::run().await
}
