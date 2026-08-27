use anyhow::Result;
use clap::{Args, Subcommand};
use tabled::{settings::Style, Table};

use crate::operations::{self, Session};

#[derive(Subcommand, Debug)]
pub enum ProjectCommands {
    /// Ver progreso de milestones de un proyecto
    Milestones(ProjectArgs),
    /// Ver tasks de un proyecto
    Tasks(ProjectArgs),
}

#[derive(Args, Debug)]
pub struct ProjectArgs {
    /// ID del proyecto
    pub project_id: u64,
}

pub async fn execute(cmd: ProjectCommands, json: bool) -> Result<()> {
    match cmd {
        ProjectCommands::Milestones(args) => project_milestones(args, json).await,
        ProjectCommands::Tasks(args) => project_tasks(args, json).await,
    }
}

async fn project_milestones(args: ProjectArgs, json: bool) -> Result<()> {
    let session = Session::open().await?;

    let milestones = operations::projects::milestones(&session.client, args.project_id).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&milestones)?);
        return Ok(());
    }

    if milestones.is_empty() {
        println!(
            "⚠️  No se encontraron milestones para el proyecto {}.",
            args.project_id
        );
        return Ok(());
    }

    let mut table = Table::new(&milestones);
    table.with(Style::rounded());

    println!("\n🎯 Milestones del proyecto {}:\n", args.project_id);
    println!("{}", table);
    println!();

    Ok(())
}

async fn project_tasks(args: ProjectArgs, json: bool) -> Result<()> {
    let session = Session::open().await?;

    let resp = operations::projects::tasks(&session.client, args.project_id).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&resp)?);
        return Ok(());
    }

    if resp.results.is_empty() {
        println!(
            "⚠️  No se encontraron tasks para el proyecto {}.",
            args.project_id
        );
        return Ok(());
    }

    let mut table = Table::new(&resp.results);
    table.with(Style::rounded());

    println!(
        "\n📌 Tasks del proyecto {} ({} total):\n",
        args.project_id, resp.count
    );
    println!("{}", table);
    println!();

    Ok(())
}
