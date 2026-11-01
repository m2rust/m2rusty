//! Port of `neoeter/src/render/TextTag.cpp`: the `|cAARRGGBB`, `|r`, `||`,
//! `|H...|h` markup of chat and interface strings.
//!
//! Strings are slices of `u32` code points (Linux `wchar_t`), as in C++.
//! Positions are code point indices. Callers that pass a position/length pair
//! to the `find_*` functions must keep them inside the buffer, as the C++ IME
//! does; reads that C++ would make outside the buffer return "no match" here.

pub const TEXT_TAG_PLAIN: i32 = 0;
/// `||`
pub const TEXT_TAG_TAG: i32 = 1;
/// `|cffffffff`
pub const TEXT_TAG_COLOR: i32 = 2;
/// `|H`
pub const TEXT_TAG_HYPERLINK_START: i32 = 3;
/// `|h`
pub const TEXT_TAG_HYPERLINK_END: i32 = 4;
pub const TEXT_TAG_RESTORE_COLOR: i32 = 5;

const BAR: u32 = '|' as u32;

/// `GetTextTag`: classifies the tag at the start of `src` (the whole rest of
/// the string). Returns `(tag, tag_len)`. For [`TEXT_TAG_COLOR`], `extra_info`
/// receives the 8 hex digits; otherwise it is left untouched.
pub fn get_text_tag(src: &[u32], extra_info: &mut Vec<u32>) -> (i32, usize) {
    let max_len = src.len();
    if max_len < 2 || src[0] != BAR {
        return (TEXT_TAG_PLAIN, 1);
    }
    match char::from_u32(src[1]) {
        Some('c') => {
            if max_len < 10 {
                return (TEXT_TAG_PLAIN, 1);
            }
            extra_info.clear();
            extra_info.extend_from_slice(&src[2..10]);
            (TEXT_TAG_COLOR, 10)
        }
        Some('|') => (TEXT_TAG_TAG, 2),
        Some('r') => (TEXT_TAG_RESTORE_COLOR, 2),
        Some('H') => (TEXT_TAG_HYPERLINK_START, 2),
        Some('h') => (TEXT_TAG_HYPERLINK_END, 2),
        _ => (TEXT_TAG_PLAIN, 1),
    }
}

/// `GetTextTagOutputString`: the text as displayed (tags removed, hyperlink
/// targets dropped, `||` kept as one `|`... as C++ does: the *first* `|` of
/// `||` is the character that is copied).
pub fn get_text_tag_output_string(src: &[u32]) -> Vec<u32> {
    let mut dst = Vec::new();
    let mut extra = Vec::new();
    let mut hyperlink_step = 0;
    let mut i = 0;
    while i < src.len() {
        let (tag, len) = get_text_tag(&src[i..], &mut extra);
        if tag == TEXT_TAG_PLAIN || tag == TEXT_TAG_TAG {
            if hyperlink_step == 0 {
                dst.push(src[i]);
            }
        } else if tag == TEXT_TAG_HYPERLINK_START {
            hyperlink_step = 1;
        } else if tag == TEXT_TAG_HYPERLINK_END {
            hyperlink_step = 0;
        }
        i += len;
    }
    dst
}

/// `GetTextTagInternalPosFromRenderPos`: index in `src` of the character that
/// is displayed at position `offset`.
pub fn get_text_tag_internal_pos_from_render_pos(src: &[u32], offset: i32) -> i32 {
    let mut extra = Vec::new();
    let mut output_len = 0;
    let mut hyperlink_step = 0;
    let mut color_tag = false;
    let mut internal_offset = 0;
    let mut i = 0;
    while i < src.len() {
        let (tag, len) = get_text_tag(&src[i..], &mut extra);
        if tag == TEXT_TAG_COLOR {
            color_tag = true;
            internal_offset = i as i32;
        } else if tag == TEXT_TAG_RESTORE_COLOR {
            color_tag = false;
        } else if tag == TEXT_TAG_PLAIN || tag == TEXT_TAG_TAG {
            if hyperlink_step == 0 {
                if !color_tag {
                    internal_offset = i as i32;
                }
                if offset <= output_len {
                    return internal_offset;
                }
                output_len += 1;
            }
        } else if tag == TEXT_TAG_HYPERLINK_START {
            hyperlink_step = 1;
        } else if tag == TEXT_TAG_HYPERLINK_END {
            hyperlink_step = 0;
        }
        i += len;
    }
    internal_offset
}

/// `GetTextTagOutputLen`: number of displayed characters.
pub fn get_text_tag_output_len(src: &[u32]) -> i32 {
    let mut extra = Vec::new();
    let mut output_len = 0;
    let mut hyperlink_step = 0;
    let mut i = 0;
    while i < src.len() {
        let (tag, len) = get_text_tag(&src[i..], &mut extra);
        if tag == TEXT_TAG_PLAIN || tag == TEXT_TAG_TAG {
            if hyperlink_step == 0 {
                output_len += 1;
            }
        } else if tag == TEXT_TAG_HYPERLINK_START {
            hyperlink_step = 1;
        } else if tag == TEXT_TAG_HYPERLINK_END {
            hyperlink_step = 0;
        }
        i += len;
    }
    output_len
}

