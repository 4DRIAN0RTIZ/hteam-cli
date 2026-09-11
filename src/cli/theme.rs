use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use colored::Colorize;

use crate::config::Config;
use crate::tui::theme::{Theme, THEME_NAMES};

#[derive(Subcommand, Debug)]
pub enum ThemeCommands {
    /// Listar los presets embebidos y los temas personalizados, marcando el activo
    List,
    /// Cambiar el tema activo del TUI (preset embebido o personalizado)
    Set(ThemeSetArgs),
}

#[derive(Args, Debug)]
pub struct ThemeSetArgs {
    /// Nombre del preset (default, solarized, high-contrast) o de un tema
    /// personalizado ya creado en ~/.config/hteam/themes/<name>.toml
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
    let custom = Config::custom_theme_names()?;

    if json {
        let payload = serde_json::json!({
            "presets": THEME_NAMES,
            "custom": custom,
            "active": active,
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(());
    }

    println!("\n🎨 Presets embebidos:\n");
    for name in THEME_NAMES {
        if name == active {
            println!("  {} {}", "•".cyan(), name.bold());
        } else {
            println!("    {}", name);
        }
    }

    if custom.is_empty() {
        println!(
            "\nSin temas personalizados. Creá uno en {}/<nombre>.toml\n",
            Config::themes_dir()?.display()
        );
    } else {
        println!("\nPersonalizados:\n");
        for name in &custom {
            if name == active {
                println!("  {} {}", "•".cyan(), name.bold());
            } else {
                println!("    {}", name);
            }
        }
        println!();
    }
    Ok(())
}

fn set(args: ThemeSetArgs, json: bool) -> Result<()> {
    if !THEME_NAMES.contains(&args.name.as_str()) {
        // No es un preset embebido: tiene que ser un tema personalizado que
        // ya parsee — falla acá, no dejar que el usuario se entere recién al
        // abrir el TUI de que tenía un typo o un color inválido.
        Theme::load_custom(&args.name).with_context(|| {
            format!(
                "Preset desconocido: '{}'. Opciones embebidas: {}. Para uno personalizado, creá {}",
                args.name,
                THEME_NAMES.join(", "),
                Config::theme_file(&args.name)
                    .map(|p| p.display().to_string())
                    .unwrap_or_default()
            )
        })?;
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
