mod cli;
mod client;
mod config;
mod mcp;
mod models;
mod operations;
mod tui;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    cli::run().await
}
