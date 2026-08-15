/// Vertical scroll offset shared by every scrollable popup (help, reminders,
/// comments, the projects detail pane). Replaces four near-identical `u16`
/// fields on `App`, each paired with its own hand-written clamped-increment
/// function in `events.rs`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Scroll(u16);

impl Scroll {
    pub fn offset(self) -> u16 {
        self.0
    }

    /// Moves the offset by `delta` rows, clamped at 0 — never scrolls above
    /// the top.
    pub fn by(&mut self, delta: i32) {
        self.0 = (self.0 as i32 + delta).max(0) as u16;
    }

    pub fn reset(&mut self) {
        self.0 = 0;
    }
}
