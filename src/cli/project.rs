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
    /// Crear un milestone en un proyecto
    MilestoneCreate(MilestoneCreateArgs),
    /// Editar un milestone de un proyecto
    MilestoneEdit(MilestoneEditArgs),
    /// Crear una task de proyecto dentro de un milestone
    TaskCreate(ProjectTaskCreateArgs),
    /// Editar milestone, dependencia o fecha de inicio de una task de proyecto
    TaskEdit(ProjectTaskEditArgs),
    /// Ver tasks de un proyecto
    Tasks(ProjectArgs),
    /// Resumir progreso, fechas y tiempos de un proyecto
    Status(ProjectArgs),
    /// Reemplazar los miembros de un proyecto por IDs de usuario
    MembersSet(ProjectMembersSetArgs),
    /// Ver el árbol de dependencias de tasks de un proyecto
    TaskTree(ProjectArgs),
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
pub struct MilestoneCreateArgs {
    /// ID del proyecto
    pub project_id: u64,
    /// Nombre del milestone
    #[arg(long)]
    pub name: String,
    /// Descripción (opcional)
    #[arg(long)]
    pub description: Option<String>,
}

#[derive(Args, Debug)]
pub struct MilestoneEditArgs {
    /// ID del proyecto
    pub project_id: u64,
    /// ID del milestone
    pub milestone_id: u64,
    /// Nuevo nombre (opcional)
    #[arg(long)]
    pub name: Option<String>,
    /// Nueva descripción (opcional; cadena vacía la limpia)
    #[arg(long)]
    pub description: Option<String>,
}

#[derive(Args, Debug)]
pub struct ProjectTaskEditArgs {
    /// ID del proyecto
    pub project_id: u64,
    /// ID interno de la task (visible con `hteam project tasks --json`)
    pub task_id: u64,
    /// Nuevo ID de milestone
    #[arg(long)]
    pub milestone: Option<u64>,
    /// Nuevo ID de task de la que depende
    #[arg(long)]
    pub dependence: Option<u64>,
    /// Nueva fecha de inicio (YYYY-MM-DD)
    #[arg(long)]
    pub start_date: Option<String>,
}

#[derive(Args, Debug)]
pub struct ProjectTaskCreateArgs {
    /// ID del proyecto
    pub project_id: u64,
    /// ID del milestone
    pub milestone_id: u64,
    /// Nombre de la task
    #[arg(long)]
    pub name: String,
    #[arg(long)]
    pub description: Option<String>,
    #[arg(long)]
    pub estimate: Option<String>,
    #[arg(long)]
    pub start_date: Option<String>,
    #[arg(long)]
    pub dependence: Option<u64>,
    /// ID o username del responsable
    #[arg(long)]
    pub responsible: Option<String>,
    #[arg(long)]
    pub priority: Option<u8>,
}

#[derive(Args, Debug)]
pub struct ProjectMembersSetArgs {
    /// ID del proyecto
    pub project_id: u64,
    /// IDs de usuario que quedarán como miembros
    #[arg(required = true)]
    pub member_ids: Vec<u64>,
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
        ProjectCommands::MilestoneCreate(args) => milestone_create(args, json).await,
        ProjectCommands::MilestoneEdit(args) => milestone_edit(args, json).await,
        ProjectCommands::TaskCreate(args) => task_create(args, json).await,
        ProjectCommands::TaskEdit(args) => task_edit(args, json).await,
        ProjectCommands::Tasks(args) => project_tasks(args, json).await,
        ProjectCommands::Status(args) => project_status(args, json).await,
        ProjectCommands::MembersSet(args) => project_members_set(args, json).await,
        ProjectCommands::TaskTree(args) => project_task_tree(args, json).await,
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
            "No hay proyectos en estado '{}'.",
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
            "No se encontraron milestones para el proyecto {}.",
            args.project_id
        );
        return Ok(());
    }

    let mut table = Table::new(&milestones);
    table.with(Style::rounded());

    println!("\nMilestones del proyecto {}:\n", args.project_id);
    println!("{}", table);
    println!();

    Ok(())
}

async fn milestone_create(args: MilestoneCreateArgs, json: bool) -> Result<()> {
    let session = Session::open().await?;
    let result = operations::projects::create_milestone(
        &session.client,
        args.project_id,
        &args.name,
        args.description.as_deref(),
    )
    .await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        let id = result
            .milestone_id
            .map(|id| format!(" ({id})"))
            .unwrap_or_default();
        println!("\nMilestone creado: {}{}\n", result.name, id);
    }
    Ok(())
}

