use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

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

/// One entry in the TUI projects MRU. TOML accepts a bare id (`500`) or a
/// table (`{ id = 500, name = "…" }`) so existing configs keep loading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownProject {
    pub id: u64,
    pub name: Option<String>,
}

impl KnownProject {
    pub fn new(id: u64, name: Option<String>) -> Self {
        Self {
            id,
            name: name.filter(|s| !s.trim().is_empty()),
        }
    }
}

impl Serialize for KnownProject {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.name.as_deref() {
            Some(name) => {
                use serde::ser::SerializeMap;
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("id", &self.id)?;
                map.serialize_entry("name", name)?;
                map.end()
            }
            None => serializer.serialize_u64(self.id),
        }
    }
}

impl<'de> Deserialize<'de> for KnownProject {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Id(u64),
            Named {
                id: u64,
                #[serde(default)]
                name: Option<String>,
            },
        }
        match Raw::deserialize(deserializer)? {
            Raw::Id(id) => Ok(KnownProject::new(id, None)),
            Raw::Named { id, name } => Ok(KnownProject::new(id, name)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TuiConfig {
    /// List names (case-insensitive) hidden from the `hteam tui` board. Toggled
    /// with 'v'/'V' inside the TUI; empty means every list is shown.
    #[serde(default)]
    pub hidden_lists: Vec<String>,
    /// Projects looked up from the 'P' popup, most-recently-used first. Names
    /// are cached when learned so the detail header does not stay as bare ids.
    #[serde(default)]
    pub known_projects: Vec<KnownProject>,
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
    /// Nombre del tema activo del TUI: un preset embebido ("default",
    /// "solarized", "high-contrast") o un tema personalizado en
    /// `themes_dir()/{active}.toml`. Resuelto por `Theme::resolve`, que cae
    /// a "default" (con aviso en el status bar) si no matchea nada o el
    /// archivo personalizado no parsea.
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

    /// Carpeta donde vive un archivo `{nombre}.toml` por cada tema
    /// personalizado — ver `tui::theme::Theme::load_custom`.
    pub fn themes_dir() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("themes"))
    }

    pub fn theme_file(name: &str) -> Result<PathBuf> {
        Ok(Self::themes_dir()?.join(format!("{name}.toml")))
    }

    /// Nombres de los temas personalizados encontrados en `themes_dir()`
    /// (el stem de cada `*.toml`), orden alfabético. Vacío si la carpeta
    /// todavía no existe — no haber creado ningún tema personalizado no es
    /// un error.
    pub fn custom_theme_names() -> Result<Vec<String>> {
        let dir = Self::themes_dir()?;
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut names: Vec<String> = fs::read_dir(&dir)
            .with_context(|| format!("No se pudo leer {}", dir.display()))?
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().extension().and_then(|e| e.to_str()) == Some("toml"))
            .filter_map(|entry| {
                entry
                    .path()
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
            })
            .collect();
        names.sort();
        Ok(names)
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
        self.save_to_path(&Self::config_file()?)
    }

    fn save_to_path(&self, config_file: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self).context("Error al serializar configuración")?;
        write_private_file(config_file, &content)
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

fn write_private_file(path: &Path, content: &str) -> Result<()> {
    let parent = path.parent().filter(|dir| !dir.as_os_str().is_empty());
    if let Some(dir) = parent {
        ensure_private_dir(dir)?;
        restrict_file_mode(dir, 0o700)?;
        cleanup_stale_temps(dir, path);
    }

    let tmp_path = private_temp_path(path);
    let mut guard = TempFileGuard::new(tmp_path.clone());
    write_temp_file(&tmp_path, content)?;
    fs::rename(&tmp_path, path)
        .with_context(|| format!("No se pudo reemplazar {}", path.display()))?;
    guard.keep();
    restrict_file_mode(path, 0o600)
}

struct TempFileGuard {
    path: PathBuf,
    keep: bool,
}

impl TempFileGuard {
    fn new(path: PathBuf) -> Self {
        Self { path, keep: false }
    }

    fn keep(&mut self) {
        self.keep = true;
    }
}

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn cleanup_stale_temps(dir: &Path, dest: &Path) {
    let Some(prefix) = temp_name_prefix(dest) else {
        return;
    };
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !is_stale_temp_name(name, &prefix) {
            continue;
        }
        let _ = fs::remove_file(entry.path());
    }
}

fn temp_name_prefix(path: &Path) -> Option<String> {
    path.file_name()
        .map(|name| format!(".{}{}", name.to_string_lossy(), ".tmp"))
}

