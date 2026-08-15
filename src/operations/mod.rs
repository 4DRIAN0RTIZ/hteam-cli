//! Business logic shared by the CLI, TUI and MCP server. Every function here
//! takes a `&HteamClient` (and, when it needs to persist a preference, a
//! `&mut Config`) and returns plain domain data — no `println!`, no
//! `ratatui`, no JSON-formatting decisions. Each delivery surface (`cli`,
//! `tui`, `mcp`) is a thin adapter that calls into this module and decides
//! how to present the result.

pub mod boards;
pub mod cards;
pub mod checkin;
pub mod comments;
pub mod daily_work;
pub mod projects;
pub mod reminders;
pub mod session;
pub mod users;
pub mod working;

pub use session::{resolve_board_number, Session};
