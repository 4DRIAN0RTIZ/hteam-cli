use anyhow::Result;
use clap::{Args, Subcommand};

use crate::config::Config;
use crate::operations;

#[derive(Subcommand, Debug)]
pub enum BoardCommands {
    /// Listar boards configurados
    List,
    /// Cambiar board activo
    Switch(BoardSwitchArgs),
    /// Mostrar board actual
    Current,
}

#[derive(Args, Debug)]
pub struct BoardSwitchArgs {
    /// ID del board
    pub board_id: u64,

    /// Nombre del board (opcional)
    #[arg(short, long)]
    pub name: Option<String>,
}

pub async fn execute(cmd: BoardCommands, json: bool) -> Result<()> {
    match cmd {
        BoardCommands::List => list_boards(json).await,
        BoardCommands::Switch(args) => switch_board(args).await,
        BoardCommands::Current => show_current().await,
    }
}

async fn list_boards(json: bool) -> Result<()> {
    let config = Config::load()?;

    if json {
        println!("{}", serde_json::to_string_pretty(&config.boards)?);
        return Ok(());
    }

    println!("📋 Boards configurados:\n");

    if config.boards.is_empty() {
        println!("   No hay boards configurados.");
        println!("   Usa: hteam board switch <board_id> --name <nombre>");
        return Ok(());
    }

    for (id, info) in &config.boards {
        let current = if Some(id.parse::<u64>().unwrap_or(0)) == config.get_board_number() {
            " (actual)"
        } else {
            ""
        };
        println!("   {} - {}{}", id, info.name, current);
    }

    Ok(())
}

async fn switch_board(args: BoardSwitchArgs) -> Result<()> {
    let mut config = Config::load()?;
    let board_name = operations::boards::switch_board(&mut config, args.board_id, args.name)?;

    println!("✅ Board cambiado a: {} ({})", board_name, args.board_id);

    Ok(())
}

async fn show_current() -> Result<()> {
    let config = Config::load()?;

    match operations::boards::current_board(&config) {
        Some((board_id, name)) => {
            println!("📋 Board actual: {} ({})", name, board_id);
        }
        None => {
            println!("⚠️  No hay board configurado. Usa 'hteam board switch <id>'");
        }
    }

    Ok(())
}
