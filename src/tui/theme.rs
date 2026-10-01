//! Roles semánticos de color para el TUI y sus 3 presets embebidos. `ui.rs` y
//! `widgets/popup.rs` no referencian `Color::` directamente — resuelven todo
//! a través del `Theme` activo, cargado una vez al iniciar `hteam tui` desde
//! `config.toml`'s `[theme] active`.

use std::fs;
use std::str::FromStr;

use anyhow::{Context, Result};
use ratatui::style::Color;
use serde::Deserialize;

use crate::config::Config;

/// Nombres de los presets embebidos, en el orden que `hteam theme list` los
/// muestra. No incluye temas personalizados — esos se listan aparte, ver
/// `Config::custom_theme_names`.
pub const THEME_NAMES: [&str; 3] = ["default", "solarized", "high-contrast"];

/// Espejo 1:1 de `Theme`, pero con cada color opcional y en texto — el
/// formato de `~/.config/hteam/themes/{name}.toml`. Un campo ausente hereda
/// el color de `base` (un preset embebido; "default" si se omite), así que
/// un tema personalizado puede sobreescribir uno o dos roles sin tener que
/// declarar los 18. Cada valor acepta cualquier string que entienda
/// `ratatui::style::Color::from_str`: un nombre (`"cyan"`, `"light-blue"`),
/// hex (`"#268bd2"`) o un índice ANSI (`"166"`).
#[derive(Debug, Default, Deserialize)]
struct ThemeFile {
    #[serde(default)]
    base: Option<String>,
    #[serde(default)]
    background: Option<String>,
    #[serde(default)]
    foreground: Option<String>,
    #[serde(default)]
    success: Option<String>,
    #[serde(default)]
    warning: Option<String>,
    #[serde(default)]
    danger: Option<String>,
    #[serde(default)]
    muted: Option<String>,
    #[serde(default)]
    highlight: Option<String>,
    #[serde(default)]
    selection_bg: Option<String>,
    #[serde(default)]
    selection_fg: Option<String>,
    #[serde(default)]
    border_active: Option<String>,
    #[serde(default)]
    border_help: Option<String>,
    #[serde(default)]
    border_new_card: Option<String>,
    #[serde(default)]
    border_reminders: Option<String>,
    #[serde(default)]
    border_objectives: Option<String>,
    #[serde(default)]
    border_description: Option<String>,
    #[serde(default)]
    border_comments: Option<String>,
    #[serde(default)]
    border_projects: Option<String>,
    #[serde(default)]
    border_board_switch: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    /// Fondo de toda la pantalla — board y popups (ver `ui::draw` y
    /// `widgets::draw_frame`). `Color::Reset` (el default) deja el fondo
    /// nativo de la terminal en vez de pintar uno propio.
    pub background: Color,
    /// Color de texto por defecto en toda la pantalla, salvo donde un rol
    /// más específico (`muted`, `warning`, etc.) lo pisa. `Color::Reset`
    /// deja el color de texto nativo de la terminal.
    pub foreground: Color,
    /// Texto positivo — tiempo restante de jornada laboral, seguimiento "On time".
    pub success: Color,
    /// Seguimiento por expirar pronto o desviado (pospuesto).
    pub warning: Color,
    /// Seguimiento expirado.
    pub danger: Color,
    /// Texto/borde de baja prioridad — versión en el footer, borde de campo
    /// sin foco, placeholder de fecha vacía.
    pub muted: Color,
    /// Resalta algo que requiere atención — borde de campo con foco, borde de
    /// card "working on it", fila del board activo en el popup de boards.
    pub highlight: Color,
    /// Fondo de la fila/card seleccionada.
    pub selection_bg: Color,
    /// Texto sobre `selection_bg`.
    pub selection_fg: Color,
    /// Borde de lo que está "activo" fuera del contexto de selección de fila:
    /// la lista con foco en el board, el panel con foco en el popup de
    /// proyectos.
    pub border_active: Color,
    pub border_help: Color,
    pub border_new_card: Color,
    pub border_reminders: Color,
    pub border_objectives: Color,
    pub border_description: Color,
    pub border_comments: Color,
    pub border_projects: Color,
    pub border_board_switch: Color,
}

