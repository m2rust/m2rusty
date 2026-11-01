//! The edit buffer behind `ime.*` in the Linux/SDL client
//! (`neoeter/src/render/IME_Sdl.cpp`): text, cursor, composition and the
//! insertion / deletion rules, with offsets in bytes of UTF-8 as in C++.
//!
//! The Windows IMM implementation (`IME.cpp`) is compiled out on the targets
//! this port has; its tag-aware cursor movement is not ported.

const IMESTR_MAXLEN: usize = 1024;

fn is_cont(b: u8) -> bool {
    b & 0xC0 == 0x80
}

fn utf8_length(s: &[u8]) -> usize {
    s.iter().filter(|b| !is_cont(**b)).count()
}

/// Decodes one code point at `s[*i]`, advancing `i`; invalid input is U+FFFD.
fn decode_utf8(s: &[u8], i: &mut usize) -> u32 {
    let c = s[*i];
    *i += 1;
    let (mut extra, mut cp) = (0, c as u32);
    if c >= 0xF0 {
        extra = 3;
        cp = (c & 0x07) as u32;
    } else if c >= 0xE0 {
        extra = 2;
        cp = (c & 0x0F) as u32;
    } else if c >= 0xC0 {
        extra = 1;
        cp = (c & 0x1F) as u32;
    } else if c >= 0x80 {
        return 0xFFFD;
    }
    while extra > 0 {
        if *i >= s.len() || !is_cont(s[*i]) {
            return 0xFFFD;
        }
        cp = (cp << 6) | (s[*i] & 0x3F) as u32;
        *i += 1;
        extra -= 1;
    }
    cp
}

#[derive(Debug, Clone)]
pub struct ImeEdit {
    text: Vec<u8>,
    composition: Vec<u8>,
    cur_pos: i32,
    last_pos: i32,
    comp_len: i32,
    ul_begin: i32,
    ul_end: i32,
    max: i32,
    only_number: bool,
    except_keys: Vec<u32>,
    paste_enabled: bool,
    capture_input: bool,
}

impl Default for ImeEdit {
    fn default() -> Self {
        Self {
            text: Vec::new(),
            composition: Vec::new(),
            cur_pos: 0,
            last_pos: 0,
            comp_len: 0,
            ul_begin: 0,
            ul_end: 0,
            max: IMESTR_MAXLEN as i32,
            only_number: false,
            except_keys: Vec::new(),
            paste_enabled: true,
            capture_input: false,
        }
    }
}

impl ImeEdit {
    pub fn new() -> Self {
        Self::default()
    }

    /// `CIME::Clear`.
    pub fn clear(&mut self) {
        self.text.clear();
        self.composition.clear();
        self.cur_pos = 0;
        self.last_pos = 0;
        self.comp_len = 0;
        self.ul_begin = 0;
        self.ul_end = 0;
    }

    pub fn set_max(&mut self, max: i32) {
        self.max = max;
    }
    pub fn set_number_mode(&mut self) {
        self.only_number = true;
    }
    pub fn set_string_mode(&mut self) {
        self.only_number = false;
    }
    pub fn add_except_key(&mut self, key: u32) {
        self.except_keys.push(key);
    }
    pub fn clear_except_keys(&mut self) {
        self.except_keys.clear();
    }
    pub fn enable_paste(&mut self, flag: bool) {
        self.paste_enabled = flag;
    }
    pub fn set_capture_input(&mut self, flag: bool) {
        self.capture_input = flag;
    }

    pub fn is_writable(&self, key: u32) -> bool {
        !self.except_keys.contains(&key)
    }

    /// `SetText`: replaces the text, cursor at the end. `len < 0` is the whole
    /// string.
    pub fn set_text(&mut self, text: &[u8], len: i32) {
        let n = if len < 0 { text.len() } else { (len as usize).min(text.len()) };
        self.text = text[..n].to_vec();
        self.cur_pos = self.text.len() as i32;
        self.last_pos = self.cur_pos;
    }

    pub fn text_bytes(&self) -> &[u8] {
        &self.text
    }

