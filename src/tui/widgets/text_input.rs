/// A plain text buffer with push/backspace, shared by every popup that
/// captures free text (description, comment, project id, new card name).
/// Replaces four `String` fields on `App`, each paired with its own
/// hand-written `_push`/`_backspace` pair in `events.rs`.
#[derive(Debug, Default, Clone)]
pub struct TextInput(String);

impl TextInput {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn push(&mut self, ch: char) {
        self.0.push(ch);
    }

    pub fn push_str(&mut self, s: &str) {
        self.0.push_str(s);
    }

    pub fn backspace(&mut self) {
        self.0.pop();
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn truncate(&mut self, new_len: usize) {
        self.0.truncate(new_len);
    }

    pub fn set(&mut self, text: impl Into<String>) {
        self.0 = text.into();
    }

    pub fn trim(&self) -> &str {
        self.0.trim()
    }
}
