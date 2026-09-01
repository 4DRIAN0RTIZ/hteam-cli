use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::models::{WeeklyObjective, WeeklyObjectivesSet};

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

/// Cuánto tiempo se considera "fresco" el cache de objetivos semanales antes
/// de que `hteam objectives show` vuelva a pegarle a SharePad.
const WEEKLY_OBJECTIVES_CACHE_TTL_HOURS: i64 = 24;

/// Notebook público de SharePad usado por defecto por `hteam objectives show`
/// — configurable vía `[weekly_objectives] source_url` en `config.toml`.
pub const DEFAULT_WEEKLY_OBJECTIVES_SOURCE_URL: &str = "https://sharepad.in/n/okr-semanales";

fn default_weekly_objectives_source_url() -> String {
    DEFAULT_WEEKLY_OBJECTIVES_SOURCE_URL.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeeklyObjectivesConfig {
    /// URL del notebook público de SharePad a scrapear.
    #[serde(default = "default_weekly_objectives_source_url")]
    pub source_url: String,
    /// Timestamp RFC3339 de la última sincronización exitosa (mismo formato
    /// que `BoardInfo::last_used`). `None` si nunca se sincronizó.
    #[serde(default)]
    pub last_synced_at: Option<String>,
    /// Título del último resultado scrapeado exitosamente.
    #[serde(default)]
    pub cached_title: Option<String>,
    /// Objetivos (con sus tareas) del último resultado scrapeado exitosamente.
    #[serde(default)]
    pub cached_objectives: Vec<WeeklyObjective>,
}

impl Default for WeeklyObjectivesConfig {
    fn default() -> Self {
        Self {
            source_url: default_weekly_objectives_source_url(),
            last_synced_at: None,
            cached_title: None,
            cached_objectives: Vec::new(),
        }
    }
}

impl WeeklyObjectivesConfig {
    /// `true` cuando `last_synced_at` existe y tiene menos de 24h respecto a
    /// `now`. `--force` se resuelve en el caller (`operations::objectives`),
    /// no acá, para mantener esta función puramente sobre la antigüedad del
    /// cache.
    pub fn is_cache_fresh(&self, now: DateTime<Utc>) -> bool {
        self.last_synced_at
            .as_deref()
            .and_then(|ts| DateTime::parse_from_rfc3339(ts).ok())
            .map(|synced_at| {
                now.signed_duration_since(synced_at)
                    < Duration::hours(WEEKLY_OBJECTIVES_CACHE_TTL_HOURS)
            })
            .unwrap_or(false)
    }

    /// Reconstruye el último resultado cacheado, si hay uno completo
    /// (requiere al menos un título guardado).
    pub fn cached_set(&self) -> Option<WeeklyObjectivesSet> {
        let title = self.cached_title.clone()?;
        Some(WeeklyObjectivesSet {
            title,
            objectives: self.cached_objectives.clone(),
        })
    }
}

fn default_theme_name() -> String {
    "default".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    /// Nombre del preset activo del TUI: "default", "solarized" o
    /// "high-contrast". Si no matchea ninguno conocido, `Theme::from_name`
    /// hace fallback silencioso a "default" sin panic.
    #[serde(default = "default_theme_name")]
    pub active: String,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            active: default_theme_name(),
        }
    }
}

/// Cuánto tiempo se considera "fresco" el chequeo de nueva versión antes de
/// que `hteam` vuelva a pegarle a la API de GitHub.
const UPDATE_CHECK_TTL_HOURS: i64 = 6;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateConfig {
    /// Timestamp RFC3339 del último chequeo exitoso contra GitHub releases
    /// (mismo formato que `WeeklyObjectivesConfig::last_synced_at`). `None`
    /// si nunca se chequeó.
    #[serde(default)]
    pub last_checked_at: Option<String>,
    /// Última versión conocida publicada en GitHub (sin prefijo `v`),
    /// cacheada para no pegarle a la API en cada invocación del CLI.
    #[serde(default)]
    pub latest_known_version: Option<String>,
    /// Versión del binario la última vez que se mostró el changelog
    /// post-actualización. `None` antes de la primera corrida.
    #[serde(default)]
    pub last_seen_version: Option<String>,
}

