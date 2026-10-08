use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Default, Debug)]
pub struct TextInput {
    pub text: String,
    pub cursor: usize,
}
impl TextInput {
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let cursor = text.len();
        Self { text, cursor }
    }
    pub fn insert(&mut self, value: &str, multiline: bool) {
        let clean: String = value
            .chars()
            .filter(|&c| !c.is_control() || (multiline && c == '\n'))
            .collect();
        self.text.insert_str(self.cursor, &clean);
        self.cursor += clean.len();
    }
    pub fn key(&mut self, event: KeyEvent) {
        if event.modifiers.contains(KeyModifiers::CONTROL) {
            if event.code == KeyCode::Char('u') {
                self.text.clear();
                self.cursor = 0;
            }
            return;
        }
        let previous = || {
            self.text[..self.cursor]
                .grapheme_indices(true)
                .next_back()
                .map(|(index, _)| index)
                .unwrap_or(0)
        };
        let next = || {
            self.text[self.cursor..]
                .graphemes(true)
                .next()
                .map(|value| self.cursor + value.len())
                .unwrap_or(self.cursor)
        };
        match event.code {
            KeyCode::Char(value) => self.insert(&value.to_string(), false),
            KeyCode::Left => self.cursor = previous(),
            KeyCode::Right => self.cursor = next(),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.text.len(),
            KeyCode::Backspace => {
                let index = previous();
                self.text.replace_range(index..self.cursor, "");
                self.cursor = index;
            }
            KeyCode::Delete => {
                let index = next();
                self.text.replace_range(self.cursor..index, "");
            }
            _ => {}
        }
    }
    pub fn viewport(&self, width: usize) -> (String, u16) {
        if width == 0 {
            return (String::new(), 0);
        }
        let display = self.text.replace('\n', " ");
        let before = &display[..self.cursor];
        let mut start = 0;
        let mut cursor_width = UnicodeWidthStr::width(before);
        for (index, grapheme) in before.grapheme_indices(true) {
            if cursor_width < width {
                break;
            }
            cursor_width = cursor_width.saturating_sub(UnicodeWidthStr::width(grapheme));
            start = index + grapheme.len();
        }
        (
            truncate(&display[start..], width, false),
            cursor_width.min(width - 1) as u16,
        )
    }
}

pub fn truncate(text: &str, width: usize, ascii: bool) -> String {
    if UnicodeWidthStr::width(text) <= width {
        return text.to_owned();
    }
    if width == 0 {
        return String::new();
    }
    let suffix = if ascii { "~" } else { "…" };
    let mut output = String::new();
    let mut used = 0;
    for grapheme in text.graphemes(true) {
        let next = UnicodeWidthStr::width(grapheme);
        if used + next > width - 1 {
            break;
        }
        output.push_str(grapheme);
        used += next;
    }
    output.push_str(suffix);
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deletes_graphemes_not_utf8_bytes() {
        let mut input = TextInput::new("中文e\u{301}👩‍💻");
        input.key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        assert_eq!(input.text, "中文e\u{301}");
        input.key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        assert_eq!(input.text, "中文");
        input.key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
        input.insert("好", false);
        assert_eq!(input.text, "中好文");
        assert_eq!(truncate("中文abc", 5, false), "中文…");
        assert!(input.viewport(3).1 < 3);
    }
}
