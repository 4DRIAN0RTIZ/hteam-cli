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
pub struct WorkingHoursConfig {
    /// Hora de inicio de la jornada, formato "HH:MM" (24h).
    #[serde(default)]
    pub start: Option<String>,
    /// Hora de fin de la jornada, formato "HH:MM" (24h).
    #[serde(default)]
    pub end: Option<String>,
}

impl WorkingHoursConfig {
    /// Tiempo restante hasta `end`, formateado "Xh YYm restantes" — `None` si
    /// no está configurada o `now` cae fuera del rango [start, end].
    pub fn remaining_display(&self, now: chrono::NaiveTime) -> Option<String> {
        let start = self.start.as_deref().and_then(parse_time)?;
        let end = self.end.as_deref().and_then(parse_time)?;
        if now < start || now > end {
            return None;
        }
        let remaining = end - now;
        Some(format!(
            " {}h {:02}m restantes ",
            remaining.num_hours(),
            remaining.num_minutes() % 60
        ))
    }
}

fn parse_time(s: &str) -> Option<chrono::NaiveTime> {
    chrono::NaiveTime::parse_from_str(s, "%H:%M").ok()
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
    #[serde(default)]
    pub working_hours: WorkingHoursConfig,
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

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "variables are part of the persisted config schema"
        )
    )]
    pub fn get_variable(&self, name: &str) -> Option<&String> {
        self.variables.get(name)
    }

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "variables are part of the persisted config schema"
        )
    )]
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveTime;

    #[test]
    fn config_default_has_no_auth_and_empty_collections() {
        let config = Config::default();

        assert!(!config.is_authenticated());
        assert!(config.boards.is_empty());
        assert!(config.variables.is_empty());
        assert!(config.tui.hidden_lists.is_empty());
    }

    #[test]
    fn is_authenticated_requires_session_id_and_csrf_token() {
        let mut config = Config::default();
        assert!(!config.is_authenticated());

        config.auth.session_id = Some("sid".to_string());
        assert!(!config.is_authenticated());

        config.auth.csrf_token = Some("csrf".to_string());
        assert!(config.is_authenticated());
    }

    #[test]
    fn board_number_getter_and_setter_round_trip() {
        let mut config = Config::default();
        assert_eq!(config.get_board_number(), None);

        config.set_board_number(42);

        assert_eq!(config.get_board_number(), Some(42));
    }

    #[test]
    fn variable_getter_and_setter_round_trip() {
        let mut config = Config::default();
        assert_eq!(config.get_variable("env"), None);

        config.set_variable("env".to_string(), "prod".to_string());

        assert_eq!(config.get_variable("env"), Some(&"prod".to_string()));
    }

    #[test]
    fn config_round_trips_through_toml() {
        let mut original = Config::default();
        original.auth.session_id = Some("sid".to_string());
        original.auth.csrf_token = Some("csrf".to_string());
        original.auth.board_number = Some(7);
        original
            .variables
            .insert("env".to_string(), "prod".to_string());
        original.tui.hidden_lists = vec!["Done".to_string()];
        original.working_hours.start = Some("09:00".to_string());
        original.working_hours.end = Some("18:00".to_string());

        let serialized = toml::to_string_pretty(&original).expect("serializes to TOML");
        let parsed: Config = toml::from_str(&serialized).expect("parses back from TOML");

        assert_eq!(parsed.auth.session_id, original.auth.session_id);
        assert_eq!(parsed.auth.board_number, original.auth.board_number);
        assert_eq!(parsed.variables.get("env"), Some(&"prod".to_string()));
        assert_eq!(parsed.tui.hidden_lists, vec!["Done".to_string()]);
        assert_eq!(parsed.working_hours.start.as_deref(), Some("09:00"));
    }

    #[test]
    fn config_deserializes_from_partial_toml_with_defaults() {
        let toml_str = r#"
            [auth]
            session_id = "sid"
        "#;

        let config: Config = toml::from_str(toml_str).expect("parses partial config");

        assert_eq!(config.auth.session_id.as_deref(), Some("sid"));
        assert!(config.boards.is_empty());
        assert!(config.variables.is_empty());
        assert!(config.tui.hidden_lists.is_empty());
        assert_eq!(config.working_hours.start, None);
    }

    #[test]
    fn board_info_skips_serializing_last_used_when_none() {
        let board = BoardInfo {
            name: "Ops".to_string(),
            last_used: None,
        };

        let serialized = toml::to_string(&board).expect("serializes board info");

        assert!(!serialized.contains("last_used"));
    }

    #[test]
    fn remaining_display_returns_none_when_unconfigured() {
        let hours = WorkingHoursConfig::default();

        assert_eq!(
            hours.remaining_display(NaiveTime::from_hms_opt(10, 0, 0).unwrap()),
            None
        );
    }

    #[test]
    fn remaining_display_returns_none_outside_range() {
        let hours = WorkingHoursConfig {
            start: Some("09:00".to_string()),
            end: Some("18:00".to_string()),
        };

        assert_eq!(
            hours.remaining_display(NaiveTime::from_hms_opt(8, 0, 0).unwrap()),
            None
        );
        assert_eq!(
            hours.remaining_display(NaiveTime::from_hms_opt(19, 0, 0).unwrap()),
            None
        );
    }

    #[test]
    fn remaining_display_formats_time_left_within_range() {
        let hours = WorkingHoursConfig {
            start: Some("09:00".to_string()),
            end: Some("18:00".to_string()),
        };

        let display = hours
            .remaining_display(NaiveTime::from_hms_opt(17, 15, 0).unwrap())
            .expect("should be within range");

        assert_eq!(display, " 0h 45m restantes ");
    }

    #[test]
    fn remaining_display_returns_none_for_invalid_time_format() {
        let hours = WorkingHoursConfig {
            start: Some("not-a-time".to_string()),
            end: Some("18:00".to_string()),
        };

        assert_eq!(
            hours.remaining_display(NaiveTime::from_hms_opt(10, 0, 0).unwrap()),
            None
        );
    }

    #[test]
    fn config_file_path_is_under_config_dir() {
        let config_dir = Config::config_dir().expect("resolves config dir");
        let config_file = Config::config_file().expect("resolves config file");

        assert_eq!(config_file, config_dir.join("config.toml"));
        assert!(config_dir.ends_with("hteam"));
    }

    #[test]
    fn last_ticket_path_is_under_home_dir() {
        let path = Config::get_last_ticket_path().expect("resolves last ticket path");

        assert_eq!(
            path.file_name().and_then(|n| n.to_str()),
            Some(".last_ticket")
        );
    }
}