impl Theme {
    /// Replica 1:1 la paleta que `ui.rs` tenía hardcodeada antes de esta
    /// feature — el preset por defecto no debe cambiar la apariencia actual.
    pub fn default_preset() -> Self {
        Self {
            background: Color::Reset,
            foreground: Color::Reset,
            success: Color::Green,
            warning: Color::Yellow,
            danger: Color::Red,
            muted: Color::DarkGray,
            highlight: Color::Yellow,
            selection_bg: Color::Cyan,
            selection_fg: Color::Black,
            border_active: Color::Cyan,
            border_help: Color::White,
            border_new_card: Color::Cyan,
            border_reminders: Color::Magenta,
            border_objectives: Color::Green,
            border_description: Color::Blue,
            border_comments: Color::Blue,
            border_projects: Color::Green,
            border_board_switch: Color::Yellow,
        }
    }

    pub fn solarized() -> Self {
        Self {
            background: Color::Rgb(0, 43, 54),
            foreground: Color::Rgb(131, 148, 150),
            success: Color::Rgb(133, 153, 0),
            warning: Color::Rgb(181, 137, 0),
            danger: Color::Rgb(220, 50, 47),
            muted: Color::Rgb(88, 110, 117),
            highlight: Color::Rgb(181, 137, 0),
            selection_bg: Color::Rgb(42, 161, 152),
            selection_fg: Color::Rgb(0, 43, 54),
            border_active: Color::Rgb(42, 161, 152),
            border_help: Color::Rgb(147, 161, 161),
            border_new_card: Color::Rgb(42, 161, 152),
            border_reminders: Color::Rgb(211, 54, 130),
            border_objectives: Color::Rgb(133, 153, 0),
            border_description: Color::Rgb(38, 139, 210),
            border_comments: Color::Rgb(38, 139, 210),
            border_projects: Color::Rgb(133, 153, 0),
            border_board_switch: Color::Rgb(181, 137, 0),
        }
    }

    /// Máximo contraste para terminales con poca fidelidad de color o
    /// necesidades de accesibilidad: solo variantes `Light*`/`White`/`Black`,
    /// sin grises intermedios (`DarkGray`/`Gray`) que se pierden en muchas
    /// terminales — incluido `muted`, que a propósito NO se atenúa acá.
    pub fn high_contrast() -> Self {
        Self {
            background: Color::Black,
            foreground: Color::White,
            success: Color::LightGreen,
            warning: Color::LightYellow,
            danger: Color::LightRed,
            muted: Color::White,
            highlight: Color::LightYellow,
            selection_bg: Color::White,
            selection_fg: Color::Black,
            border_active: Color::LightCyan,
            border_help: Color::White,
            border_new_card: Color::LightCyan,
            border_reminders: Color::LightMagenta,
            border_objectives: Color::LightGreen,
            border_description: Color::LightBlue,
            border_comments: Color::LightBlue,
            border_projects: Color::LightGreen,
            border_board_switch: Color::LightYellow,
        }
    }

    /// Resuelve un preset por nombre; si `name` no matchea ninguno conocido
    /// (config corrupta o de una versión vieja), hace fallback silencioso a
    /// `default_preset` sin panic.
    pub fn from_name(name: &str) -> Self {
        match name {
            "solarized" => Self::solarized(),
            "high-contrast" => Self::high_contrast(),
            _ => Self::default_preset(),
        }
    }

    /// Resuelve el tema activo: primero contra los presets embebidos, y si
    /// `name` no matchea ninguno, contra un tema personalizado en
    /// `~/.config/hteam/themes/{name}.toml`. A diferencia de `from_name`,
    /// acá el fallback a `default_preset` nunca es silencioso — un tema
    /// personalizado roto (typo en el nombre, color inválido, TOML mal
    /// formado) es un error del usuario que vale la pena mostrarle, no un
    /// `config.toml` viejo de una versión anterior. El mensaje, si lo hay,
    /// es para mostrar en el status bar del TUI al arrancar.
    pub fn resolve(name: &str) -> (Self, Option<String>) {
        if THEME_NAMES.contains(&name) {
            return (Self::from_name(name), None);
        }

        match Self::load_custom(name) {
            Ok(theme) => (theme, None),
            Err(e) => (
                Self::default_preset(),
                Some(format!(
                    "No se pudo cargar el tema personalizado '{name}': {e:#}. Usando 'default'."
                )),
            ),
        }
    }

