use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuthConfig {
    pub session_id: Option<String>,
    pub csrf_token: Option<String>,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub board_number: Option<u64>,
    pub board_id: Option<u64>,
    pub user_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BoardInfo {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_used: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TuiConfig {
    /// List names (case-insensitive) hidden from the `hteam tui` board. Toggled
    /// with 'v'/'V' inside the TUI; empty means every list is shown.
    #[serde(default)]
    pub hidden_lists: Vec<String>,
    /// Project ids looked up from the 'P' popup, most-recently-used first, so
    /// you can pick one again without retyping it.
    #[serde(default)]
    pub known_projects: Vec<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub auth: AuthConfig,
    #[serde(default)]
    pub boards: HashMap<String, BoardInfo>,
    #[serde(default)]
    pub variables: HashMap<String, String>,
    #[serde(default)]
    pub tui: TuiConfig,
}

impl Config {
    pub fn config_dir() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("No se pudo encontrar el directorio de configuración")?
            .join("hteam");
        Ok(config_dir)
    }

    pub fn config_file() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("config.toml"))
    }

    pub fn load() -> Result<Self> {
        let config_file = Self::config_file()?;

        if !config_file.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&config_file)
            .with_context(|| format!("No se pudo leer {}", config_file.display()))?;

        let config: Config = toml::from_str(&content).context("Error al parsear config.toml")?;

        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let config_dir = Self::config_dir()?;
        let config_file = Self::config_file()?;

        if !config_dir.exists() {
            fs::create_dir_all(&config_dir).with_context(|| {
                format!("No se pudo crear el directorio {}", config_dir.display())
            })?;
        }

        let content = toml::to_string_pretty(self).context("Error al serializar configuración")?;

        fs::write(&config_file, content)
            .with_context(|| format!("No se pudo escribir {}", config_file.display()))?;

        Ok(())
    }

    pub fn is_authenticated(&self) -> bool {
        self.auth.session_id.is_some() && self.auth.csrf_token.is_some()
    }

    pub fn get_board_number(&self) -> Option<u64> {
        self.auth.board_number
    }

    pub fn set_board_number(&mut self, board_number: u64) {
        self.auth.board_number = Some(board_number);
    }

    pub fn get_variable(&self, name: &str) -> Option<&String> {
        self.variables.get(name)
    }

    pub fn set_variable(&mut self, name: String, value: String) {
        self.variables.insert(name, value);
    }

    pub fn get_last_ticket_path() -> Result<PathBuf> {
        let home = dirs::home_dir().context("No se pudo encontrar el directorio home")?;
        Ok(home.join(".last_ticket"))
    }

    pub fn load_last_ticket(&self) -> Result<Option<u64>> {
        let path = Self::get_last_ticket_path()?;
        if !path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(&path)?;
        let board_number = content.trim().parse::<u64>()?;
        Ok(Some(board_number))
    }

    pub fn save_last_ticket(board_number: u64) -> Result<()> {
        let path = Self::get_last_ticket_path()?;
        fs::write(&path, board_number.to_string())?;
        Ok(())
    }
}
