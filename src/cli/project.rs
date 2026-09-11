use anyhow::Result;
use clap::{Args, Subcommand};
use tabled::{settings::Style, Table};

use crate::operations::{self, Session};

#[derive(Subcommand, Debug)]
pub enum ProjectCommands {
    /// Listar proyectos (por defecto, en estado "Execution")
    List(ProjectListArgs),
    /// Ver progreso de milestones de un proyecto
    Milestones(ProjectArgs),
    /// Ver tasks de un proyecto
    Tasks(ProjectArgs),
    /// Copiar el link de un proyecto al portapapeles
    Link(ProjectLinkArgs),
}

#[derive(Args, Debug)]
pub struct ProjectArgs {
    /// ID del proyecto
    pub project_id: u64,
}

#[derive(Args, Debug)]
pub struct ProjectListArgs {
    /// Estado a filtrar: All, Planning, Execution (default), Finished, Cancelled
    #[arg(short, long)]
    pub status: Option<String>,
}

#[derive(Args, Debug)]
pub struct ProjectLinkArgs {
    /// ID del proyecto
    pub project_id: u64,

    /// Solo imprimir el link, sin copiarlo al portapapeles
    #[arg(long)]
    pub no_copy: bool,
}

pub async fn execute(cmd: ProjectCommands, json: bool) -> Result<()> {
    match cmd {
        ProjectCommands::List(args) => project_list(args, json).await,
        ProjectCommands::Milestones(args) => project_milestones(args, json).await,
        ProjectCommands::Tasks(args) => project_tasks(args, json).await,
        ProjectCommands::Link(args) => project_link(args, json).await,
    }
}

async fn project_list(args: ProjectListArgs, json: bool) -> Result<()> {
    let session = Session::open().await?;

    let status = args.status.as_deref();
    let projects = operations::projects::list(&session.client, status).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&projects)?);
        return Ok(());
    }

    if projects.is_empty() {
        println!(
            "⚠️  No hay proyectos en estado '{}'.",
            status.unwrap_or("Execution")
        );
        return Ok(());
    }

    let mut table = Table::new(&projects);
    table.with(Style::rounded());

    println!(
        "\n{} Proyectos ({}):\n",
        operations::PROJECT_ICON,
        status.unwrap_or("Execution")
    );
    println!("{}", table);
    println!();

    Ok(())
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

async fn project_link(args: ProjectLinkArgs, json: bool) -> Result<()> {
    let session = Session::open().await?;

    let url = operations::projects::project_url(&session.client, args.project_id);

    let copied = if args.no_copy {
        false
    } else {
        operations::clipboard::copy(&url).is_ok()
    };

    if json {
        println!(
            "{{\"project_id\": {}, \"url\": \"{}\", \"copied\": {}}}",
            args.project_id, url, copied
        );
        return Ok(());
    }

    println!("\n{} {}", operations::COPY_ICON, url);
    if copied {
        println!("   (copiado al portapapeles)");
    }
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
