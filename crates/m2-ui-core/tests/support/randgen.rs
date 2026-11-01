//! Seeded random scenario generator. Emits only commands that are valid for
//! the window they target (the C++ side does not guard against, e.g., a
//! `btn` command on a plain window).

#![allow(dead_code)]

use std::fmt::Write;

pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03)
    }
    pub fn next(&mut self) -> u64 {
        // xorshift64*
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    pub fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + self.below((hi - lo + 1) as u64) as i64
    }
    pub fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
    pub fn pick<'a, T>(&mut self, v: &'a [T]) -> &'a T {
        &v[self.below(v.len() as u64) as usize]
    }
}

const TYPES: &[&str] = &[
    "window", "window", "window", "box", "bar", "line", "bar3d", "imagebox", "expimagebox", "markbox", "aniimagebox",
    "button", "button", "radiobutton", "togglebutton", "dragbutton", "textline", "numline", "slot", "gridslot",
];
const LAYERS: &[&str] = &["GAME", "UI_BOTTOM", "UI", "UI", "UI", "TOP_MOST", "CURTAIN"];
const FLAGS: &[&str] = &[
    "movable", "limit", "dragable", "attach", "restrict_x", "restrict_y", "float", "float", "not_pick", "ignore_size", "rtl", "ltr",
];
const EVENTS: &[&str] = &[
    "OnMouseLeftButtonDown", "OnMouseLeftButtonDoubleClick", "OnMouseRightButtonDown", "OnMouseRightButtonUp",
    "OnMouseMiddleButtonDown", "OnMouseMiddleButtonUp", "OnMouseDrag", "OnMoveWindow", "OnSetFocus", "OnKillFocus",
    "OnMouseOverIn", "OnMouseOverOut", "OnTop", "OnUpdate", "OnKeyDown", "OnKeyUp", "OnPressEscapeKey", "OnPressExitKey",
    "OnIMEUpdate", "OnIMETab", "OnIMEReturn", "OnIMEKeyDown", "OnIMEChangeCodePage", "OnIMEOpenCandidateList",
    "OnIMECloseCandidateList", "OnIMEOpenReadingWnd", "OnIMECloseReadingWnd", "CallEvent", "DownEvent", "ShowToolTip",
    "HideToolTip", "OnToggleUp", "OnToggleDown", "OnMove", "OnEndFrame", "OnSelectEmptySlot", "OnSelectItemSlot",
    "OnUnselectEmptySlot", "OnUnselectItemSlot", "OnUseSlot", "OnOverInItem", "OnOverOutItem", "OnPressedSlotButton",
];

#[derive(Clone)]
struct W {
    id: i32,
    ty: &'static str,
    slot_numbers: Vec<u32>,
}

fn is_button(t: &str) -> bool {
    matches!(t, "button" | "radiobutton" | "togglebutton" | "dragbutton")
}

const TEXT_PIECES: &[&str] = &[
    "Hello", " ", "world", "|cFFFF0000", "|r", "||", "|Hitem:1:2:3|h", "[Sword]", "|h", "Ünï", "한글", "|cff00ff00x|r", "@1252",
    "@0949", "@9999", "@12a4", "abc", "|", "|c12", "|Hx", "\u{20ac}", "日本", "line", "0123456789",
];

fn mouse(r: &mut Rng, last: &mut (i64, i64)) -> (i64, i64) {
    if r.chance(50) {
        *last = ((last.0 + r.range(-4, 4)).clamp(-30, 830), (last.1 + r.range(-4, 4)).clamp(-30, 630));
    } else {
        *last = (r.range(-10, 810), r.range(-10, 610));
    }
    *last
}

