use anyhow::Result;
use colored::Colorize;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::process::Command as ProcessCommand;

use crate::cli;
use crate::config::Config;

pub async fn run() -> Result<()> {
    println!("{}", "🚀 Hteam CLI - Modo Interactivo".bold());
    println!(
        "{}",
        "Escribe 'help' para ver los comandos disponibles.".dimmed()
    );
    println!("{}", "Escribe 'exit' o 'quit' para salir.\n".dimmed());

    let mut rl = DefaultEditor::new()?;

    // Load history if available
    let history_path = dirs::home_dir()
        .map(|h| h.join(".hteam_history"))
        .unwrap_or_else(|| std::path::PathBuf::from(".hteam_history"));

    if history_path.exists() {
        let _ = rl.load_history(&history_path);
    }

    let config = Config::load()?;
    if let Some(board) = config.get_board_number() {
        println!("📋 Board activo: {}\n", board);
    }

    loop {
        let readline = rl.readline("hteam> ");

        match readline {
            Ok(line) => {
                let trimmed = line.trim();

                if trimmed.is_empty() {
                    continue;
                }

                rl.add_history_entry(trimmed)?;

                // Check for internal commands first
                if let Err(e) = process_command(trimmed).await {
                    eprintln!("{} {}", "Error:".red().bold(), e);
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("\n{} Usa 'exit' o Ctrl+D para salir", "⚠️".yellow());
                continue;
            }
            Err(ReadlineError::Eof) => {
                println!("\n👋 Adiós!");
                break;
            }
            Err(err) => {
                eprintln!("{} {:?}", "Error:".red().bold(), err);
                break;
            }
        }
    }

    // Save history
    let _ = rl.save_history(&history_path);

    Ok(())
}

async fn process_command(line: &str) -> Result<()> {
    let parts: Vec<&str> = line.split_whitespace().collect();

    if parts.is_empty() {
        return Ok(());
    }

    let cmd = parts[0];
    let args = &parts[1..];

    match cmd {
        "exit" | "quit" | "q" => {
            println!("👋 Adiós!");
            std::process::exit(0);
        }
        "help" | "h" | "?" => {
            print_help();
        }
        "clear" | "cls" => {
            print!("\x1B[2J\x1B[1;1H");
        }
        "board" => {
            handle_board_command(args).await?;
        }
        "lists" | "ls" => {
            cli::cards::lists(None, false).await?;
        }
        "open" | "o" => {
            cli::cards::open_cards(None, false).await?;
        }
        "closed" | "c" => {
            cli::cards::closed_cards(None, false).await?;
        }
        "cards" => {
            handle_cards_command(args).await?;
        }
        "create" | "mk" => {
            handle_create_command(args).await?;
        }
        "move" | "mv" => {
            handle_move_command(args).await?;
        }
        "detail" | "show" => {
            handle_detail_command(args).await?;
        }
        "working" | "w" => {
            handle_working_command(args).await?;
        }
        "comment" | "co" => {
            handle_comment_command(args).await?;
        }
        "config" => {
            handle_config_command(args).await?;
        }
        "!" => {
            // Shell command
            if args.is_empty() {
                println!("⚠️  Uso: ! <comando>");
            } else {
                let shell_cmd = args.join(" ");
                let status = ProcessCommand::new("sh")
                    .arg("-c")
                    .arg(&shell_cmd)
                    .status()?;
                if !status.success() {
                    eprintln!("⚠️  El comando terminó con código: {:?}", status.code());
                }
            }
        }
        _ => {
            println!("{} Comando desconocido: '{}'", "❌".red(), cmd);
            println!("   Escribe 'help' para ver los comandos disponibles.");
        }
    }

    Ok(())
}

fn print_help() {
    println!("{}", "\n📚 Comandos disponibles:\n".bold());

    println!("{}", "Gestión de Boards:".bold());
    println!("  board list              Listar boards configurados");
    println!("  board switch <id>       Cambiar board activo");
    println!("  board current           Mostrar board actual");
    println!("  lists, ls               Listar listas del board");
    println!();

    println!("{}", "Cards:".bold());
    println!("  open, o                 Ver cards abiertos");
    println!("  closed, c               Ver cards cerrados");
    println!("  cards <list_id>         Ver cards de una lista");
    println!("  create, mk <nombre>     Crear un nuevo card");
    println!("  move, mv <id> --to <n>  Mover card a otra lista");
    println!("  detail, show <id>       Ver detalle de un card");
    println!("  comment, co <id> <txt>  Agregar comentario");
    println!();

    println!("{}", "Working On It:".bold());
    println!("  working, w list         Ver en qué estás trabajando");
    println!("  working start <id>      Empezar a trabajar en un card");
    println!("  working stop <id>       Dejar de trabajar");
    println!();

    println!("{}", "Configuración:".bold());
    println!("  config show             Mostrar configuración");
    println!();

    println!("{}", "Utilidades:".bold());
    println!("  ! <comando>             Ejecutar comando shell");
    println!("  clear, cls              Limpiar pantalla");
    println!("  help, h, ?              Mostrar esta ayuda");
    println!("  exit, quit, q           Salir del programa");
    println!();
}

async fn handle_board_command(args: &[&str]) -> Result<()> {
    use crate::cli::board::{BoardCommands, BoardSwitchArgs};

    if args.is_empty() {
        // Show current board
        cli::board::execute(BoardCommands::Current, false).await?;
        return Ok(());
    }

    match args[0] {
        "list" | "ls" => {
            cli::board::execute(BoardCommands::List, false).await?;
        }
        "switch" | "sw" => {
            if args.len() < 2 {
                println!("⚠️  Uso: board switch <board_id>");
                return Ok(());
            }
            let board_id = args[1].parse::<u64>()?;
            let name = args.get(2).map(|s| s.to_string());
            cli::board::execute(
                BoardCommands::Switch(BoardSwitchArgs { board_id, name }),
                false,
            )
            .await?;
        }
        "current" => {
            cli::board::execute(BoardCommands::Current, false).await?;
        }
        _ => {
            println!("⚠️  Subcomando desconocido: {}", args[0]);
        }
    }

    Ok(())
}

async fn handle_cards_command(args: &[&str]) -> Result<()> {
    use crate::cli::cards::{CardsCommands, ListCardsArgs};

    if args.is_empty() {
        println!("⚠️  Uso: cards <list_id>");
        return Ok(());
    }

    let list_id = args[0].parse::<u64>()?;
    cli::cards::execute(CardsCommands::List(ListCardsArgs { list_id }), None, false).await?;
    Ok(())
}

async fn handle_create_command(args: &[&str]) -> Result<()> {
    use crate::cli::cards::{CardsCommands, CreateArgs};

    if args.is_empty() {
        println!("⚠️  Uso: create <nombre> [--complement <id>] [--process <id>]");
        return Ok(());
    }

    let name = args.join(" ");
    cli::cards::execute(
        CardsCommands::Create(CreateArgs {
            name,
            list_id: None,
        }),
        None,
        false,
    )
    .await?;
    Ok(())
}

async fn handle_move_command(args: &[&str]) -> Result<()> {
    use crate::cli::cards::{CardsCommands, MoveArgs};

    if args.len() < 2 {
        println!("⚠️  Uso: move <card_id> --to <list_id>");
        return Ok(());
    }

    let card_id = args[0].parse::<u64>()?;
    let mut to_list = None;
    let mut from_list = None;

    let mut i = 1;
    while i < args.len() {
        match args[i] {
            "--to" | "-t" => {
                if i + 1 < args.len() {
                    to_list = Some(args[i + 1].parse::<u64>()?);
                    i += 2;
                } else {
                    println!("⚠️  Falta valor para --to");
                    return Ok(());
                }
            }
            "--from" | "-f" => {
                if i + 1 < args.len() {
                    from_list = Some(args[i + 1].parse::<u64>()?);
                    i += 2;
                } else {
                    println!("⚠️  Falta valor para --from");
                    return Ok(());
                }
            }
            _ => i += 1,
        }
    }

    let to = to_list.ok_or_else(|| anyhow::anyhow!("Se requiere --to <list_id>"))?;

    cli::cards::execute(
        CardsCommands::Move(MoveArgs {
            card_id,
            to,
            from: from_list,
        }),
        None,
        false,
    )
    .await?;
    Ok(())
}

async fn handle_detail_command(args: &[&str]) -> Result<()> {
    use crate::cli::cards::{CardsCommands, DetailArgs};

    if args.is_empty() {
        println!("⚠️  Uso: detail <card_id>");
        return Ok(());
    }

    let card_id = args[0].parse::<u64>()?;
    cli::cards::execute(CardsCommands::Detail(DetailArgs { card_id }), None, false).await?;
    Ok(())
}

async fn handle_working_command(args: &[&str]) -> Result<()> {
    use crate::cli::working::{WorkingCommands, WorkingStartArgs, WorkingStopArgs};

    if args.is_empty() {
        // List working on
        cli::working::execute(WorkingCommands::List, false).await?;
        return Ok(());
    }

    match args[0] {
        "list" | "ls" => {
            cli::working::execute(WorkingCommands::List, false).await?;
        }
        "start" | "s" => {
            if args.len() < 2 {
                println!("⚠️  Uso: working start <card_id>");
                return Ok(());
            }
            let card_id = args[1].parse::<u64>()?;
            cli::working::execute(WorkingCommands::Start(WorkingStartArgs { card_id }), false)
                .await?;
        }
        "stop" => {
            if args.len() < 2 {
                println!("⚠️  Uso: working stop <working_id>");
                return Ok(());
            }
            let working_id = args[1].parse::<u64>()?;
            cli::working::execute(WorkingCommands::Stop(WorkingStopArgs { working_id }), false)
                .await?;
        }
        _ => {
            println!("⚠️  Subcomando desconocido: {}", args[0]);
        }
    }

    Ok(())
}

async fn handle_comment_command(args: &[&str]) -> Result<()> {
    use crate::cli::cards::{CardsCommands, CommentArgs};

    const USAGE: &str = "⚠️  Uso: comment <card_id> [--follow] [--date YYYY-MM-DD HH:MM] <texto>";

    if args.len() < 2 {
        println!("{}", USAGE);
        return Ok(());
    }

    let card_id = args[0].parse::<u64>()?;
    let mut rest = &args[1..];
    let mut follow = false;
    let mut date: Option<String> = None;

    loop {
        match rest.first() {
            Some(&"--follow") => {
                follow = true;
                rest = &rest[1..];
            }
            Some(&"--date") => {
                if rest.len() < 3 {
                    println!("⚠️  --date requiere fecha y hora: --date YYYY-MM-DD HH:MM");
                    return Ok(());
                }
                date = Some(format!("{} {}", rest[1], rest[2]));
                rest = &rest[3..];
            }
            _ => break,
        }
    }

    if rest.is_empty() {
        println!("{}", USAGE);
        return Ok(());
    }
    let text = rest.join(" ");

    cli::cards::execute(
        CardsCommands::Comment(CommentArgs {
            card_id,
            text,
            board: None,
            follow,
            date,
        }),
        None,
        false,
    )
    .await?;
    Ok(())
}

async fn handle_config_command(args: &[&str]) -> Result<()> {
    if args.is_empty() {
        println!("⚠️  Uso: config show");
        return Ok(());
    }

    match args[0] {
        "show" => {
            let config = Config::load()?;
            println!("\n{}", "Configuración:".bold());

            if config.is_authenticated() {
                println!("  Estado: ✅ Autenticado");
                if let Some(board) = config.get_board_number() {
                    println!("  Board activo: {}", board);
                }
            } else {
                println!("  Estado: ❌ No autenticado");
            }

            if !config.boards.is_empty() {
                println!("\n  Boards:");
                for (id, info) in &config.boards {
                    println!("    {} - {}", id, info.name);
                }
            }
            println!();
        }
        _ => {
            println!("⚠️  Subcomando desconocido: {}", args[0]);
        }
    }

    Ok(())
}
