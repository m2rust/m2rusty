//! Font-independent part of `CGraphicTextInstance` (`GrpTextInstance.cpp`):
//! tag walking, hyperlink boxes, secret mode, line wrapping and cursor maths.
//! Glyph metrics come from a [`TextMeasure`], so a Bevy font backend plugs in
//! later; nothing here draws.
//!
//! Not ported: the Arabic branch of `Update` (right-to-left reordering and
//! shaping; `m2_formats::locale::arabic` has the shaping tables), and
//! `Render` itself. [`TextInstance::layout`] reproduces the position
//! arithmetic of `Render` for a renderer to use; that part is checked by unit
//! tests only because `Render` cannot run without a GPU.

use crate::text_tag::*;

pub const CP_UTF8: u32 = 65001;

/// Metrics of one glyph (`CGraphicFontTexture::TCharacterInfomation`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Glyph {
    pub width: i16,
    pub height: i16,
    pub advance: f32,
}

/// Font backend. `code_page` is the key the C++ font texture uses for its
/// per-code-page fonts; `None` means the glyph could not be produced.
pub trait TextMeasure {
    fn glyph(&mut self, code_page: u32, ch: u32) -> Option<Glyph>;
}

pub const HORIZONTAL_ALIGN_LEFT: u8 = 0x01;
pub const HORIZONTAL_ALIGN_CENTER: u8 = 0x02;
pub const HORIZONTAL_ALIGN_RIGHT: u8 = 0x03;
pub const VERTICAL_ALIGN_TOP: u8 = 0x10;
pub const VERTICAL_ALIGN_CENTER: u8 = 0x20;
pub const VERTICAL_ALIGN_BOTTOM: u8 = 0x30;

/// One clickable hyperlink box (`SHyperlink`; the `short` fields wrap).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hyperlink {
    pub sx: i16,
    pub ex: i16,
    pub text: Vec<u32>,
}

/// One laid-out glyph for a renderer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacedGlyph {
    pub index: usize,
    pub x: f32,
    pub y: f32,
    pub color: u32,
}

#[derive(Debug, Clone, Copy)]
struct Placed {
    glyph: Glyph,
    color: u32,
}

#[derive(Debug, Clone)]
pub struct TextInstance {
    text: String,
    /// `(code page, characters)` between `@NNNN` code page tokens.
    segments: Vec<(u32, Vec<u32>)>,
    pub code_page: u32,
    text_color: u32,
    pub outline_color: u32,
    text_width: u16,
    text_height: u16,
    pub h_align: u8,
    pub v_align: u8,
    pub max: u16,
    pub limit_width: f32,
    pub cursor: bool,
    pub secret: bool,
    pub multi_line: bool,
    pub outline: bool,
    pub position: (f32, f32, f32),
    has_font: bool,
    is_update: bool,
    placed: Vec<Placed>,
    hyperlinks: Vec<Hyperlink>,
}

impl Default for TextInstance {
    fn default() -> Self {
        Self {
            text: String::new(),
            segments: Vec::new(),
            code_page: CP_UTF8,
            text_color: 0,
            outline_color: 0xff00_0000,
            text_width: 0,
            text_height: 0,
            h_align: HORIZONTAL_ALIGN_LEFT,
            v_align: VERTICAL_ALIGN_TOP,
            max: 0,
            limit_width: 1600.0,
            cursor: false,
            secret: false,
            multi_line: false,
            outline: false,
            position: (0.0, 0.0, 0.0),
            has_font: false,
            is_update: false,
            placed: Vec::new(),
            hyperlinks: Vec::new(),
        }
    }
}

fn is_digit(c: u32) -> bool {
    (b'0' as u32..=b'9' as u32).contains(&c)
}

/// `FindToken`: index of the next valid `@NNNN` (needs 6 characters left), or
/// `s.len()`.
fn find_token(s: &[u32], begin: usize) -> usize {
    let end = s.len();
    let mut b = begin;
    while b < end {
        while b < end && s[b] != '@' as u32 {
            b += 1;
        }
        if end - b > 5 && s[b] == '@' as u32 && s[b + 1..b + 5].iter().all(|&c| is_digit(c)) {
            return b;
        }
        b += 1;
    }
    end
}

/// `ReadToken`: 9999 means UTF-8.
fn read_token(s: &[u32]) -> u32 {
    let n = (s[1] - '0' as u32) * 1000 + (s[2] - '0' as u32) * 100 + (s[3] - '0' as u32) * 10 + (s[4] - '0' as u32);
    if n == 9999 { CP_UTF8 } else { n }
}

