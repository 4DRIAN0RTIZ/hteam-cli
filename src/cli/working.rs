use anyhow::Result;
use clap::{Args, Subcommand};
use tabled::{settings::Style, Table};

use crate::operations::{self, Session};

#[derive(Subcommand, Debug)]
pub enum WorkingCommands {
    /// Ver cards en los que estoy trabajando
    List,
    /// Empezar a trabajar en un card
    Start(WorkingStartArgs),
    /// Dejar de trabajar en un card
    Stop(WorkingStopArgs),
}

#[derive(Args, Debug)]
pub struct WorkingStartArgs {
    /// ID del card
    pub card_id: u64,
}

#[derive(Args, Debug)]
pub struct WorkingStopArgs {
    /// ID del registro working on it
    pub working_id: u64,
}

pub async fn execute(cmd: WorkingCommands, json: bool) -> Result<()> {
    match cmd {
        WorkingCommands::List => list_working(json).await,
        WorkingCommands::Start(args) => start_working(args, json).await,
        WorkingCommands::Stop(args) => stop_working(args, json).await,
    }
}

async fn list_working(json: bool) -> Result<()> {
    let session = Session::open().await?;

    let working = operations::working::list_working(&session.client).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&working)?);
        return Ok(());
    }

    if working.is_empty() {
        println!("⚠️  No estás trabajando en ningún card.");
        return Ok(());
    }

    let mut table = Table::new(&working);
    table.with(Style::rounded());

    println!("\n🔨 Working On It:\n");
    println!("{}", table);
    println!();

    Ok(())
}

async fn start_working(args: WorkingStartArgs, json: bool) -> Result<()> {
    let session = Session::open().await?;

    operations::working::start_working(&session.client, args.card_id).await?;

    if json {
        println!("{{\"success\": true, \"card_id\": {}}}", args.card_id);
        return Ok(());
    }

    println!("\n🔨 Ahora estás trabajando en el card {}", args.card_id);
    println!();

    Ok(())
}

async fn stop_working(args: WorkingStopArgs, json: bool) -> Result<()> {
    let session = Session::open().await?;

    operations::working::stop_working(&session.client, args.working_id).await?;

    if json {
        println!("{{\"success\": true, \"working_id\": {}}}", args.working_id);
        return Ok(());
    }

    println!("\n🛑 Dejaste de trabajar en el working {}", args.working_id);
    println!();

    Ok(())
}
