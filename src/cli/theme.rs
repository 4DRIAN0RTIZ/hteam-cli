use anyhow::Result;
use clap::{Args, Subcommand};
use colored::Colorize;

use crate::config::Config;
use crate::tui::theme::THEME_NAMES;

#[derive(Subcommand, Debug)]
pub enum ThemeCommands {
    /// Listar los presets de tema disponibles, marcando el activo
    List,
    /// Cambiar el preset de tema activo del TUI
    Set(ThemeSetArgs),
}

#[derive(Args, Debug)]
pub struct ThemeSetArgs {
    /// Nombre del preset: default, solarized, high-contrast
    pub name: String,
}

pub async fn execute(cmd: ThemeCommands, json: bool) -> Result<()> {
    match cmd {
        ThemeCommands::List => list(json),
        ThemeCommands::Set(args) => set(args, json),
    }
}

fn list(json: bool) -> Result<()> {
    let config = Config::load()?;
    let active = config.theme.active.as_str();

    if json {
        let payload = serde_json::json!({
            "presets": THEME_NAMES,
            "active": active,
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(());
    }

    println!("\n🎨 Presets de tema:\n");
    for name in THEME_NAMES {
        if name == active {
            println!("  {} {}", "•".cyan(), name.bold());
        } else {
            println!("    {}", name);
        }
    }
    println!();
    Ok(())
}

fn set(args: ThemeSetArgs, json: bool) -> Result<()> {
    if !THEME_NAMES.contains(&args.name.as_str()) {
        anyhow::bail!(
            "Preset desconocido: '{}'. Opciones válidas: {}",
            args.name,
            THEME_NAMES.join(", ")
        );
    }

    let mut config = Config::load()?;
    config.theme.active = args.name.clone();
    config.save()?;

    if json {
        let payload = serde_json::json!({ "active": args.name });
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(());
    }

    println!(
        "\n✅ Tema activo: {}. Se aplica la próxima vez que abras 'hteam tui'.\n",
        args.name.bold()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_names_are_valid_subcommand_targets() {
        assert!(THEME_NAMES.contains(&"default"));
        assert!(THEME_NAMES.contains(&"solarized"));
        assert!(THEME_NAMES.contains(&"high-contrast"));
    }
}
