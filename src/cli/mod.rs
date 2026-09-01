use anyhow::Result;
use colored::Colorize;
use std::io;

use crate::{client, operations};

pub mod board;
pub mod cards;
pub mod interactive;
pub mod login;
pub mod objectives;
pub mod project;
pub mod theme;
pub mod update;
pub mod working;

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};

#[derive(Parser, Debug)]
#[command(name = "hteam")]
#[command(about = "CLI para gestionar boards de Hteam")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Mostrar salida en formato JSON
    #[arg(long, global = true)]
    pub json: bool,

    /// ID del board a usar
    #[arg(short, long, global = true)]
    pub board: Option<u64>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Iniciar sesión con cookies
    Login(login::LoginArgs),

    /// Gestionar boards
    #[command(subcommand)]
    Board(board::BoardCommands),

    /// Listar listas del board
    Lists,

    /// Operaciones con cards
    #[command(subcommand)]
    Cards(cards::CardsCommands),

    /// Ver cards abiertos
    Open,

    /// Ver cards cerrados
    Closed,

    /// Gestionar working on it
    #[command(subcommand)]
    Working(working::WorkingCommands),

    /// Gestionar proyectos
    #[command(subcommand)]
    Project(project::ProjectCommands),

    /// Objetivos semanales (scraping de SharePad, sólo lectura)
    #[command(subcommand)]
    Objectives(objectives::ObjectivesCommands),

    /// Registrar entrada del día (check in)
    Checkin,

    /// Ver recordatorios pendientes
    Reminders,

    /// Ver historial de trabajo diario del equipo
    DailyWork(DailyWorkArgs),

    /// Buscar usuarios por nombre
    Users(UsersArgs),

    /// Modo interactivo REPL
    Interactive,

    /// Board visual tipo kanban (TUI)
    Tui,

    /// Gestionar el tema de color del TUI
    #[command(subcommand)]
    Theme(theme::ThemeCommands),

    /// Iniciar servidor MCP (Model Context Protocol) sobre stdio
    Mcp,

    /// Descargar e instalar la última versión publicada de hteam-cli
    Update,

    /// Generar script de autocompletado para la shell indicada
    Completions {
        /// Shell destino: bash, zsh, fish, elvish, powershell
        shell: Shell,
    },
}

#[derive(clap::Args, Debug)]
pub struct UsersArgs {
    /// Texto a buscar
    pub query: String,
}

#[derive(clap::Args, Debug)]
pub struct DailyWorkArgs {
    /// Filtrar por usuario (username)
    #[arg(short, long)]
    pub user: Option<String>,

    /// Tipo de actividad: time, tracking, objectives
    #[arg(short = 't', long = "type")]
    pub activity_type: Option<String>,

    /// Rango de tiempo: today, this_week, this_month
    #[arg(short, long, default_value = "today")]
    pub range: String,
}

pub async fn run() -> Result<()> {
    let cli = Cli::parse();

    // Mcp habla JSON-RPC por stdout y Completions/Tui no se benefician de un
    // aviso que se imprime después de que el comando ya terminó (Mcp/
    // Interactive no terminan nunca en un uso normal, Tui reemplaza la
    // pantalla; Tui tiene su propio aviso vía `app.set_status`, ver
    // `tui::run`). Update también se saltea: el proceso en ejecución sigue
    // siendo el binario viejo (reemplazar el archivo en disco no afecta el
    // proceso ya cargado en memoria), así que mostrar "hay una versión
    // nueva, corré hteam update" justo después de haber corrido `hteam
    // update` con éxito sería confuso.
    let skip_update_notice = matches!(
        &cli.command,
        Commands::Mcp
            | Commands::Completions { .. }
            | Commands::Tui
            | Commands::Interactive
            | Commands::Update
    );

    let result = match cli.command {
        Commands::Login(args) => login::execute(args).await,
        Commands::Board(cmd) => board::execute(cmd, cli.json).await,
        Commands::Lists => cards::lists(cli.board, cli.json).await,
        Commands::Cards(cmd) => cards::execute(cmd, cli.board, cli.json).await,
        Commands::Open => cards::open_cards(cli.board, cli.json).await,
        Commands::Closed => cards::closed_cards(cli.board, cli.json).await,
        Commands::Working(cmd) => working::execute(cmd, cli.json).await,
        Commands::Project(cmd) => project::execute(cmd, cli.json).await,
        Commands::Objectives(cmd) => objectives::execute(cmd, cli.json).await,
        Commands::Checkin => checkin(cli.json).await,
        Commands::Reminders => reminders(cli.json).await,
        Commands::DailyWork(args) => daily_work(args, cli.json).await,
        Commands::Users(args) => search_users(args, cli.json).await,
        Commands::Interactive => interactive::run().await,
        Commands::Tui => crate::tui::run(cli.board).await,
        Commands::Theme(cmd) => theme::execute(cmd, cli.json).await,
        Commands::Mcp => crate::mcp::run().await,
        Commands::Update => update::execute(cli.json).await,
        Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            let name = cmd.get_name().to_string();
            generate(shell, &mut cmd, name, &mut io::stdout());
            Ok(())
        }
    };

    if !skip_update_notice && result.is_ok() {
        maybe_notify_update().await;
    }

    result
}