    /// Carga y valida `~/.config/hteam/themes/{name}.toml` — ver `ThemeFile`
    /// para el formato. Usada tanto por `resolve` (fallback silencioso al
    /// default) como por `hteam theme set` (falla duro para avisar del
    /// error apenas el usuario lo activa, en vez de que se entere recién al
    /// abrir el TUI).
    pub fn load_custom(name: &str) -> Result<Self> {
        let path = Config::theme_file(name)?;
        let content = fs::read_to_string(&path)
            .with_context(|| format!("no se encontró {}", path.display()))?;
        Self::parse_custom(&content).with_context(|| format!("error al parsear {}", path.display()))
    }

    /// Parte pura de `load_custom` — separada para poder testear el parseo
    /// (herencia de `base`, colores inválidos) sin tocar el filesystem real
    /// del usuario.
    fn parse_custom(content: &str) -> Result<Self> {
        let file: ThemeFile = toml::from_str(content).context("TOML inválido")?;

        let mut theme = file
            .base
            .as_deref()
            .map(Self::from_name)
            .unwrap_or_else(Self::default_preset);

        let parse = |field: &str, s: &str| -> Result<Color> {
            Color::from_str(s).with_context(|| format!("color inválido en '{field}': '{s}'"))
        };

        if let Some(s) = &file.background {
            theme.background = parse("background", s)?;
        }
        if let Some(s) = &file.foreground {
            theme.foreground = parse("foreground", s)?;
        }
        if let Some(s) = &file.success {
            theme.success = parse("success", s)?;
        }
        if let Some(s) = &file.warning {
            theme.warning = parse("warning", s)?;
        }
        if let Some(s) = &file.danger {
            theme.danger = parse("danger", s)?;
        }
        if let Some(s) = &file.muted {
            theme.muted = parse("muted", s)?;
        }
        if let Some(s) = &file.highlight {
            theme.highlight = parse("highlight", s)?;
        }
        if let Some(s) = &file.selection_bg {
            theme.selection_bg = parse("selection_bg", s)?;
        }
        if let Some(s) = &file.selection_fg {
            theme.selection_fg = parse("selection_fg", s)?;
        }
        if let Some(s) = &file.border_active {
            theme.border_active = parse("border_active", s)?;
        }
        if let Some(s) = &file.border_help {
            theme.border_help = parse("border_help", s)?;
        }
        if let Some(s) = &file.border_new_card {
            theme.border_new_card = parse("border_new_card", s)?;
        }
        if let Some(s) = &file.border_reminders {
            theme.border_reminders = parse("border_reminders", s)?;
        }
        if let Some(s) = &file.border_objectives {
            theme.border_objectives = parse("border_objectives", s)?;
        }
        if let Some(s) = &file.border_description {
            theme.border_description = parse("border_description", s)?;
        }
        if let Some(s) = &file.border_comments {
            theme.border_comments = parse("border_comments", s)?;
        }
        if let Some(s) = &file.border_projects {
            theme.border_projects = parse("border_projects", s)?;
        }
        if let Some(s) = &file.border_board_switch {
            theme.border_board_switch = parse("border_board_switch", s)?;
        }

        Ok(theme)
    }