impl UpdateConfig {
    /// `true` cuando `last_checked_at` existe y tiene menos de
    /// `UPDATE_CHECK_TTL_HOURS` respecto a `now`.
    pub fn is_check_fresh(&self, now: DateTime<Utc>) -> bool {
        self.last_checked_at
            .as_deref()
            .and_then(|ts| DateTime::parse_from_rfc3339(ts).ok())
            .map(|checked_at| {
                now.signed_duration_since(checked_at) < Duration::hours(UPDATE_CHECK_TTL_HOURS)
            })
            .unwrap_or(false)
    }
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
    #[serde(default)]
    pub weekly_objectives: WeeklyObjectivesConfig,
    #[serde(default)]
    pub theme: ThemeConfig,
    #[serde(default)]
    pub update: UpdateConfig,
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

    #[test]
    fn weekly_objectives_default_uses_sharepad_url() {
        let config = Config::default();

        assert_eq!(
            config.weekly_objectives.source_url,
            DEFAULT_WEEKLY_OBJECTIVES_SOURCE_URL
        );
        assert_eq!(config.weekly_objectives.last_synced_at, None);
        assert!(config.weekly_objectives.cached_objectives.is_empty());
    }

    #[test]
    fn weekly_objectives_deserializes_from_config_missing_the_section() {
        let toml_str = r#"
            [auth]
            session_id = "sid"
        "#;

        let config: Config = toml::from_str(toml_str).expect("parses config without section");

        assert_eq!(
            config.weekly_objectives.source_url,
            DEFAULT_WEEKLY_OBJECTIVES_SOURCE_URL
        );
    }

    #[test]
    fn weekly_objectives_round_trips_source_url_and_cache_through_toml() {
        let mut original = Config::default();
        original.weekly_objectives.source_url = "https://sharepad.in/n/otro-notebook".to_string();
        original.weekly_objectives.last_synced_at = Some("2026-08-30T10:00:00+00:00".to_string());
        original.weekly_objectives.cached_title = Some("Objetivos semanales".to_string());
        original.weekly_objectives.cached_objectives = vec![WeeklyObjective {
            name: "Demo CCL".to_string(),
            tasks: vec![crate::models::WeeklyTask {
                description: "Tarea 1 xd".to_string(),
                done: false,
            }],
        }];

        let serialized = toml::to_string_pretty(&original).expect("serializes to TOML");
        let parsed: Config = toml::from_str(&serialized).expect("parses back from TOML");

        assert_eq!(
            parsed.weekly_objectives.source_url,
            "https://sharepad.in/n/otro-notebook"
        );
        assert_eq!(
            parsed.weekly_objectives.last_synced_at.as_deref(),
            Some("2026-08-30T10:00:00+00:00")
        );
        assert_eq!(
            parsed.weekly_objectives.cached_title.as_deref(),
            Some("Objetivos semanales")
        );
        assert_eq!(parsed.weekly_objectives.cached_objectives.len(), 1);
        assert_eq!(
            parsed.weekly_objectives.cached_objectives[0].tasks[0].description,
            "Tarea 1 xd"
        );
    }

    #[test]
    fn weekly_objectives_source_url_falls_back_when_section_present_but_field_missing() {
        let toml_str = r#"
            [auth]
            session_id = "sid"

            [weekly_objectives]
            last_synced_at = "2026-08-30T10:00:00+00:00"
        "#;

        let config: Config = toml::from_str(toml_str).expect("parses partial section");

        assert_eq!(
            config.weekly_objectives.source_url,
            DEFAULT_WEEKLY_OBJECTIVES_SOURCE_URL
        );
        assert_eq!(
            config.weekly_objectives.last_synced_at.as_deref(),
            Some("2026-08-30T10:00:00+00:00")
        );
    }

    #[test]
    fn is_cache_fresh_is_false_without_last_synced_at() {
        let weekly = WeeklyObjectivesConfig::default();

        assert!(!weekly.is_cache_fresh(Utc::now()));
    }

    #[test]
    fn is_cache_fresh_is_true_under_24h() {
        let now = Utc::now();
        let weekly = WeeklyObjectivesConfig {
            last_synced_at: Some((now - Duration::hours(23)).to_rfc3339()),
            ..WeeklyObjectivesConfig::default()
        };

        assert!(weekly.is_cache_fresh(now));
    }

    #[test]
    fn is_cache_fresh_is_false_at_or_after_24h() {
        let now = Utc::now();
        let weekly = WeeklyObjectivesConfig {
            last_synced_at: Some((now - Duration::hours(24)).to_rfc3339()),
            ..WeeklyObjectivesConfig::default()
        };

        assert!(!weekly.is_cache_fresh(now));
    }