pub fn generate(seed: u64, steps: usize) -> String {
    let mut r = Rng::new(seed);
    let mut o = String::new();
    let mut live: Vec<W> = Vec::new();
    let mut next_id = 1;

    if r.chance(30) {
        writeln!(o, "screen 800 600 {} {}", r.pick(&[640, 800, 1024]), r.pick(&[480, 600, 768])).unwrap();
    } else {
        writeln!(o, "screen 800 600").unwrap();
    }
    if r.chance(60) {
        writeln!(o, "mousehandler").unwrap();
    }
    let n = r.range(4, 14);
    for _ in 0..n {
        let ty = *r.pick(TYPES);
        let id = next_id;
        next_id += 1;
        writeln!(o, "new {id} {ty} {}", r.pick(LAYERS)).unwrap();
        let mut w = W { id, ty, slot_numbers: Vec::new() };
        if !live.is_empty() && r.chance(60) {
            let p = r.pick(&live).id;
            writeln!(o, "parent {id} {p}").unwrap();
        }
        let (wd, ht) = if r.chance(5) { (-1, 20) } else { (r.range(10, 300), r.range(10, 200)) };
        writeln!(o, "size {id} {wd} {ht}").unwrap();
        writeln!(o, "pos {id} {} {}", r.range(-20, 500), r.range(-20, 400)).unwrap();
        if r.chance(25) {
            writeln!(o, "halign {id} {}", r.range(0, 3)).unwrap();
        }
        if r.chance(25) {
            writeln!(o, "valign {id} {}", r.range(0, 3)).unwrap();
        }
        if r.chance(85) {
            writeln!(o, "show {id}").unwrap();
        }
        for _ in 0..r.below(3) {
            writeln!(o, "flag {id} {}", r.pick(FLAGS)).unwrap();
        }
        if r.chance(15) {
            writeln!(o, "bias {id} {} {} {} {}", r.range(0, 30), r.range(0, 30), r.range(0, 30), r.range(0, 30)).unwrap();
        }
        for e in EVENTS {
            if r.chance(35) {
                let ret = *r.pick(&["0", "1", "1", "none"]);
                writeln!(o, "handler {id} {e} {ret}").unwrap();
            }
        }
        match ty {
            "slot" => {
                let count = r.range(1, 8);
                for k in 0..count {
                    let num = k as u32 * 2 + r.below(2) as u32;
                    w.slot_numbers.push(num);
                    writeln!(o, "slot_append {id} {num} {} {} {} {}", (k % 4) * 34, (k / 4) * 34, 32, 32).unwrap();
                }
            }
            "gridslot" => {
                let (xc, yc) = (r.range(1, 5), r.range(1, 5));
                let start = r.range(0, 10) * 10;
                writeln!(o, "grid {id} {start} {xc} {yc} 32 32 {} {}", r.range(0, 2), r.range(0, 2)).unwrap();
                w.slot_numbers = (start as u32..(start + xc * yc) as u32).collect();
            }
            "aniimagebox" => {
                writeln!(o, "anidelay {id} {}", r.range(1, 4)).unwrap();
            }
            "dragbutton" => {
                if r.chance(60) {
                    writeln!(o, "restrict {id} {} {} {} {}", r.range(0, 100), r.range(0, 100), r.range(20, 400), r.range(20, 300)).unwrap();
                }
            }
            "textline" => {
                writeln!(o, "font {id}").unwrap();
            }
            _ => {}
        }
        live.push(w);
    }
    writeln!(o, "dumptree").unwrap();

    let mut last = (r.range(0, 800), r.range(0, 600));
    for _ in 0..steps {
        let kind = r.below(100);
        if live.is_empty() {
            break;
        }
        let w = r.pick(&live).clone();
        let id = w.id;
        match kind {
            0..=19 => {
                let (x, y) = mouse(&mut r, &mut last);
                let cmd = *r.pick(&["move", "move", "move", "ldown", "lup", "ldbl", "rdown", "rup", "rdbl", "mdown", "mup"]);
                writeln!(o, "{cmd} {x} {y}").unwrap();
                if r.chance(40) {
                    writeln!(o, "dumpmgr").unwrap();
                }
            }
            20..=26 => {
                let (x, y) = mouse(&mut r, &mut last);
                writeln!(o, "move {x} {y}\nldown {x} {y}").unwrap();
                for _ in 0..r.range(1, 4) {
                    let (x, y) = mouse(&mut r, &mut last);
                    writeln!(o, "move {} {}", x + 20, y + 20).unwrap();
                }
                writeln!(o, "lup {} {}\ndumpmgr", last.0, last.1).unwrap();
            }
            27..=32 => {
                let k = r.range(1, 5);
                let cmd = *r.pick(&["keydown", "keyup", "keydown", "keyup"]);
                writeln!(o, "{cmd} {k}").unwrap();
                if r.chance(20) {
                    writeln!(o, "{}", r.pick(&["esc", "exitkey", "imeupdate", "imetab", "imereturn", "imecodepage", "imeopencand", "imeclosecand", "imeopenread", "imecloseread"])).unwrap();
                }
                if r.chance(15) {
                    writeln!(o, "imekeydown {k}").unwrap();
                }
            }
            33..=38 => {
                let cmd = *r.pick(&["lock", "unlock", "focus", "killfocus", "settop", "settop", "pickalways", "resetcapture"]);
                match cmd {
                    "lock" | "focus" | "settop" | "pickalways" => writeln!(o, "{cmd} {id}").unwrap(),
                    "killfocus" if r.chance(50) => writeln!(o, "killfocus {id}").unwrap(),
                    _ => writeln!(o, "{cmd}").unwrap(),
                }
                writeln!(o, "dumpmgr").unwrap();
            }
            39..=41 => {
                if r.chance(60) {
                    writeln!(o, "attach {} {} {} {} {}", r.below(3), r.below(100), r.below(20), r.range(1, 2), r.range(1, 3)).unwrap();
                } else {
                    writeln!(o, "{}", r.pick(&["deattach", "attachflag 0", "attachflag 1"])).unwrap();
                }
            }
            42..=55 => match r.below(10) {
                0 => writeln!(o, "pos {id} {} {}", r.range(-30, 600), r.range(-30, 450)).unwrap(),
                1 => writeln!(o, "size {id} {} {}", if r.chance(5) { -2 } else { r.range(5, 320) }, r.range(5, 220)).unwrap(),
                2 => writeln!(o, "halign {id} {}", r.range(0, 3)).unwrap(),
                3 => writeln!(o, "valign {id} {}", r.range(0, 3)).unwrap(),
                4 => writeln!(o, "{} {id}", if r.chance(50) { "show" } else { "hide" }).unwrap(),
                5 => writeln!(o, "flag {id} {}", r.pick(FLAGS)).unwrap(),
                6 => {
                    let p = r.pick(&live).id;
                    if p < id {
                        writeln!(o, "parent {id} {p}").unwrap();
                    }
                }
                7 => writeln!(o, "bias {id} {} {} {} {}", r.range(0, 30), r.range(0, 30), r.range(0, 30), r.range(0, 30)).unwrap(),
                8 => writeln!(o, "updaterect {id}").unwrap(),
                _ => writeln!(o, "query {id} {}", r.pick(&live).id).unwrap(),
            },
            56..=63 if is_button(w.ty) => match r.below(6) {
                0 => writeln!(o, "enable {id}").unwrap(),
                1 => writeln!(o, "disable {id}").unwrap(),
                2 => writeln!(o, "btn {id} {}", r.pick(&["up", "down", "setup", "flash", "over"])).unwrap(),
                3 if w.ty == "dragbutton" => writeln!(o, "restrict {id} {} {} {} {}", r.range(0, 100), r.range(0, 100), r.range(20, 400), r.range(20, 300)).unwrap(),
                _ => writeln!(o, "dump {id}").unwrap(),
            },
            64..=75 if w.ty == "slot" || w.ty == "gridslot" => {
                let nums: Vec<u32> = w.slot_numbers.clone();
                let num = *r.pick(&nums);
                let c = r.below(30);
                match c {
                    0..=3 => writeln!(o, "slot_set {id} {num} {} {} {}", r.range(1, 50), r.range(1, 2), r.range(1, 3)).unwrap(),
                    4 => writeln!(o, "slot_clear {id} {num}").unwrap(),
                    5 if w.ty == "slot" && r.chance(30) => writeln!(o, "slot_clearall {id}").unwrap(),
                    6 => writeln!(o, "slot_count {id} {num} {}", r.range(0, 99)).unwrap(),
                    7 => writeln!(o, "slot_countnew {id} {num} {} {}", r.range(0, 3), r.range(0, 99)).unwrap(),
                    8 => writeln!(o, "slot_cooltime {id} {num} {} {}", r.range(1, 30), r.range(0, 5)).unwrap(),
                    9 => writeln!(o, "slot_lock {id} {num}").unwrap(),
                    10 => writeln!(o, "slot_unlock {id} {num}").unwrap(),
                    11 => writeln!(o, "slot_cantuse {id} {num}").unwrap(),
                    12 => writeln!(o, "slot_use {id} {num}").unwrap(),
                    13 => writeln!(o, "slot_enable {id} {num}").unwrap(),
                    14 => writeln!(o, "slot_disable {id} {num}").unwrap(),
                    15 => writeln!(o, "slot_activate {id} {num}").unwrap(),
                    16 => writeln!(o, "slot_deactivate {id} {num}").unwrap(),
                    17 | 18 => writeln!(o, "slot_select {id} {num}").unwrap(),
                    19 => writeln!(o, "slot_clearselected {id}").unwrap(),
                    20 => writeln!(o, "slot_style {id} {}", r.range(0, 2)).unwrap(),
                    21 => writeln!(o, "slot_usemode {id} {}\nslot_usable {id} {}", r.range(0, 1), r.range(0, 1)).unwrap(),
                    22 => {
                        writeln!(o, "slot_cover {id} {num} {} {}", r.range(0, 1), r.range(0, 1)).unwrap();
                        if r.chance(70) {
                            writeln!(o, "slot_internalsize {id} cover {num} {} {}", r.range(8, 40), r.range(8, 40)).unwrap();
                        }
                    }
                    23 => writeln!(o, "{} {id} {num}", r.pick(&["slot_coverenable", "slot_coverdisable"])).unwrap(),
                    24 => writeln!(o, "slot_alwayscover {id} {num} {}", r.range(0, 1)).unwrap(),
                    25 => {
                        writeln!(o, "slot_button {id}").unwrap();
                        writeln!(o, "slot_showbutton {id} {num}").unwrap();
                        if r.chance(60) {
                            writeln!(o, "slot_internalsize {id} btn {num} {} {}", r.range(4, 12), r.range(4, 12)).unwrap();
                        }
                    }
                    26 => {
                        writeln!(o, "slot_sign {id}").unwrap();
                        writeln!(o, "slot_showsign {id} {num}").unwrap();
                    }
                    27 => writeln!(o, "slot_refresh {id}").unwrap(),
                    28 => writeln!(o, "{} {id} {num}", r.pick(&["slot_showbase", "slot_hidebase", "slot_hidesign", "slot_hidebuttons"])).unwrap(),
                    _ => writeln!(o, "slot_type {id} {}", r.range(0, 5)).unwrap(),
                }
                if r.chance(30) {
                    writeln!(o, "dumpslot {id}").unwrap();
                }
            }
            76..=82 if w.ty == "textline" => {
                match r.below(8) {
                    0..=2 => {
                        let mut s = String::new();
                        for _ in 0..r.range(0, 8) {
                            s.push_str(r.pick(TEXT_PIECES));
                        }
                        writeln!(o, "text {id} {}", super::hex(s.as_bytes())).unwrap();
                    }
                    3 => writeln!(o, "textalign {id} {} {}", r.pick(&[1, 2, 3]), r.pick(&[16, 32, 48])).unwrap(),
                    4 => writeln!(o, "limitwidth {id} {}", r.range(10, 200)).unwrap(),
                    5 => writeln!(o, "secret {id} {}", r.range(0, 1)).unwrap(),
                    6 => writeln!(o, "fontcolor {id} ff{:06x}", r.below(0xffffff)).unwrap(),
                    _ => writeln!(o, "multiline {id} {}", r.range(0, 1)).unwrap(),
                }
                writeln!(o, "dumptext {id}").unwrap();
            }
            83..=86 => {
                for _ in 0..r.range(1, 5) {
                    writeln!(o, "tick").unwrap();
                }
                writeln!(o, "update").unwrap();
            }
            87..=88 if live.len() > 3 => {
                writeln!(o, "destroy {id}").unwrap();
                live.retain(|x| x.id != id);
                writeln!(o, "dumpmgr").unwrap();
            }
            89..=91 => writeln!(o, "dumptree").unwrap(),
            92..=94 => {
                let (x, y) = mouse(&mut r, &mut last);
                writeln!(o, "{} {id} {x} {y}", r.pick(&["pickwin", "picktop"])).unwrap();
                writeln!(o, "mgrpick {x} {y}").unwrap();
            }
            _ => writeln!(o, "dump {id}").unwrap(),
        }
    }
    writeln!(o, "dumptree\ndumpmgr").unwrap();
    for w in &live {
        if w.ty == "slot" || w.ty == "gridslot" {
            writeln!(o, "dumpslot {}", w.id).unwrap();
        }
        if w.ty == "textline" {
            writeln!(o, "dumptext {}", w.id).unwrap();
        }
    }
    o
}