impl TextInstance {
    pub fn new() -> Self {
        Self::default()
    }

    /// `Destroy` + `__Initialize`.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Attaches a font (`SetTextPointer`); without one `update` does nothing.
    pub fn set_font(&mut self, has_font: bool) {
        self.has_font = has_font;
    }

    pub fn has_font(&self) -> bool {
        self.has_font
    }

    /// `SetValue` with UTF-8 text. `@NNNN` tokens switch the code page key
    /// used for glyph lookup and are not displayed.
    pub fn set_value(&mut self, text: &str) {
        if text == self.text {
            return;
        }
        let wide = to_wide(text);
        let mut segments = Vec::new();
        let mut cp = self.code_page;
        let mut begin = 0;
        while begin < wide.len() {
            let token = find_token(&wide, begin);
            segments.push((cp, wide[begin..token].to_vec()));
            if token < wide.len() {
                cp = read_token(&wide[token..]);
                begin = token + 5;
            } else {
                begin = token;
            }
        }
        self.text = text.to_string();
        self.segments = segments;
        self.is_update = false;
    }

    /// The legacy entry point (decision D14): `bytes` in `code_page` (or the
    /// instance's default when `None`), split at `@NNNN` tokens like C++ and
    /// converted to UTF-8 once, here.
    pub fn set_value_bytes(&mut self, bytes: &[u8], code_page: Option<u32>) {
        let default_cp = code_page.unwrap_or(self.code_page);
        let end = bytes.len();
        let mut segments = Vec::new();
        let mut text = String::new();
        let mut cp = default_cp;
        let mut begin = 0;
        while begin < end {
            let wide: Vec<u32> = bytes.iter().map(|&b| b as u32).collect();
            let token = find_token(&wide, begin);
            let chars: Vec<u32> = m2_formats::utf8::decode(cp, &bytes[begin..token])
                .chars()
                .map(u32::from)
                .collect();
            text.push_str(&from_wide(&chars));
            segments.push((cp, chars));
            if token < end {
                text.push_str(&String::from_utf8_lossy(&bytes[token..token + 5]));
                cp = read_token(&wide[token..]);
                begin = token + 5;
            } else {
                begin = token;
            }
        }
        if text == self.text && self.segments == segments {
            return;
        }
        self.text = text;
        self.segments = segments;
        self.code_page = default_cp;
        self.is_update = false;
    }

    pub fn value(&self) -> &str {
        &self.text
    }

    pub fn set_color(&mut self, color: u32) {
        if self.text_color != color {
            for p in &mut self.placed {
                if p.color == self.text_color {
                    p.color = color;
                }
            }
            self.text_color = color;
        }
    }

    pub fn color(&self) -> u32 {
        self.text_color
    }

    pub fn text_size(&self) -> (i32, i32) {
        (self.text_width as i32, self.text_height as i32)
    }

    pub fn hyperlinks(&self) -> &[Hyperlink] {
        &self.hyperlinks
    }

    pub fn glyph_count(&self) -> usize {
        self.placed.len()
    }

    /// `(glyph, colour)` of the i-th laid-out character.
    pub fn glyph(&self, i: usize) -> Option<(Glyph, u32)> {
        self.placed.get(i).map(|p| (p.glyph, p.color))
    }

    fn draw_char(&mut self, m: &mut dyn TextMeasure, cp: u32, ch: u32, color: u32) -> i32 {
        if let Some(g) = m.glyph(cp, ch) {
            self.placed.push(Placed { glyph: g, color });
            self.text_width = (self.text_width as f32 + g.advance) as u16;
            self.text_height = (g.height as i32).max(self.text_height as i32) as u16;
            g.advance as i32
        } else {
            0
        }
    }