    #[test]
    fn is_cache_fresh_ignores_invalid_timestamps() {
        let weekly = WeeklyObjectivesConfig {
            last_synced_at: Some("not-a-timestamp".to_string()),
            ..WeeklyObjectivesConfig::default()
        };

        assert!(!weekly.is_cache_fresh(Utc::now()));
    }

    #[test]
    fn cached_set_returns_none_without_a_cached_title() {
        let weekly = WeeklyObjectivesConfig::default();

        assert_eq!(weekly.cached_set(), None);
    }

    #[test]
    fn cached_set_rebuilds_the_last_scraped_result() {
        let weekly = WeeklyObjectivesConfig {
            cached_title: Some("Objetivos semanales".to_string()),
            cached_objectives: vec![WeeklyObjective {
                name: "Demo CCL".to_string(),
                tasks: vec![],
            }],
            ..WeeklyObjectivesConfig::default()
        };

        let set = weekly.cached_set().expect("cached set");

        assert_eq!(set.title, "Objetivos semanales");
        assert_eq!(set.objectives[0].name, "Demo CCL");
    }

    #[test]
    fn theme_defaults_to_default_preset_name() {
        let config = Config::default();

        assert_eq!(config.theme.active, "default");
    }

    #[test]
    fn theme_deserializes_from_config_missing_the_section() {
        let toml_str = r#"
            [auth]
            session_id = "sid"
        "#;

        let config: Config = toml::from_str(toml_str).expect("parses config without section");

        assert_eq!(config.theme.active, "default");
    }

    #[test]
    fn theme_round_trips_active_preset_through_toml() {
        let mut original = Config::default();
        original.theme.active = "solarized".to_string();

        let serialized = toml::to_string_pretty(&original).expect("serializes to TOML");
        let parsed: Config = toml::from_str(&serialized).expect("parses back from TOML");

        assert_eq!(parsed.theme.active, "solarized");
    }

    #[test]
    fn update_default_has_no_cached_state() {
        let config = Config::default();

        assert_eq!(config.update.last_checked_at, None);
        assert_eq!(config.update.latest_known_version, None);
        assert_eq!(config.update.last_seen_version, None);
    }

    #[test]
    fn update_deserializes_from_config_missing_the_section() {
        let toml_str = r#"
            [auth]
            session_id = "sid"
        "#;

        let config: Config = toml::from_str(toml_str).expect("parses config without section");

        assert_eq!(config.update.latest_known_version, None);
    }

    #[test]
    fn update_round_trips_through_toml() {
        let mut original = Config::default();
        original.update.last_checked_at = Some("2026-08-31T10:00:00+00:00".to_string());
        original.update.latest_known_version = Some("0.7.0".to_string());
        original.update.last_seen_version = Some("0.6.2".to_string());

        let serialized = toml::to_string_pretty(&original).expect("serializes to TOML");
        let parsed: Config = toml::from_str(&serialized).expect("parses back from TOML");

        assert_eq!(
            parsed.update.last_checked_at.as_deref(),
            Some("2026-08-31T10:00:00+00:00")
        );
        assert_eq!(parsed.update.latest_known_version.as_deref(), Some("0.7.0"));
        assert_eq!(parsed.update.last_seen_version.as_deref(), Some("0.6.2"));
    }

    #[test]
    fn is_check_fresh_is_false_without_last_checked_at() {
        let update = UpdateConfig::default();

        assert!(!update.is_check_fresh(Utc::now()));
    }

    #[test]
    fn is_check_fresh_is_true_under_ttl() {
        let now = Utc::now();
        let update = UpdateConfig {
            last_checked_at: Some((now - Duration::hours(5)).to_rfc3339()),
            ..UpdateConfig::default()
        };

        assert!(update.is_check_fresh(now));
    }

    #[test]
    fn is_check_fresh_is_false_at_or_after_ttl() {
        let now = Utc::now();
        let update = UpdateConfig {
            last_checked_at: Some((now - Duration::hours(6)).to_rfc3339()),
            ..UpdateConfig::default()
        };

        assert!(!update.is_check_fresh(now));
    }

    #[test]
    fn is_check_fresh_ignores_invalid_timestamps() {
        let update = UpdateConfig {
            last_checked_at: Some("not-a-timestamp".to_string()),
            ..UpdateConfig::default()
        };

        assert!(!update.is_check_fresh(Utc::now()));
    }
}
