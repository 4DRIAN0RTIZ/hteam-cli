use anyhow::Result;
use clap::Args;
use inquire::{Password, Text};

use crate::config::Config;
use crate::client::HteamClient;

#[derive(Args, Debug)]
pub struct LoginArgs {
    /// Session ID (cookie)
    #[arg(short, long)]
    pub session_id: Option<String>,

    /// CSRF Token (cookie)
    #[arg(short, long)]
    pub csrf_token: Option<String>,

    /// Board number por defecto
    #[arg(short, long)]
    pub board: Option<u64>,
}

pub async fn execute(args: LoginArgs) -> Result<()> {
    println!("🔐 Iniciando sesión en Hteam...\n");

    let session_id = match args.session_id {
        Some(s) => s,
        None => Text::new("Session ID:")
            .with_help_message("Pega el valor de la cookie sessionid")
            .prompt()?,
    };

    let csrf_token = match args.csrf_token {
        Some(s) => s,
        None => Password::new("CSRF Token:")
            .with_help_message("Pega el valor de la cookie csrftoken")
            .without_confirmation()
            .prompt()?, // Using Password but the value will be visible in the prompt
    };

    let mut config = Config::load()?;
    config.auth.session_id = Some(session_id.clone());
    config.auth.csrf_token = Some(csrf_token.clone());

    if let Some(board) = args.board {
        config.set_board_number(board);
        Config::save_last_ticket(board)?;
        
        config.boards.entry(board.to_string()).or_insert_with(|| crate::config::BoardInfo {
            name: format!("Board {}", board),
            last_used: Some(chrono::Utc::now().to_rfc3339()),
        });
    }

    // Test authentication
    println!("\n🔄 Verificando credenciales...");
    let client = HteamClient::new(config.clone())?;
    
    match client.test_auth().await {
        Ok(true) => {
            config.save()?;
            println!("✅ Autenticación exitosa!");
            
            if let Some(board) = config.get_board_number() {
                println!("📋 Board activo: {}", board);
            }
        }
        Ok(false) => {
            anyhow::bail!("❌ Autenticación fallida. Verifica tus cookies.");
        }
        Err(e) => {
            anyhow::bail!("❌ Error al verificar autenticación: {}", e);
        }
    }

    Ok(())
}
