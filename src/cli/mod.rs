use anyhow::Result;
use serde_json;

pub mod board;
pub mod cards;
pub mod interactive;
pub mod login;
pub mod project;
pub mod working;

use clap::{Parser, Subcommand};

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

    /// Registrar entrada del día (check in)
    Checkin,

    /// Ver recordatorios pendientes
    Reminders,

    /// Buscar usuarios por nombre
    Users(UsersArgs),

    /// Modo interactivo REPL
    Interactive,

    /// Iniciar servidor MCP (Model Context Protocol) sobre stdio
    Mcp,
}

#[derive(clap::Args, Debug)]
pub struct UsersArgs {
    /// Texto a buscar
    pub query: String,
}

pub async fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Login(args) => login::execute(args).await,
        Commands::Board(cmd) => board::execute(cmd, cli.json).await,
        Commands::Lists => cards::lists(cli.board, cli.json).await,
        Commands::Cards(cmd) => cards::execute(cmd, cli.board, cli.json).await,
        Commands::Open => cards::open_cards(cli.board, cli.json).await,
        Commands::Closed => cards::closed_cards(cli.board, cli.json).await,
        Commands::Working(cmd) => working::execute(cmd, cli.json).await,
        Commands::Project(cmd) => project::execute(cmd, cli.json).await,
        Commands::Checkin => checkin(cli.json).await,
        Commands::Reminders => reminders(cli.json).await,
        Commands::Users(args) => search_users(args, cli.json).await,
        Commands::Interactive => interactive::run().await,
        Commands::Mcp => crate::mcp::run().await,
    }
}

async fn checkin(json: bool) -> Result<()> {
    use crate::client::HteamClient;
    use crate::config::Config;

    let config = Config::load()?;
    let client = HteamClient::with_auth(config).await?;
    let result = client.check_in().await?;

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
    use crate::client::HteamClient;
    use crate::config::Config;
    use tabled::{Table, settings::Style};

    let config = Config::load()?;
    let client = HteamClient::with_auth(config).await?;
    let items = client.get_reminders().await?;

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

async fn search_users(args: UsersArgs, json: bool) -> Result<()> {
    use crate::client::HteamClient;
    use crate::config::Config;
    use tabled::{Table, settings::Style};

    let config = Config::load()?;
    let client = HteamClient::with_auth(config).await?;
    let users = client.search_users(&args.query).await?;

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
