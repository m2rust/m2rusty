//! Rust twin of `RunText` in `tools/differential/ui/UiDump.cpp`: the tag
//! parser and the text layout over a corpus of `<code page> <hex bytes>`
//! lines.

#![allow(dead_code)]

use std::fmt::Write;

use m2_formats::utf8;
use m2_ui_core::text::{Glyph, TextInstance, TextMeasure};
use m2_ui_core::text_tag::*;

use super::{dump_text_instance, fake_glyph, unhex, whex};

pub struct Fake;

impl TextMeasure for Fake {
    fn glyph(&mut self, _cp: u32, ch: u32) -> Option<Glyph> {
        Some(fake_glyph(ch))
    }
}

pub fn dump_corpus(corpus: &str) -> String {
    let mut out = String::new();
    for (n, line) in corpus.lines().enumerate() {
        let mut it = line.split_whitespace();
        let cp: u32 = it.next().unwrap().parse().unwrap();
        let bytes = unhex(it.next().unwrap_or("-"));
        let w: Vec<u32> = utf8::decode(cp, &bytes).chars().map(u32::from).collect();
        writeln!(out, "t {n} cp={cp}").unwrap();
        writeln!(out, " w {}", whex(&w)).unwrap();
        let olen = get_text_tag_output_len(&w);
        writeln!(out, " out {} olen={olen}", whex(&get_text_tag_output_string(&w))).unwrap();
        write!(out, " tags").unwrap();
        let mut extra = Vec::new();
        let mut i = 0;
        while i < w.len() {
            let (tag, len) = get_text_tag(&w[i..], &mut extra);
            write!(out, " {i}:{tag}:{len}:{}", if tag == TEXT_TAG_COLOR { whex(&extra) } else { "-".into() }).unwrap();
            i += len;
        }
        write!(out, "\n ipos").unwrap();
        for o in 0..=olen + 1 {
            write!(out, " {}", get_text_tag_internal_pos_from_render_pos(&w, o)).unwrap();
        }
        write!(out, "\n fe").unwrap();
        for p in 0..=w.len() {
            write!(out, " {}", find_color_tag_end_position(&w, p, (w.len() - p) as i32)).unwrap();
        }
        write!(out, "\n fs").unwrap();
        for p in 1..=w.len() {
            write!(out, " {}", find_color_tag_start_position(&w, p - 1, p as i32)).unwrap();
        }
        writeln!(out).unwrap();
        for mode in 0..3 {
            let mut ti = TextInstance::new();
            ti.code_page = cp;
            ti.set_font(true);
            ti.set_color(0xff11_2233);
            ti.limit_width = match mode {
                1 => 60.0,
                2 => 25.0,
                _ => 1600.0,
            };
            ti.secret = mode == 2;
            ti.set_value_bytes(&bytes, Some(cp));
            ti.update(&mut Fake);
            let label = match mode {
                0 => " L1600",
                1 => " L60",
                _ => " S25",
            };
            dump_text_instance(label, &ti, &bytes, &mut out);
        }
    }
    out
}
