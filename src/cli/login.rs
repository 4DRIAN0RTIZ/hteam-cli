use anyhow::{bail, Result};
use clap::Args;
use inquire::Text;

use crate::client::HteamClient;
use crate::config::Config;
use crate::operations::session::{
    check_auth, persist_login, resolve_login_board, AuthCheck, LoginBoardResolution,
};

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
        None => Text::new("CSRF Token:")
            .with_help_message("Pega el valor de la cookie csrftoken")
            .prompt()?,
    };

    // Keep disk config untouched until the session probe succeeds.
    let mut config = Config::load().unwrap_or_default();
    let configured_board = config.get_board_number();

    let mut probe_config = config.clone();
    probe_config.auth.session_id = Some(session_id.clone());
    probe_config.auth.csrf_token = Some(csrf_token.clone());

    println!("\n🔄 Verificando credenciales...");
    let client = HteamClient::new(probe_config)?;

    let boards = match check_auth(&client).await {
        Ok(AuthCheck::Ok { boards }) => boards,
        Ok(AuthCheck::Unauthorized) => {
            bail!("❌ Sesión inválida (unauthorized). Verifica sessionid y csrftoken.");
        }
        Err(e) => {
            bail!("❌ Error de red al verificar autenticación: {e}");
        }
    };

    let resolution = resolve_login_board(&boards, args.board, configured_board);
    let board_to_persist = match &resolution {
        LoginBoardResolution::Selected { id, name } => Some((*id, name.clone())),
        LoginBoardResolution::NotFound { .. }
        | LoginBoardResolution::NeedsSwitch { .. }
        | LoginBoardResolution::NoneAvailable => None,
    };

    persist_login(&mut config, session_id, csrf_token, board_to_persist)?;

    println!("✅ Autenticación exitosa!");

    match resolution {
        LoginBoardResolution::Selected { id, name } => {
            println!("📋 Board activo: {name} ({id})");
        }
        LoginBoardResolution::NeedsSwitch { boards } => {
            println!(
                "📋 Hay varios boards disponibles. Elige uno con `hteam board switch <id>`:\n"
            );
            for (id, name) in boards {
                println!("   {id} - {name}");
            }
        }
        LoginBoardResolution::NoneAvailable => {
            println!("⚠️  La sesión es válida pero no hay boards en Execution visibles.");
        }
        LoginBoardResolution::NotFound { requested } => {
            bail!(
                "❌ Sesión guardada, pero el board {requested} no está en tu listado. \
                 Usa `hteam board switch <id>` con un board visible."
            );
        }
    }

    Ok(())
}
