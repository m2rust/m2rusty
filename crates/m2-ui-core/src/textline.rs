//! `CTextLine`: the window wrapper around a [`TextInstance`].

use crate::event::UiHost;
use crate::text::TextInstance;
use crate::window::{Ui, WindowId};

impl Ui {
    /// The text instance of a text line window.
    pub fn text_mut(&mut self, id: WindowId) -> &mut TextInstance {
        self.w_mut(id).text.as_deref_mut().expect("not a text line")
    }

    /// `SetFontName`: gives the line a font (glyphs come from the host).
    pub fn text_set_font(&mut self, id: WindowId, has_font: bool) {
        self.text_mut(id).set_font(has_font);
    }

    /// `CTextLine::SetText`: sets the value and updates the layout at once.
    pub fn text_set_text(&mut self, host: &mut dyn UiHost, id: WindowId, text: &str) {
        let t = self.text_mut(id);
        t.set_value(text);
        t.update(host);
    }

    /// Legacy-bytes entry point (D14): text as stored in the data files.
    pub fn text_set_text_bytes(&mut self, host: &mut dyn UiHost, id: WindowId, bytes: &[u8], code_page: Option<u32>) {
        let t = self.text_mut(id);
        t.set_value_bytes(bytes, code_page);
        t.update(host);
    }

    /// `GetCursorPosition`: character under the mouse.
    pub fn text_cursor_position(&self, id: WindowId) -> i32 {
        let (lx, _) = self.get_mouse_local_position(id);
        self.w(id).text().expect("not a text line").pixel_position_to_character_position(lx as i32)
    }
}