/// Aviso post-comando, no bloqueante para la salida del comando en sí (se
/// imprime a stderr después de que el resultado ya se calculó): (1) si el
/// binario cambió de versión desde la última corrida, muestra una única vez
/// el changelog de las versiones nuevas; (2) chequea (con cache TTL, ver
/// `UpdateConfig::is_check_fresh`) si hay una versión más nueva publicada en
/// GitHub y sugiere `hteam update`. Nunca escribe a stdout — así no rompe
/// `--json` — y cualquier error (config corrupta, sin red, rate limit) se
/// ignora en silencio para no romper el comando que el usuario sí pidió.
async fn maybe_notify_update() {
    use crate::config::Config;

    let Ok(mut config) = Config::load() else {
        return;
    };

    let mut dirty = false;

    if let Some(changelog) = operations::update::resolve_startup_changelog(
        &mut config,
        operations::update::EMBEDDED_CHANGELOG,
    ) {
        eprintln!(
            "\n{}\n{}",
            "📦 hteam se actualizó — novedades:".bold(),
            changelog.trim_end()
        );
        dirty = true;
    }

    if let Ok(result) =
        operations::update::check_for_update(&mut config, client::update::GITHUB_API_BASE).await
    {
        dirty = dirty || !result.from_cache;

        if result.update_available {
            eprintln!(
                "{}",
                format!(
                    "⬆️  Nueva versión disponible: {} (actual: {}). Corré `hteam update`.",
                    result.latest_version, result.current_version
                )
                .yellow()
            );
        }
    }

    if dirty {
        let _ = config.save();
    }
}

async fn checkin(json: bool) -> Result<()> {
    use crate::operations::{self, Session};

    let session = Session::open().await?;
    let result = operations::checkin::check_in(&session.client).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    println!("\n✅ Check in registrado.");
    if let Some(time) = &result.check_in {
        println!("   Hora: {}", time);
    }
    println!();
    Ok(())
}

async fn reminders(json: bool) -> Result<()> {
    use crate::operations::{self, Session};
    use tabled::{settings::Style, Table};

    let session = Session::open().await?;
    let items = operations::reminders::list(&session.client).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&items)?);
        return Ok(());
    }

    if items.is_empty() {
        println!("⚠️  No tienes recordatorios pendientes.");
        return Ok(());
    }

    let mut table = Table::new(&items);
    table.with(Style::rounded());
    println!("\n🔔 Recordatorios:\n");
    println!("{}", table);
    println!();
    Ok(())
}

async fn daily_work(args: DailyWorkArgs, json: bool) -> Result<()> {
    use crate::operations::{self, Session};
    use tabled::{settings::Style, Table};

    let session = Session::open().await?;
    let entries = operations::daily_work::history(
        &session.client,
        args.user.as_deref(),
        args.activity_type.as_deref(),
        Some(&args.range),
    )
    .await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&entries)?);
        return Ok(());
    }

    if entries.is_empty() {
        println!("⚠️  No hay actividad para mostrar.");
        return Ok(());
    }

    let mut table = Table::new(&entries);
    table.with(Style::rounded());
    println!("\n📅 Historial de trabajo diario:\n");
    println!("{}", table);
    println!();
    Ok(())
}

async fn search_users(args: UsersArgs, json: bool) -> Result<()> {
    use crate::operations::{self, Session};
    use tabled::{settings::Style, Table};

    let session = Session::open().await?;
    let users = operations::users::search(&session.client, &args.query).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&users)?);
        return Ok(());
    }

    if users.is_empty() {
        println!("⚠️  No se encontraron usuarios para '{}'.", args.query);
        return Ok(());
    }

    let mut table = Table::new(&users);
    table.with(Style::rounded());
    println!("\n👤 Usuarios:\n");
    println!("{}", table);
    println!();
    Ok(())
}
