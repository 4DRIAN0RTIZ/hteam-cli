use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::{Args, Subcommand};
use colored::Colorize;
use serde::Serialize;

use crate::config::Config;
use crate::operations;

#[derive(Subcommand, Debug)]
pub enum ObjectivesCommands {
    /// Mostrar los objetivos semanales del notebook de SharePad
    Show(ObjectivesShowArgs),
}

#[derive(Args, Debug)]
pub struct ObjectivesShowArgs {
    /// Ignorar el cache local y volver a scrapear SharePad
    #[arg(long)]
    pub force: bool,

    /// Mostrar además el origen del resultado (cache/red) y hace cuánto se sincronizó
    #[arg(short, long)]
    pub verbose: bool,
}

/// Forma del `--json` de este comando: el set de objetivos más metadata de
/// sincronización, siempre presente (a diferencia de la salida de texto, que
/// sólo la muestra con `--verbose`) porque `--json` ya es la superficie
/// "detallada" para scripting.
#[derive(Serialize)]
struct ObjectivesShowJson<'a> {
    #[serde(flatten)]
    set: &'a crate::models::WeeklyObjectivesSet,
    from_cache: bool,
    synced_at: DateTime<Utc>,
}

pub async fn execute(cmd: ObjectivesCommands, json: bool) -> Result<()> {
    match cmd {
        ObjectivesCommands::Show(args) => show(args, json).await,
    }
}

/// `hteam objectives show [--force]` — comando de solo lectura: nunca hace
/// login contra hteam.mx (no usa `Session::open()`), sólo lee/escribe el
/// cache local en `config.toml` y hace `GET` al notebook público de
/// SharePad configurado en `[weekly_objectives] source_url`.
async fn show(args: ObjectivesShowArgs, json: bool) -> Result<()> {
    let mut config = Config::load()?;

    let view = operations::objectives::show(&mut config, args.force).await?;

    // El cache en memoria sólo se persiste a disco cuando hubo un fetch
    // exitoso — si vino de cache, config.toml ya tiene esos mismos datos.
    if !view.from_cache {
        config.save()?;
    }

    if json {
        let payload = ObjectivesShowJson {
            set: &view.set,
            from_cache: view.from_cache,
            synced_at: view.synced_at,
        };
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(());
    }

    println!("\n🎯 {}\n", view.set.title.bold());

    for objective in &view.set.objectives {
        println!("## {}", objective.name.bold());
        if objective.tasks.is_empty() {
            println!("   (sin tareas)");
        }
        for task in &objective.tasks {
            let mark = if task.done { "x".green() } else { " ".normal() };
            println!("   [{}] {}", mark, task.description);
        }
        println!();
    }

    if args.verbose {
        let source = if view.from_cache { "cache" } else { "red" };
        println!(
            "{}",
            format!("{}, {}", source, relative_time(view.synced_at)).dimmed()
        );
    }

    Ok(())
}

/// Texto tipo "hace 2h 15m" / "hace 40m" / "hace 12s" a partir de un
/// timestamp pasado. Decisión de presentación, por eso vive en `cli` y no en
/// `operations` (que sólo expone `synced_at`).
fn relative_time(synced_at: chrono::DateTime<Utc>) -> String {
    let elapsed = Utc::now().signed_duration_since(synced_at);

    if elapsed.num_hours() > 0 {
        format!(
            "sincronizado hace {}h {}m",
            elapsed.num_hours(),
            elapsed.num_minutes() % 60
        )
    } else if elapsed.num_minutes() > 0 {
        format!("sincronizado hace {}m", elapsed.num_minutes())
    } else {
        format!("sincronizado hace {}s", elapsed.num_seconds().max(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn relative_time_formats_hours_and_minutes() {
        let synced_at = Utc::now() - Duration::hours(2) - Duration::minutes(15);

        assert_eq!(relative_time(synced_at), "sincronizado hace 2h 15m");
    }

    #[test]
    fn relative_time_formats_minutes_only() {
        let synced_at = Utc::now() - Duration::minutes(40);

        assert_eq!(relative_time(synced_at), "sincronizado hace 40m");
    }

    #[test]
    fn relative_time_formats_seconds_for_recent_sync() {
        let synced_at = Utc::now() - Duration::seconds(5);

        assert_eq!(relative_time(synced_at), "sincronizado hace 5s");
    }
}