async fn milestone_edit(args: MilestoneEditArgs, json: bool) -> Result<()> {
    let session = Session::open().await?;
    let result = operations::projects::edit_milestone(
        &session.client,
        args.project_id,
        args.milestone_id,
        args.name.as_deref(),
        args.description.as_deref(),
    )
    .await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!(
            "\nMilestone actualizado: {} ({})\n",
            result.name, args.milestone_id
        );
    }
    Ok(())
}

async fn task_create(args: ProjectTaskCreateArgs, json: bool) -> Result<()> {
    let session = Session::open().await?;
    let responsible = match args.responsible.as_deref() {
        Some(reference) => {
            Some(operations::projects::resolve_user_id(&session.client, reference).await?)
        }
        None => None,
    };
    let task = operations::projects::create_task(
        &session.client,
        args.project_id,
        args.milestone_id,
        operations::projects::ProjectTaskCreate {
            name: &args.name,
            description: args.description.as_deref(),
            time_estimated: args.estimate.as_deref(),
            start_date: args.start_date.as_deref(),
            dependence: args.dependence,
            responsible,
            priority: args.priority,
        },
    )
    .await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&task)?);
    } else {
        let id = task
            .task_id
            .map(|id| format!(" ({id})"))
            .unwrap_or_default();
        println!("\nTask creada: {}{}\n", task.name, id);
    }
    Ok(())
}

async fn task_edit(args: ProjectTaskEditArgs, json: bool) -> Result<()> {
    let session = Session::open().await?;
    let task = operations::projects::edit_task(
        &session.client,
        args.project_id,
        args.task_id,
        operations::projects::ProjectTaskEdit {
            milestone_id: args.milestone,
            dependence: args.dependence,
            start_date: args.start_date.as_deref(),
        },
    )
    .await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&task)?);
    } else {
        println!("\nTask actualizada: {}\n", args.task_id);
    }
    Ok(())
}

async fn project_members_set(args: ProjectMembersSetArgs, json: bool) -> Result<()> {
    let session = Session::open().await?;
    operations::projects::set_members(&session.client, args.project_id, &args.member_ids).await?;
    if json {
        println!(
            "{}",
            serde_json::json!({"project_id": args.project_id, "member_ids": args.member_ids})
        );
    } else {
        println!(
            "Miembros actualizados para el proyecto {}.",
            args.project_id
        );
    }
    Ok(())
}

async fn project_status(args: ProjectArgs, json: bool) -> Result<()> {
    let session = Session::open().await?;
    let summary = operations::projects::status(&session.client, args.project_id).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&summary)?);
        return Ok(());
    }

    println!(
        "\nEstado de {} (#{}):",
        summary.project.name, summary.project.id
    );
    println!("Progreso: {:.0}%", summary.project.progress);
    println!(
        "Tasks: {}/{} cerradas",
        summary.closed_tasks, summary.total_tasks
    );
    println!("Milestones: {}", summary.milestones.len());
    println!("Vencidas: {}", summary.overdue_tasks.len());
    println!("Próximas 7 días: {}", summary.upcoming_tasks.len());
    let (estimated, spent) = summary
        .times
        .as_array()
        .into_iter()
        .flatten()
        .skip(1)
        .filter_map(|row| row.as_array())
        .fold((0.0, 0.0), |(estimated, spent), row| {
            (
                estimated
                    + row
                        .get(1)
                        .and_then(serde_json::Value::as_f64)
                        .unwrap_or(0.0),
                spent
                    + row
                        .get(2)
                        .and_then(serde_json::Value::as_f64)
                        .unwrap_or(0.0),
            )
        });
    println!(
        "Tiempo: {:.1}h estimadas / {:.1}h reales\n",
        estimated, spent
    );
    Ok(())
}

async fn project_task_tree(args: ProjectArgs, json: bool) -> Result<()> {
    let session = Session::open().await?;
    let tree = operations::projects::task_tree(&session.client, args.project_id).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&tree)?);
        return Ok(());
    }

    if tree.is_empty() {
        println!(
            "No se encontraron tasks en el árbol del proyecto {}.",
            args.project_id
        );
        return Ok(());
    }

    fn print_node(node: &crate::models::ProjectTaskTreeNode, depth: usize) {
        println!("{}#{} {}", "  ".repeat(depth), node.id, node.name);
        for child in &node.children {
            print_node(child, depth + 1);
        }
    }

    println!("\nÁrbol de tasks del proyecto {}:\n", args.project_id);
    for node in &tree {
        print_node(node, 0);
    }
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
            "No se encontraron tasks para el proyecto {}.",
            args.project_id
        );
        return Ok(());
    }

    let mut table = Table::new(&resp.results);
    table.with(Style::rounded());

    println!(
        "\nTasks del proyecto {} ({} total):\n",
        args.project_id, resp.count
    );
    println!("{}", table);
    println!();

    Ok(())
}
