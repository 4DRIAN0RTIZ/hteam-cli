//! Roles semánticos de color para el TUI y sus 3 presets embebidos. `ui.rs` y
//! `widgets/popup.rs` no referencian `Color::` directamente — resuelven todo
//! a través del `Theme` activo, cargado una vez al iniciar `hteam tui` desde
//! `config.toml`'s `[theme] active`.

use ratatui::style::Color;

/// Nombres válidos de preset, en el orden que `hteam theme list` los muestra.
pub const THEME_NAMES: [&str; 3] = ["default", "solarized", "high-contrast"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    /// Texto positivo — hoy solo el tiempo restante de jornada laboral.
    pub success: Color,
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
            success: Color::Green,
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
            success: Color::Rgb(133, 153, 0),
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
            success: Color::LightGreen,
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
}
