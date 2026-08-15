//! Small, reusable TUI building blocks shared across popups — scroll state,
//! text-input state, and the centered-popup rendering boilerplate. None of
//! this knows about `App` or any specific popup; it's generic ratatui
//! plumbing that every popup used to reimplement by hand.

mod popup;
mod scroll;
mod text_input;

pub use popup::{centered_rect, draw_frame};
pub use scroll::Scroll;
pub use text_input::TextInput;
