use anyhow::{Context, Result};

/// Copia texto al portapapeles del sistema. Único punto donde el CLI y el
/// TUI tocan `arboard`, para no duplicar la inicialización del clipboard.
pub fn copy(text: &str) -> Result<()> {
    let mut clipboard = arboard::Clipboard::new().context("No se pudo acceder al portapapeles")?;
    clipboard
        .set_text(text.to_string())
        .context("No se pudo copiar al portapapeles")
}