    /// `Update` (non-Arabic branch).
    pub fn update(&mut self, m: &mut dyn TextMeasure) {
        if self.is_update || !self.has_font {
            return;
        }
        let def_cp = self.code_page;
        let space_height = m.glyph(def_cp, ' ' as u32).map(|g| g.height as i32).unwrap_or(12);
        self.placed.clear();
        self.hyperlinks.clear();
        self.text_width = 0;
        self.text_height = space_height as u16;

        let mut color = self.text_color;
        let segments = std::mem::take(&mut self.segments);
        for (cp, wide) in &segments {
            if self.secret {
                for _ in 0..wide.len() {
                    self.draw_char(m, *cp, '*' as u32, color);
                }
                continue;
            }
            let mut x: i32 = 0;
            let mut step = 0;
            let mut link = Hyperlink { sx: 0, ex: 0, text: Vec::new() };
            let mut buffer: Vec<u32> = Vec::new();
            let mut i = 0;
            while i < wide.len() {
                let (tag, len) = get_text_tag(&wide[i..], &mut buffer);
                if tag == TEXT_TAG_PLAIN || tag == TEXT_TAG_TAG {
                    if step == 1 {
                        buffer.push(wide[i]);
                    } else {
                        let w = self.draw_char(m, *cp, wide[i], color);
                        link.ex = (link.ex as i32).wrapping_add(w) as i16;
                        x += w;
                    }
                } else if tag == TEXT_TAG_COLOR {
                    color = htoi(&buffer) as u32;
                } else if tag == TEXT_TAG_RESTORE_COLOR {
                    color = self.text_color;
                } else if tag == TEXT_TAG_HYPERLINK_START {
                    step = 1;
                    buffer.clear();
                } else if tag == TEXT_TAG_HYPERLINK_END {
                    if step == 1 {
                        step += 1;
                        link.sx = x as i16;
                        link.ex = x as i16;
                    } else {
                        link.text = std::mem::take(&mut buffer);
                        self.hyperlinks.push(link.clone());
                        step = 0;
                        buffer.clear();
                    }
                }
                i += len;
            }
        }
        self.segments = segments;
        self.is_update = true;
    }

    /// `GetTextLineCount`.
    pub fn line_count(&self) -> u16 {
        let mut fx = 0.0f32;
        let mut lines: u16 = 1;
        for p in &self.placed {
            if fx + p.glyph.width as f32 > self.limit_width {
                fx = 0.0;
                lines = lines.wrapping_add(1);
            }
            fx += p.glyph.advance;
        }
        lines
    }

    /// `PixelPositionToCharacterPosition`: -1 past the end.
    pub fn pixel_position_to_character_position(&self, pixel: i32) -> i32 {
        let mut cur = 0i32;
        for (i, p) in self.placed.iter().enumerate() {
            cur += p.glyph.width as i32;
            if pixel < cur {
                return i as i32;
            }
        }
        -1
    }

    /// `__GetTextPos`: top-left of the glyph at `index` (clamped), wrapping at
    /// the limit width.
    pub fn text_pos(&self, index: usize) -> (f32, f32) {
        let index = index.min(self.placed.len());
        let (mut sx, mut sy, mut max_h) = (0.0f32, 0.0f32, 0.0f32);
        for p in &self.placed[..index] {
            if sx + p.glyph.width as f32 > self.limit_width {
                sx = 0.0;
                sy += max_h;
            }
            sx += p.glyph.advance;
            max_h = max_h.max(p.glyph.height as f32);
        }
        (sx, sy)
    }

    /// The position arithmetic of `Render` (main text pass): where each glyph
    /// goes for a text at `self.position`. `clip_top` skips glyph rows at or
    /// above it, as the `pClipRect` argument does. Like C++, a new line
    /// advances by the height of the glyph that wrapped, not the line's tallest
    /// glyph.
    pub fn layout(&self, clip_top: Option<f32>) -> Vec<PlacedGlyph> {
        let (px, py, _) = self.position;
        let mut stan_x = px;
        let mut stan_y = py + 1.0;
        match self.h_align {
            HORIZONTAL_ALIGN_RIGHT => stan_x -= self.text_width as f32,
            HORIZONTAL_ALIGN_CENTER => stan_x -= (self.text_width / 2) as f32,
            _ => {}
        }
        match self.v_align {
            VERTICAL_ALIGN_BOTTOM => stan_y -= self.text_height as f32,
            VERTICAL_ALIGN_CENTER => stan_y -= self.text_height as f32 / 2.0,
            _ => {}
        }
        let (mut cur_x, mut cur_y) = (stan_x, stan_y);
        let mut out = Vec::new();
        for (i, p) in self.placed.iter().enumerate() {
            let w = p.glyph.width as f32;
            let h = p.glyph.height as f32;
            if (cur_x + w) - px > self.limit_width {
                if self.multi_line {
                    cur_x = stan_x;
                    cur_y += h;
                } else {
                    break;
                }
            }
            if let Some(top) = clip_top
                && cur_y <= top
            {
                cur_x += p.glyph.advance;
                continue;
            }
            out.push(PlacedGlyph { index: i, x: cur_x - 0.5, y: cur_y - 0.5, color: p.color });
            cur_x += p.glyph.advance;
        }
        out
    }
}