    /// `GetText` as UTF-8 (invalid sequences replaced).
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.text).into_owned()
    }

    pub fn cur_pos(&self) -> i32 {
        self.cur_pos
    }
    pub fn last_pos(&self) -> i32 {
        self.last_pos
    }
    pub fn comp_len(&self) -> i32 {
        self.comp_len
    }
    pub fn underline(&self) -> (i32, i32) {
        (self.ul_begin, self.ul_end)
    }
    pub fn composition(&self) -> &[u8] {
        &self.composition
    }

    pub fn inc_cur_pos(&mut self) {
        if self.cur_pos < self.last_pos {
            self.cur_pos += 1;
        }
        while self.cur_pos < self.last_pos && is_cont(self.byte(self.cur_pos)) {
            self.cur_pos += 1;
        }
    }

    pub fn dec_cur_pos(&mut self) {
        if self.cur_pos > 0 {
            self.cur_pos -= 1;
        }
        while self.cur_pos > 0 && is_cont(self.byte(self.cur_pos)) {
            self.cur_pos -= 1;
        }
    }

    fn byte(&self, i: i32) -> u8 {
        self.text.get(i as usize).copied().unwrap_or(0)
    }

    /// `SetCurPos(offset)`: `offset` counts characters; negative means the end.
    pub fn set_cur_pos(&mut self, offset: i32) {
        self.last_pos = self.text.len() as i32;
        if offset < 0 {
            self.cur_pos = self.last_pos;
            return;
        }
        let mut pos = 0usize;
        let mut n = 0;
        while n < offset && pos < self.text.len() {
            pos += 1;
            while pos < self.text.len() && is_cont(self.text[pos]) {
                pos += 1;
            }
            n += 1;
        }
        self.cur_pos = pos as i32;
    }

    /// Delete key: removes the character at the cursor.
    pub fn del_cur_pos(&mut self) {
        if self.cur_pos < self.last_pos {
            let start = self.cur_pos as usize;
            let mut end = start + 1;
            while end < self.text.len() && is_cont(self.text[end]) {
                end += 1;
            }
            let end = end.min(self.text.len());
            if start < self.text.len() {
                self.text.drain(start..end);
            }
            self.last_pos = self.text.len() as i32;
        }
    }

    /// `WMChar` for backspace (8) and delete (127); other characters are
    /// not edit-buffer business. Returns whether the key was an editing key.
    pub fn wm_char(&mut self, c: u8) -> bool {
        if c == 8 || c == 127 {
            if !self.capture_input {
                return true;
            }
            if c == 8 {
                if self.cur_pos > 0 {
                    self.dec_cur_pos();
                    self.del_cur_pos();
                }
            } else {
                self.del_cur_pos();
            }
            return true;
        }
        false
    }

    /// `OnChar`: appends one byte (the C++ casts the character to `char`).
    pub fn on_char(&mut self, c: u32) {
        if self.only_number && !(('0' as u32)..=('9' as u32)).contains(&c) {
            return;
        }
        if !self.is_writable(c) {
            return;
        }
        self.text.push(c as u8);
        self.cur_pos = self.text.len() as i32;
        self.last_pos = self.cur_pos;
    }

    /// `PasteString`.
    pub fn paste_string(&mut self, s: &[u8]) {
        if !self.paste_enabled {
            return;
        }
        self.text.extend_from_slice(s);
        self.cur_pos = self.text.len() as i32;
        self.last_pos = self.cur_pos;
    }

    /// `OnSDLTextEditing`.
    pub fn on_text_editing(&mut self, utf8: &[u8], start: i32, length: i32) {
        self.composition = utf8.to_vec();
        self.comp_len = utf8.len() as i32;
        self.ul_begin = start;
        self.ul_end = start + length;
    }

    /// `OnSDLTextInput`: committed text. Only accepted while input capture is
    /// on; `|` is doubled so it cannot start a tag.
    pub fn on_text_input(&mut self, utf8: &[u8]) {
        self.composition.clear();
        self.comp_len = 0;
        if !self.capture_input {
            return;
        }
        let max_chars = if self.max > 0 && (self.max as usize) < IMESTR_MAXLEN { self.max as usize } else { IMESTR_MAXLEN };
        let mut insert: Vec<u8> = Vec::new();
        let mut length = utf8_length(&self.text);
        let mut i = 0;
        while i < utf8.len() {
            let begin = i;
            let c = decode_utf8(utf8, &mut i);
            if self.only_number && !(('0' as u32)..=('9' as u32)).contains(&c) {
                continue;
            }
            if !self.is_writable(c) {
                continue;
            }
            let copies = if c == '|' as u32 { 2 } else { 1 };
            let mut n = 0;
            while n < copies && length < max_chars {
                insert.extend_from_slice(&utf8[begin..i]);
                n += 1;
                length += 1;
            }
        }
        if insert.is_empty() {
            return;
        }
        if self.cur_pos < 0 || self.cur_pos > self.text.len() as i32 {
            self.cur_pos = self.text.len() as i32;
        }
        let at = self.cur_pos as usize;
        self.text.splice(at..at, insert.iter().copied());
        self.cur_pos += insert.len() as i32;
        self.last_pos = self.text.len() as i32;
    }
}