fn is_stale_temp_name(name: &str, prefix: &str) -> bool {
    if name == prefix {
        return true;
    }
    let Some(rest) = name.strip_prefix(&format!("{prefix}-")) else {
        return false;
    };
    let Some(pid_str) = rest.split('-').next() else {
        return true;
    };
    let Ok(pid) = pid_str.parse::<u32>() else {
        return true;
    };
    !process_exists(pid)
}

fn process_exists(pid: u32) -> bool {
    #[cfg(unix)]
    {
        if pid == 0 {
            return false;
        }
        let result = unsafe { libc::kill(pid as libc::pid_t, 0) };
        if result == 0 {
            return true;
        }
        std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        true
    }
}

fn private_temp_path(path: &Path) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let suffix = format!(
        ".tmp-{}-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    temp_path_with_suffix(path, &suffix)
}

fn temp_path_with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut tmp_name = std::ffi::OsString::from(".");
    match path.file_name() {
        Some(name) => {
            tmp_name.push(name);
            tmp_name.push(suffix);
        }
        None => {
            tmp_name.push("config");
            tmp_name.push(suffix);
        }
    }

    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.join(tmp_name),
        _ => PathBuf::from(tmp_name),
    }
}

fn write_temp_file(tmp_path: &Path, content: &str) -> Result<()> {
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        opts.mode(0o600);
    }

    let mut file = opts.open(tmp_path).with_context(|| {
        format!(
            "No se pudo crear el archivo temporal de configuración {}",
            tmp_path.display()
        )
    })?;
    restrict_file_mode(tmp_path, 0o600)?;
    file.write_all(content.as_bytes()).with_context(|| {
        format!(
            "No se pudo escribir el archivo temporal de configuración {}",
            tmp_path.display()
        )
    })?;
    file.sync_all().with_context(|| {
        format!(
            "No se pudo sincronizar el archivo temporal de configuración {}",
            tmp_path.display()
        )
    })?;
    Ok(())
}

fn ensure_private_dir(dir: &Path) -> Result<()> {
    if dir.exists() {
        return Ok(());
    }

    if let Some(parent) = dir.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        if !parent.exists() {
            fs::create_dir_all(parent)
                .with_context(|| format!("No se pudo crear el directorio {}", parent.display()))?;
        }
    }

    #[cfg(unix)]
    {
        fs::DirBuilder::new()
            .mode(0o700)
            .create(dir)
            .with_context(|| format!("No se pudo crear el directorio {}", dir.display()))?;
    }
    #[cfg(not(unix))]
    {
        fs::create_dir(dir)
            .with_context(|| format!("No se pudo crear el directorio {}", dir.display()))?;
    }

    Ok(())
}