    /// Resuelve el color de urgencia de un seguimiento a partir del texto que
    /// Hteam ya trae calculado en `Card::time_status` (`get_time_status` de la
    /// API, ej. "On time", "Expired"). Matchea por substring case-insensitive
    /// (cubre tanto el inglés de la API como un eventual "Expirada"/"Desviada"
    /// en español) y cae a `muted` ante cualquier valor desconocido, en vez de
    /// asumir un enum cerrado que se rompería si Hteam cambia el texto.
    pub fn time_status_color(&self, status: &str) -> Color {
        let s = status.to_lowercase();
        if s.contains("expired") || s.contains("expirad") {
            self.danger
        } else if s.contains("soon")
            || s.contains("expirar")
            || s.contains("deviat")
            || s.contains("desviad")
        {
            self.warning
        } else if s.contains("on time") || s.contains("en tiempo") {
            self.success
        } else {
            self.muted
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::default_preset()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_name_resolves_known_presets() {
        assert_eq!(Theme::from_name("default"), Theme::default_preset());
        assert_eq!(Theme::from_name("solarized"), Theme::solarized());
        assert_eq!(Theme::from_name("high-contrast"), Theme::high_contrast());
    }

    #[test]
    fn from_name_falls_back_to_default_for_unknown_name() {
        assert_eq!(Theme::from_name("no-existe"), Theme::default_preset());
        assert_eq!(Theme::from_name(""), Theme::default_preset());
    }

    #[test]
    fn parse_custom_overrides_only_given_fields_inheriting_the_rest_from_base() {
        let theme = Theme::parse_custom(
            r##"
            base = "solarized"
            success = "#ff00ff"
            "##,
        )
        .expect("valid theme file");

        assert_eq!(theme.success, Color::Rgb(255, 0, 255));
        // Todo lo demás heredado tal cual de "solarized", no del default.
        assert_eq!(theme.danger, Theme::solarized().danger);
        assert_eq!(theme.border_active, Theme::solarized().border_active);
        assert_eq!(theme.background, Theme::solarized().background);
        assert_eq!(theme.foreground, Theme::solarized().foreground);
    }

    #[test]
    fn parse_custom_overrides_background_and_foreground() {
        let theme = Theme::parse_custom(
            r##"
            background = "#0a0e27"
            foreground = "#cbd0f5"
            "##,
        )
        .expect("valid theme file");

        assert_eq!(theme.background, Color::Rgb(0x0a, 0x0e, 0x27));
        assert_eq!(theme.foreground, Color::Rgb(0xcb, 0xd0, 0xf5));
        // No tocados por el override: siguen viniendo del default preset.
        assert_eq!(theme.success, Theme::default_preset().success);
    }

    #[test]
    fn parse_custom_defaults_base_to_default_preset_when_omitted() {
        let theme = Theme::parse_custom(r#"highlight = "cyan""#).expect("valid theme file");

        assert_eq!(theme.highlight, Color::Cyan);
        assert_eq!(theme.danger, Theme::default_preset().danger);
    }

    #[test]
    fn parse_custom_accepts_named_hex_and_indexed_colors() {
        let theme = Theme::parse_custom(
            r##"
            success = "lightgreen"
            warning = "#b58900"
            danger = "160"
            "##,
        )
        .expect("valid theme file");

        assert_eq!(theme.success, Color::LightGreen);
        assert_eq!(theme.warning, Color::Rgb(0xb5, 0x89, 0x00));
        assert_eq!(theme.danger, Color::Indexed(160));
    }

    #[test]
    fn parse_custom_rejects_invalid_color() {
        let err = Theme::parse_custom(r#"success = "not-a-color""#).expect_err("should fail");
        assert!(err.to_string().contains("success"));
    }

    #[test]
    fn parse_custom_rejects_malformed_toml() {
        assert!(Theme::parse_custom("not = [valid").is_err());
    }

    #[test]
    fn time_status_color_maps_known_statuses() {
        let theme = Theme::default_preset();
        assert_eq!(theme.time_status_color("Expired"), theme.danger);
        assert_eq!(theme.time_status_color("Expirada"), theme.danger);
        assert_eq!(theme.time_status_color("Due soon"), theme.warning);
        assert_eq!(theme.time_status_color("Desviada"), theme.warning);
        assert_eq!(theme.time_status_color("On time"), theme.success);
    }

    #[test]
    fn time_status_color_falls_back_to_muted_for_unknown_status() {
        let theme = Theme::default_preset();
        assert_eq!(theme.time_status_color("who knows"), theme.muted);
        assert_eq!(theme.time_status_color(""), theme.muted);
    }
}