/// `FindColorTagStartPosition(buf + pos, src_len)`: the IME calls it with
/// `pos = cursor - 1` and `src_len = cursor`. Returns how many characters
/// before `pos` the tag run that ends at `pos` starts.
pub fn find_color_tag_start_position(buf: &[u32], pos: usize, src_len: i32) -> i32 {
    if src_len < 2 {
        return 0;
    }
    let at = |i: isize| -> Option<u32> {
        if i < 0 { None } else { buf.get(i as usize).copied() }
    };
    let mut cur = pos as isize;
    let c = at(cur);
    if c == Some('r' as u32) && at(cur - 1) == Some(BAR) {
        let mut len = src_len;
        if len >= 2 && at(cur - 2) == Some(BAR) {
            return 1;
        }
        cur -= 2;
        len -= 2;
        while len > 1 {
            if at(cur) == Some('c' as u32) && at(cur - 1) == Some(BAR) {
                return (pos as isize - cur) as i32 + 1;
            }
            cur -= 1;
            len -= 1;
        }
        return src_len;
    }
    if c == Some(BAR) && at(cur - 1) == Some(BAR) {
        return 1;
    }
    0
}

/// `FindColorTagEndPosition(buf + pos, src_len)`.
pub fn find_color_tag_end_position(buf: &[u32], pos: usize, src_len: i32) -> i32 {
    let at = |i: usize| buf.get(i).copied();
    let mut cur = pos;
    if src_len >= 4 && at(cur) == Some(BAR) && at(cur + 1) == Some('c' as u32) {
        let mut left = src_len - 2;
        cur += 2;
        while left > 1 {
            if at(cur) == Some(BAR) && at(cur + 1) == Some('r' as u32) {
                return (cur - pos) as i32 + 1;
            }
            left -= 1;
            cur += 1;
        }
    } else if src_len >= 2 && at(cur) == Some(BAR) && at(cur + 1) == Some(BAR) {
        return 1;
    }
    0
}

/// `htoi(const wchar_t *s, 8)`: 8 hex digits to a number, `-1` on a bad digit.
pub fn htoi(s: &[u32]) -> i32 {
    let mut x: i32 = 0;
    let mut y: i32 = 1;
    for &c in s.iter().rev() {
        let d = match char::from_u32(c) {
            Some(ch @ '0'..='9') => ch as i32 - '0' as i32,
            Some(ch @ 'a'..='f') => ch as i32 - 'a' as i32 + 10,
            Some(ch @ 'A'..='F') => ch as i32 - 'A' as i32 + 10,
            _ => return -1,
        };
        x = x.wrapping_add(y.wrapping_mul(d));
        y = y.wrapping_shl(4);
    }
    x
}

/// UTF-8 to the `u32` code points used here.
pub fn to_wide(s: &str) -> Vec<u32> {
    s.chars().map(|c| c as u32).collect()
}

/// Back to UTF-8; invalid code points become U+FFFD.
pub fn from_wide(w: &[u32]) -> String {
    w.iter().map(|&c| char::from_u32(c).unwrap_or('\u{fffd}')).collect()
}

/// A string with tags, as a segment list (for renderers that want runs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    /// Displayed text with the colour active for it (`None` = default).
    Text { text: String, color: Option<u32> },
    /// A hyperlink: displayed text and the `|H` target.
    Link { text: String, target: String, color: Option<u32> },
}

/// Parses tags into runs with the same state machine as the text instance
/// (colour via `|c`/`|r`, link target between `|H` and the first `|h`, link
/// text up to the second `|h`).
pub fn parse_segments(src: &[u32]) -> Vec<Segment> {
    let mut out: Vec<Segment> = Vec::new();
    let mut extra = Vec::new();
    let mut color: Option<u32> = None;
    let mut step = 0;
    let mut target: Vec<u32> = Vec::new();
    let mut link_text: Vec<u32> = Vec::new();
    let mut run: Vec<u32> = Vec::new();
    let mut i = 0;
    macro_rules! flush {
        () => {
            if !run.is_empty() {
                out.push(Segment::Text { text: from_wide(&run), color });
                run.clear();
            }
        };
    }
    while i < src.len() {
        let (tag, len) = get_text_tag(&src[i..], &mut extra);
        match tag {
            TEXT_TAG_PLAIN | TEXT_TAG_TAG => match step {
                1 => target.push(src[i]),
                2 => link_text.push(src[i]),
                _ => run.push(src[i]),
            },
            TEXT_TAG_COLOR => {
                flush!();
                color = Some(htoi(&extra) as u32);
            }
            TEXT_TAG_RESTORE_COLOR => {
                flush!();
                color = None;
            }
            TEXT_TAG_HYPERLINK_START => {
                flush!();
                step = 1;
                target.clear();
                link_text.clear();
            }
            _ => {
                if step == 1 {
                    step = 2;
                } else {
                    out.push(Segment::Link { text: from_wide(&link_text), target: from_wide(&target), color });
                    step = 0;
                    target.clear();
                    link_text.clear();
                }
            }
        }
        i += len;
    }
    flush!();
    out
}
