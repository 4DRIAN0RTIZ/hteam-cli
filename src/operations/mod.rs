//! Business logic shared by the CLI, TUI and MCP server. Every function here
//! takes a `&HteamClient` (and, when it needs to persist a preference, a
//! `&mut Config`) and returns plain domain data — no `println!`, no
//! `ratatui`, no JSON-formatting decisions. Each delivery surface (`cli`,
//! `tui`, `mcp`) is a thin adapter that calls into this module and decides
//! how to present the result.

pub mod boards;
pub mod cards;
pub mod checkin;
pub mod clipboard;
pub mod comments;
pub mod daily_work;
pub mod objectives;
pub mod projects;
pub mod reminders;
pub mod session;
pub mod update;
pub mod users;
pub mod working;

pub use session::{resolve_board_number, Session};

/// Glifo de Nerd Font para "copiar" (nf-md-content_copy), usado por el CLI y
/// el TUI al reportar que un link (card o proyecto) quedó en el
/// portapapeles — centralizado acá, no en `cards` ni `projects`, para que
/// todas las superficies lo compartan en vez de duplicar el literal. Un
/// glifo monocromo en vez de un emoji a color respeta el fg del `Style` en
/// el TUI; requiere una Nerd Font en la terminal, sin una se ve como un
/// glifo faltante.
pub const COPY_ICON: char = '\u{ed7a}';
