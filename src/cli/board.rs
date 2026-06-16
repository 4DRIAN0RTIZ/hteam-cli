use anyhow::Result;
use clap::{Args, Subcommand};

use crate::config::Config;

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
    let board_id = args.board_id;
    let name = args.name.clone();
    
    config.set_board_number(board_id);
    Config::save_last_ticket(board_id)?;
    
    // Insert or update board info
    let board_name = name.unwrap_or_else(|| {
        config.boards
            .get(&board_id.to_string())
            .map(|b| b.name.clone())
            .unwrap_or_else(|| format!("Board {}", board_id))
    });
    
    config.boards.insert(board_id.to_string(), crate::config::BoardInfo {
        name: board_name.clone(),
        last_used: Some(chrono::Utc::now().to_rfc3339()),
    });
    
    config.save()?;
    
    println!("✅ Board cambiado a: {} ({})", board_name, board_id);
    
    Ok(())
}

async fn show_current() -> Result<()> {
    let config = Config::load()?;
    
    match config.get_board_number() {
        Some(board_id) => {
            let name = config.boards
                .get(&board_id.to_string())
                .map(|b| b.name.clone())
                .unwrap_or_else(|| "Desconocido".to_string());
            println!("📋 Board actual: {} ({})", name, board_id);
        }
        None => {
            println!("⚠️  No hay board configurado. Usa 'hteam board switch <id>'");
        }
    }
    
    Ok(())
}
