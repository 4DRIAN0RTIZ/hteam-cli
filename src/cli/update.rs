use anyhow::Result;
use colored::Colorize;
use serde::Serialize;

use crate::client;
use crate::operations;

#[derive(Serialize)]
struct UpdateJson {
    version: String,
    binary_path: String,
    updated: bool,
}

/// `hteam update` — descarga el asset `hteam-linux-x86_64` de la última
/// release de GitHub y reemplaza el binario en ejecución. Si ya se está en
/// la última versión conocida, no descarga nada (ver
/// `operations::update::perform_self_update`). Falla con un error claro
/// (propagado por esa misma función) si la plataforma actual no es
/// linux-x86_64 o si la descarga/reemplazo fallan.
pub async fn execute(json: bool) -> Result<()> {
    let outcome = operations::update::perform_self_update(client::update::GITHUB_API_BASE).await?;

    if json {
        let payload = UpdateJson {
            version: outcome.version.clone(),
            binary_path: outcome.binary_path.display().to_string(),
            updated: outcome.updated,
        };
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(());
    }

    if !outcome.updated {
        println!(
            "\n{} Ya estás en la última versión ({})\n",
            "✅".green(),
            outcome.version.bold()
        );
        return Ok(());
    }

    println!(
        "\n{} hteam actualizado a la versión {} ({})\n",
        "✅".green(),
        outcome.version.bold(),
        outcome.binary_path.display()
    );
    Ok(())
}