fn restrict_file_mode(path: &Path, mode: u32) -> Result<()> {
    #[cfg(unix)]
    {
        let mut perms = fs::metadata(path)
            .with_context(|| format!("No se pudo leer permisos de {}", path.display()))?
            .permissions();
        perms.set_mode(mode);
        fs::set_permissions(path, perms)
            .with_context(|| format!("No se pudo ajustar permisos de {}", path.display()))?;
    }
    #[cfg(not(unix))]
    {
        let _ = (path, mode);
    }
    Ok(())
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
    fn known_projects_accept_bare_ids_and_named_tables() {
        let toml_str = r#"
            [auth]
            [tui]
            known_projects = [500, { id = 469, name = "OKR" }]
        "#;
        let config: Config = toml::from_str(toml_str).expect("parses known projects");
        assert_eq!(config.tui.known_projects.len(), 2);
        assert_eq!(config.tui.known_projects[0].id, 500);
        assert_eq!(config.tui.known_projects[0].name, None);
        assert_eq!(config.tui.known_projects[1].id, 469);
        assert_eq!(config.tui.known_projects[1].name.as_deref(), Some("OKR"));

        let mut round = Config::default();
        round.tui.known_projects = vec![
            KnownProject::new(500, Some("TechFlow".into())),
            KnownProject::new(469, None),
        ];
        let serialized = toml::to_string_pretty(&round).expect("serialize");
        let parsed: Config = toml::from_str(&serialized).expect("reparse");
        assert_eq!(
            parsed.tui.known_projects[0].name.as_deref(),
            Some("TechFlow")
        );
        assert_eq!(parsed.tui.known_projects[1].id, 469);
        assert_eq!(parsed.tui.known_projects[1].name, None);
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
    fn save_creates_config_file_atomically_with_private_permissions() {
        let root = unique_temp_dir("hteam-config-create");
        let config_dir = root.join("hteam");
        let config_file = config_dir.join("config.toml");

        let mut config = Config::default();
        config.auth.session_id = Some("sid".to_string());
        config.auth.csrf_token = Some("csrf".to_string());
        config.auth.board_number = Some(7);

        config
            .save_to_path(&config_file)
            .expect("creates config file");

        let loaded: Config =
            toml::from_str(&fs::read_to_string(&config_file).expect("reads created config"))
                .expect("parses created config");
        assert_eq!(loaded.auth.session_id.as_deref(), Some("sid"));
        assert_eq!(loaded.auth.board_number, Some(7));
        assert!(!config_dir.join(".config.toml.tmp").exists());
        #[cfg(unix)]
        {
            assert_eq!(unix_mode(&config_file), 0o600);
            assert_eq!(unix_mode(&config_dir), 0o700);
        }

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn save_replaces_existing_config_without_leaving_temp_file() {
        let root = unique_temp_dir("hteam-config-update");
        let config_dir = root.join("hteam");
        fs::create_dir_all(&config_dir).expect("creates existing config dir");
        #[cfg(unix)]
        {
            let mut perms = fs::metadata(&config_dir)
                .expect("reads existing dir metadata")
                .permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&config_dir, perms).expect("sets world-readable existing dir");
        }
        let config_file = config_dir.join("config.toml");
        fs::write(&config_file, "stale = true\n").expect("writes stale config");
        #[cfg(unix)]
        {
            let mut perms = fs::metadata(&config_file)
                .expect("reads stale file metadata")
                .permissions();
            perms.set_mode(0o644);
            fs::set_permissions(&config_file, perms).expect("sets world-readable stale file");
        }

        let mut config = Config::default();
        config.auth.session_id = Some("sid".to_string());
        config.auth.csrf_token = Some("csrf".to_string());
        config.set_board_number(11);

        config
            .save_to_path(&config_file)
            .expect("updates existing config");

        let loaded: Config =
            toml::from_str(&fs::read_to_string(&config_file).expect("reads updated config"))
                .expect("parses updated config");
        assert_eq!(loaded.auth.session_id.as_deref(), Some("sid"));
        assert_eq!(loaded.get_board_number(), Some(11));
        assert!(!fs::read_to_string(&config_file)
            .expect("reads updated config")
            .contains("stale"));
        assert!(!config_dir.join(".config.toml.tmp").exists());
        #[cfg(unix)]
        {
            assert_eq!(unix_mode(&config_file), 0o600);
            assert_eq!(unix_mode(&config_dir), 0o700);
        }

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn save_replaces_stale_temp_file_and_keeps_it_private() {
        let root = unique_temp_dir("hteam-config-stale-tmp");
        let config_dir = root.join("hteam");
        fs::create_dir_all(&config_dir).expect("creates config dir");
        let config_file = config_dir.join("config.toml");
        let tmp_file = config_dir.join(".config.toml.tmp");
        fs::write(&tmp_file, "stale-secret = true\n").expect("writes leftover temp");
        #[cfg(unix)]
        {
            let mut perms = fs::metadata(&tmp_file)
                .expect("reads leftover temp metadata")
                .permissions();
            perms.set_mode(0o644);
            fs::set_permissions(&tmp_file, perms).expect("sets world-readable leftover temp");
        }

        let mut config = Config::default();
        config.auth.session_id = Some("sid".to_string());
        config.auth.csrf_token = Some("csrf".to_string());

        config
            .save_to_path(&config_file)
            .expect("replaces leftover temp");

        assert!(!tmp_file.exists());
        let loaded: Config =
            toml::from_str(&fs::read_to_string(&config_file).expect("reads created config"))
                .expect("parses created config");
        assert_eq!(loaded.auth.session_id.as_deref(), Some("sid"));
        #[cfg(unix)]
        {
            assert_eq!(unix_mode(&config_file), 0o600);
            assert_eq!(unix_mode(&config_dir), 0o700);
        }

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn write_temp_file_creates_owner_only_file() {
        let root = unique_temp_dir("hteam-config-temp-mode");
        let tmp_file = root.join(".config.toml.tmp");

        write_temp_file(&tmp_file, "auth = true\n").expect("creates temp file");

        assert_eq!(
            fs::read_to_string(&tmp_file).expect("reads temp file"),
            "auth = true\n"
        );
        #[cfg(unix)]
        {
            assert_eq!(unix_mode(&tmp_file), 0o600);
        }

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn save_error_mentions_path_not_secret_values() {
        let root = unique_temp_dir("hteam-config-error");
        let blocked = root.join("blocked");
        fs::write(&blocked, "not-a-directory").expect("creates blocking file");
        let config_file = blocked.join("config.toml");

        let mut config = Config::default();
        config.auth.session_id = Some("sid".to_string());
        config.auth.csrf_token = Some("csrf".to_string());

        let err = config
            .save_to_path(&config_file)
            .expect_err("parent is a file");
        let message = format!("{err:#}");

        assert!(message.contains(&blocked.display().to_string()));
        assert!(!message.contains("sid"));
        assert!(!message.contains("csrf"));
        assert!(!message.contains("session_id"));
        assert!(!message.contains("csrf_token"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn private_temp_paths_do_not_collide() {
        let path = PathBuf::from("/tmp/hteam/config.toml");
        let first = private_temp_path(&path);
        let second = private_temp_path(&path);

        assert_ne!(first, second);
        assert_eq!(first.parent(), second.parent());
        assert!(first
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap()
            .starts_with(".config.toml.tmp-"));
    }

    #[test]
    fn save_failed_replace_keeps_existing_file() {
        let root = unique_temp_dir("hteam-config-failed-replace");
        let config_dir = root.join("hteam");
        fs::create_dir_all(&config_dir).expect("creates config dir");
        let config_file = config_dir.join("config.toml");
        fs::create_dir(&config_file).expect("destination is a directory, not a file");

        let mut config = Config::default();
        config.auth.session_id = Some("sid".to_string());
        config.auth.csrf_token = Some("csrf".to_string());

        let err = config
            .save_to_path(&config_file)
            .expect_err("cannot replace a directory with a file");
        let message = format!("{err:#}");

        assert!(config_file.is_dir());
        assert!(config_dir
            .read_dir()
            .expect("reads config dir")
            .filter_map(|entry| entry.ok())
            .all(|entry| {
                let name = entry.file_name();
                name != *".config.toml.tmp"
                    && !name.to_string_lossy().starts_with(".config.toml.tmp-")
            }));
        assert!(message.contains(&config_file.display().to_string()));
        assert!(!message.contains("sid"));
        assert!(!message.contains("csrf"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn save_removes_dead_pid_temp_and_keeps_live_pid_temp() {
        let root = unique_temp_dir("hteam-config-stale-pid");
        let config_dir = root.join("hteam");
        fs::create_dir_all(&config_dir).expect("creates config dir");
        let config_file = config_dir.join("config.toml");
        let dead = config_dir.join(format!(".config.toml.tmp-{}-0-1", unused_pid()));
        let live = config_dir.join(format!(".config.toml.tmp-{}-0-1", std::process::id()));
        fs::write(&dead, "dead-secret = true\n").expect("writes dead pid leftover");
        fs::write(&live, "live-secret = true\n").expect("writes live pid leftover");

        let mut config = Config::default();
        config.auth.session_id = Some("sid".to_string());
        config.auth.csrf_token = Some("csrf".to_string());
        config
            .save_to_path(&config_file)
            .expect("saves after leftover cleanup");

        assert!(!dead.exists());
        assert_eq!(
            fs::read_to_string(&live).expect("reads live leftover"),
            "live-secret = true\n"
        );
        assert!(!config_dir.join(".config.toml.tmp").exists());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn is_stale_temp_name_keeps_live_pid_and_drops_dead_pid() {
        let prefix = ".config.toml.tmp";
        assert!(is_stale_temp_name(prefix, prefix));
        assert!(is_stale_temp_name(
            &format!(".config.toml.tmp-{}-0-1", unused_pid()),
            prefix
        ));
        assert!(!is_stale_temp_name(
            &format!(".config.toml.tmp-{}-0-1", std::process::id()),
            prefix
        ));
        assert!(!is_stale_temp_name("config.toml", prefix));
    }

    fn unused_pid() -> u32 {
        (2..u32::MAX)
            .find(|pid| !process_exists(*pid))
            .expect("finds a pid that is not running")
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "{prefix}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        fs::create_dir_all(&path).expect("creates unique temp dir");
        path
    }

    #[cfg(unix)]
    fn unix_mode(path: &Path) -> u32 {
        fs::metadata(path)
            .expect("reads metadata")
            .permissions()
            .mode()
            & 0o777
    }

    #[test]
    fn config_file_path_is_under_config_dir() {
        let config_dir = Config::config_dir().expect("resolves config dir");
        let config_file = Config::config_file().expect("resolves config file");

        assert_eq!(config_file, config_dir.join("config.toml"));
        assert!(config_dir.ends_with("hteam"));
    }

    #[test]
    fn theme_file_path_is_under_themes_dir() {
        let themes_dir = Config::themes_dir().expect("resolves themes dir");
        let theme_file = Config::theme_file("mi-tema").expect("resolves theme file");

        assert_eq!(theme_file, themes_dir.join("mi-tema.toml"));
        assert!(themes_dir.ends_with("hteam/themes"));
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
