//! Scenario interpreter shared by the tests: the Rust twin of
//! `tools/differential/ui/UiDump.cpp` (`RunScenario`). Both read the same
//! script and must print the same lines.
//!
//! One command per line, `#` comments. `ID` is a scenario-chosen small
//! integer. Commands (see the C++ file for the exact semantics):
//! `screen W H [HRES VRES]`, `new ID TYPE LAYER`, `parent C P`, `name`, `pos`,
//! `size`, `halign`, `valign`, `show`, `hide`, `flag ID NAME`, `bias`,
//! `updaterect`, `settop`, `lock`, `unlock`, `focus`, `killfocus [ID]`,
//! `pickalways`, `destroy`, `handler ID Event RET|none`, `mousehandler`,
//! `update`, `tick`, `move|ldown|lup|ldbl|rdown|rup|rdbl|mdown|mup X Y`,
//! `keydown|keyup K`, `esc`, `exitkey`, `ime*`, `attach T I S W H`,
//! `deattach`, `attachflag`, `resetcapture`, `enable|disable ID`,
//! `btn ID up|down|over|setup|flash`, `restrict`, `anidelay`, `pickwin`,
//! `picktop`, `mgrpick`, `query`, `dumptree`, `dump ID`, `dumpmgr`,
//! `dumpslot ID`, `slot_*`, `grid`, `font`, `text ID HEX`, `textalign`,
//! `limitwidth`, `secret`, `multiline`, `fontcolor`, `dumptext`.

#![allow(dead_code)]

pub mod differential;
pub mod randgen;
pub mod text;

use std::collections::HashMap;
use std::fmt::Write;

use m2_ui_core::slot::SlotState;
use m2_ui_core::text::{Glyph, TextInstance, TextMeasure};
use m2_ui_core::window::*;
use m2_ui_core::{UiEvent, UiHost};

pub fn fake_glyph(c: u32) -> Glyph {
    let width = 4 + (c % 7) as i16;
    Glyph { width, height: 10 + (c % 4) as i16, advance: (width + 1) as f32 + if c & 1 == 1 { 0.5 } else { 0.0 } }
}

pub fn hex(b: &[u8]) -> String {
    if b.is_empty() {
        return "-".into();
    }
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub fn unhex(s: &str) -> Vec<u8> {
    if s == "-" {
        return Vec::new();
    }
    (0..s.len() / 2).map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap()).collect()
}

pub fn whex(w: &[u32]) -> String {
    if w.is_empty() {
        return "-".into();
    }
    w.iter().map(|c| format!("{c:x}")).collect::<Vec<_>>().join(",")
}

/// Emulates `CTimer` in custom-time mode (`UseCustomTime` + `Advance`).
#[derive(Default)]
struct Clock {
    index: i32,
    ms: u32,
    sec: f32,
}

impl Clock {
    fn advance(&mut self) {
        self.index += 1;
        if self.index == 1 {
            self.index = -1;
        }
        self.ms += 16 + (self.index & 1) as u32;
        self.sec = self.ms as f32 / 1000.0;
    }
}

pub struct Host {
    /// `(scenario id, method)` -> return value (`None` = non-number).
    handlers: HashMap<(i32, String), Option<i64>>,
    ids: HashMap<WindowId, i32>,
    pub out: String,
    clock: Clock,
}

impl TextMeasure for Host {
    fn glyph(&mut self, _cp: u32, ch: u32) -> Option<Glyph> {
        Some(fake_glyph(ch))
    }
}

impl Host {
    fn key(&self, w: Option<WindowId>) -> i32 {
        match w {
            None => -1,
            Some(w) => self.ids.get(&w).copied().unwrap_or(-2),
        }
    }
}

impl UiHost for Host {
    fn has_handler(&self, window: Option<WindowId>, name: &str) -> bool {
        self.handlers.contains_key(&(self.key(window), name.to_string()))
    }

    fn call(&mut self, ev: &UiEvent) -> Option<i64> {
        let id = self.key(ev.window);
        let args = ev.args.iter().map(|a| a.to_string()).collect::<Vec<_>>().join(",");
        writeln!(self.out, "ev {id} {} {args}", ev.name).unwrap();
        self.handlers[&(id, ev.name.to_string())]
    }

    fn current_second(&mut self) -> f32 {
        self.clock.sec
    }
}

fn name_in(names: &HashMap<WindowId, String>, internal: &HashMap<WindowId, String>, w: Option<WindowId>) -> String {
    match w {
        None => "null".into(),
        Some(w) => names.get(&w).or_else(|| internal.get(&w)).cloned().unwrap_or_else(|| "?".into()),
    }
}

pub struct Scenario {
    pub ui: Ui,
    host: Host,
    by_id: HashMap<i32, WindowId>,
    names: HashMap<WindowId, String>,
    internal: HashMap<WindowId, String>,
    fonts: bool,
    raw: HashMap<WindowId, Vec<u8>>,
}

fn parse_type(t: &str) -> Option<WindowType> {
    Some(match t {
        "window" => WindowType::Window,
        "slot" => WindowType::Slot,
        "gridslot" => WindowType::GridSlot,
        "textline" => WindowType::TextLine,
        "markbox" => WindowType::MarkBox,
        "imagebox" => WindowType::ImageBox,
        "expimagebox" => WindowType::ExpandedImageBox,
        "aniimagebox" => WindowType::AniImageBox,
        "button" => WindowType::Button,
        "radiobutton" => WindowType::RadioButton,
        "togglebutton" => WindowType::ToggleButton,
        "dragbutton" => WindowType::DragButton,
        "box" => WindowType::Box,
        "bar" => WindowType::Bar,
        "line" => WindowType::Line,
        "bar3d" => WindowType::Bar3D,
        "numline" => WindowType::NumberLine,
        _ => return None,
    })
}

fn flag_bit(f: &str) -> Option<(u32, bool)> {
    Some(match f {
        "movable" => (FLAG_MOVABLE, true),
        "limit" => (FLAG_LIMIT, true),
        "dragable" => (FLAG_DRAGABLE, true),
        "attach" => (FLAG_ATTACH, true),
        "restrict_x" => (FLAG_RESTRICT_X, true),
        "restrict_y" => (FLAG_RESTRICT_Y, true),
        "float" => (FLAG_FLOAT, true),
        "not_pick" => (FLAG_NOT_PICK, true),
        "ignore_size" => (FLAG_IGNORE_SIZE, true),
        "rtl" => (FLAG_RTL, true),
        "ltr" => (FLAG_RTL, false),
        _ => return None,
    })
}

impl Default for Scenario {
    fn default() -> Self {
        Self::new()
    }
}

impl Scenario {
    pub fn new() -> Self {
        let ui = Ui::new();
        let mut names = HashMap::new();
        names.insert(ui.root(), "ROOT".to_string());
        for l in LAYERS {
            names.insert(ui.layer(l).unwrap(), format!("L:{l}"));
        }
        let mut s = Self {
            ui,
            host: Host { handlers: HashMap::new(), ids: HashMap::new(), out: String::new(), clock: Clock::default() },
            by_id: HashMap::new(),
            names,
            internal: HashMap::new(),
            fonts: false,
            raw: HashMap::new(),
        };
        s.name_internals();
        s
    }

    fn name_internals(&mut self) {
        self.internal.clear();
        let mut ids: Vec<_> = self.by_id.iter().map(|(k, v)| (*k, *v)).collect();
        ids.sort();
        for (id, w) in ids {
            if !self.ui.is_alive(w) {
                continue;
            }
            if let Some(st) = self.ui.window(w).slot() {
                for sl in &st.slots {
                    let idx = sl.number;
                    if let Some(c) = sl.cover {
                        self.internal.insert(c, format!("{id}.cover.{idx}"));
                    }
                    if let Some(c) = sl.button {
                        self.internal.insert(c, format!("{id}.btn.{idx}"));
                    }
                    if let Some(c) = sl.sign {
                        self.internal.insert(c, format!("{id}.sign.{idx}"));
                    }
                }
            }
        }
    }

    fn name_of(&self, w: Option<WindowId>) -> String {
        name_in(&self.names, &self.internal, w)
    }

    fn list_names(&self, l: &[WindowId]) -> String {
        format!("[{}]", l.iter().map(|w| self.name_of(Some(*w))).collect::<Vec<_>>().join(","))
    }

    fn dump_window(&self, w: WindowId, depth: usize, out: &mut String) {
        let win = self.ui.window(w);
        let (x, y) = win.position();
        let (wd, ht) = win.size();
        let r = win.rect();
        let b = win.limit_bias();
        write!(
            out,
            "w {depth} {} {} show={} pos={x},{y} size={wd},{ht} rect={},{},{},{} flag={:x} ha={} va={} bias={},{},{},{} par={}",
            self.name_of(Some(w)),
            win.window_type().name(),
            win.is_show() as i32,
            r.left,
            r.top,
            r.right,
            r.bottom,
            win.flags(),
            win.h_align(),
            win.v_align(),
            b.left,
            b.right,
            b.top,
            b.bottom,
            self.name_of(win.parent())
        )
        .unwrap();
        use WindowType::*;
        if matches!(win.window_type(), Button | RadioButton | ToggleButton | DragButton | SlotButton | CoverButton) {
            let bs = win.button();
            write!(out, " enable={} pressed={} flash={}", bs.enable as i32, bs.pressed as i32, bs.flash as i32).unwrap();
        }
        if win.window_type() == DragButton {
            let r = win.restrict_area();
            write!(out, " restrict={},{},{},{}", r.left, r.top, r.right, r.bottom).unwrap();
        }
        if win.window_type() == AniImageBox {
            let a = win.ani();
            write!(out, " ani={},{},{}", a.cur_delay, a.delay, a.cur_index).unwrap();
        }
        writeln!(out, " kids={}", win.children().len()).unwrap();
        for &c in win.children() {
            self.dump_window(c, depth + 1, out);
        }
    }

    fn dump_mgr(&self, out: &mut String) {
        let u = &self.ui;
        let (ty, idx, slot, aw, ah) = u.attaching();
        let keys = u.key_captures().map(|(k, w)| format!("{k}={}", self.name_of(Some(w)))).collect::<Vec<_>>().join(",");
        writeln!(
            out,
            "mgr mouse={},{} picked={},{} drag={},{} dragging={} point={} active={} lock={} left={} right={} middle={} attaching={} atype={ty} aindex={idx} aslot={slot} asize={aw},{ah} locklist={} activelist={} pickalways={} reserve={} keycap=[{keys}]",
            u.mouse_position().0,
            u.mouse_position().1,
            u.picked_origin().0,
            u.picked_origin().1,
            u.drag_origin().0,
            u.drag_origin().1,
            u.is_dragging() as i32,
            self.name_of(u.point_window()),
            self.name_of(u.active_window()),
            self.name_of(u.lock_window_current()),
            self.name_of(u.left_capture()),
            self.name_of(u.right_capture()),
            self.name_of(u.middle_capture()),
            u.is_attaching() as i32,
            self.list_names(u.lock_list()),
            self.list_names(u.active_list()),
            self.list_names(u.pick_always_list()),
            self.list_names(u.reserve_delete_list()),
        )
        .unwrap();
    }

    fn dump_slot(&self, w: WindowId, out: &mut String) {
        let Some(st): Option<&SlotState> = self.ui.window(w).slot() else { return };
        let sel = st.selected.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(",");
        write!(
            out,
            "slotwin {} type={} style={} tooltip={:x} usemode={} usable={} selected=[{sel}]",
            self.name_of(Some(w)),
            st.slot_type,
            st.style,
            st.tooltip,
            st.use_mode as i32,
            st.usable_item as i32
        )
        .unwrap();
        if let Some(g) = &st.grid {
            let v = g.cells.iter().map(|i| st.slots[*i].number.to_string()).collect::<Vec<_>>().join(",");
            write!(out, " grid={}x{} vec=[{v}]", g.x_count, g.y_count).unwrap();
        }
        match self.ui.picked_slot(w) {
            Some(i) => write!(out, " picked={}", st.slots[i].number).unwrap(),
            None => write!(out, " picked=-").unwrap(),
        }
        match self.ui.selected_slot_pointer(w) {
            Some(i) => write!(out, " selptr={}", st.slots[i].number).unwrap(),
            None => write!(out, " selptr=-").unwrap(),
        }
        writeln!(out).unwrap();
        for sl in &st.slots {
            writeln!(
                out,
                "s {} state={:x} center={:x} item={} index={} cool={:08x} start={:08x} active={} pos={},{} cell={},{} placed={},{} base={} inst={} num={} cover={} btn={} sign={} fx=0",
                sl.number,
                sl.state,
                sl.center,
                sl.is_item as i32,
                sl.item_index,
                sl.cool_time.to_bits(),
                sl.start_cool_time.to_bits(),
                sl.active as i32,
                sl.x,
                sl.y,
                sl.cell_w,
                sl.cell_h,
                sl.placed_w,
                sl.placed_h,
                sl.render_base_image as i32,
                sl.has_instance as i32,
                sl.number_line.is_some() as i32,
                sl.cover.is_some() as i32,
                sl.button.is_some() as i32,
                sl.sign.is_some() as i32
            )
            .unwrap();
        }
    }

    pub fn run(&mut self, script: &str) -> String {
        let mut result = String::new();
        for line in script.lines() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            writeln!(result, "> {line}").unwrap();
            let a: Vec<&str> = line.split_whitespace().collect();
            self.command(&a, &mut result);
            result.push_str(&std::mem::take(&mut self.host.out));
        }
        result
    }

    fn command(&mut self, a: &[&str], out: &mut String) {
        let i = |n: usize| -> i64 {
            let s = a[n];
            if let Some(h) = s.strip_prefix("0x") { i64::from_str_radix(h, 16).unwrap() } else { s.parse().unwrap() }
        };
        let f = |n: usize| -> f32 { a[n].parse::<f64>().unwrap() as f32 };
        let c = a[0];
        let w: Option<WindowId> = if a.len() > 1 { a[1].parse::<i32>().ok().and_then(|id| self.by_id.get(&id).copied()) } else { None };
        let w2: Option<WindowId> = if a.len() > 2 { a[2].parse::<i32>().ok().and_then(|id| self.by_id.get(&id).copied()) } else { None };
        let is_slot = w.is_some_and(|w| self.ui.window(w).window_type().is_slot());
        let is_text = w.is_some_and(|w| self.ui.window(w).window_type() == WindowType::TextLine);
        let ui = &mut self.ui;
        let host = &mut self.host;
        match (c, w) {
            ("screen", _) => {
                ui.set_screen_size(i(1), i(2));
                if a.len() > 4 {
                    ui.set_resolution(i(3) as i32, i(4) as i32);
                } else {
                    ui.set_resolution(i(1) as i32, i(2) as i32);
                }
            }
            ("new", _) => {
                let id = i(1) as i32;
                let ty = parse_type(a[2]).unwrap();
                let nw = ui.register(ty, a[3]);
                self.by_id.insert(id, nw);
                host.ids.insert(nw, id);
                self.names.insert(nw, id.to_string());
            }
            ("parent", Some(w)) => {
                if let Some(p) = w2 {
                    ui.set_parent(w, p)
                }
            }
            ("name", Some(w)) => ui.set_name(w, a[2]),
            ("pos", Some(w)) => ui.set_position(host, w, i(2), i(3)),
            ("size", Some(w)) => ui.set_size(w, i(2), i(3)),
            ("halign", Some(w)) => ui.set_horizontal_align(host, w, i(2) as i32),
            ("valign", Some(w)) => ui.set_vertical_align(host, w, i(2) as i32),
            ("show", Some(w)) => ui.show(w),
            ("hide", Some(w)) => ui.hide(w),
            ("flag", Some(w)) => {
                if let Some((bit, on)) = flag_bit(a[2]) {
                    if on { ui.add_flag(w, bit) } else { ui.remove_flag(w, bit) }
                }
            }
            ("bias", Some(w)) => ui.set_limit_bias(w, i(2), i(3), i(4), i(5)),
            ("updaterect", Some(w)) => ui.update_rect(host, w),
            ("settop", Some(w)) => ui.set_top(host, w),
            ("lock", Some(w)) => ui.lock_window(host, w),
            ("unlock", _) => ui.unlock_window(host),
            ("focus", Some(w)) => ui.activate_window(host, w),
            ("killfocus", w) => match w {
                Some(w) => {
                    if ui.is_focus_window(w) {
                        ui.deactivate_window(host)
                    }
                }
                None => ui.deactivate_window(host),
            },
            ("pickalways", Some(w)) => ui.set_pick_always(w),
            ("destroy", Some(w)) => {
                ui.destroy_window(w);
                self.by_id.remove(&(i(1) as i32));
            }
            ("handler", Some(_)) => {
                let ret = if a[3] == "none" { None } else { Some(a[3].parse::<i64>().unwrap()) };
                host.handlers.insert((i(1) as i32, a[2].to_string()), ret);
            }
            ("mousehandler", _) => {
                host.handlers.insert((-1, "DeattachObject".into()), Some(0));
                ui.set_mouse_handler(true);
            }
            ("update", _) => {
                ui.update(host);
                // Names of freed windows go with them.
                let alive: Vec<_> = self.names.keys().copied().collect();
                for k in alive {
                    if !ui.is_alive(k) {
                        self.names.remove(&k);
                    }
                }
                self.name_internals();
            }
            ("tick", _) => host.clock.advance(),
            ("move", _) => ui.run_mouse_move(host, i(1), i(2)),
            ("ldown", _) => ui.run_mouse_left_button_down(host, i(1), i(2)),
            ("lup", _) => ui.run_mouse_left_button_up(host, i(1), i(2)),
            ("ldbl", _) => ui.run_mouse_left_button_double_click(host, i(1), i(2)),
            ("rdown", _) => ui.run_mouse_right_button_down(host, i(1), i(2)),
            ("rup", _) => ui.run_mouse_right_button_up(host, i(1), i(2)),
            ("rdbl", _) => ui.run_mouse_right_button_double_click(host, i(1), i(2)),
            ("mdown", _) => ui.run_mouse_middle_button_down(host, i(1), i(2)),
            ("mup", _) => ui.run_mouse_middle_button_up(host, i(1), i(2)),
            ("keydown", _) => ui.run_key_down(host, i(1) as i32),
            ("keyup", _) => ui.run_key_up(host, i(1) as i32),
            ("esc", _) => ui.run_press_escape_key(host),
            ("exitkey", _) => ui.run_press_exit_key(host),
            ("imeupdate", _) => ui.run_ime_update(host),
            ("imetab", _) => ui.run_ime_tab_event_all(host),
            ("imereturn", _) => ui.run_ime_return_event_all(host),
            ("imekeydown", _) => ui.run_ime_key_down(host, i(1)),
            ("imecodepage", _) => ui.run_change_code_page(host),
            ("imeopencand", _) => ui.run_open_candidate(host),
            ("imeclosecand", _) => ui.run_close_candidate(host),
            ("imeopenread", _) => ui.run_open_reading(host),
            ("imecloseread", _) => ui.run_close_reading(host),
            ("attach", _) => ui.attach_icon(i(1) as u32, i(2) as u32, i(3) as u32, i(4) as u8, i(5) as u8),
            ("deattach", _) => ui.deattach_icon(host),
            ("attachflag", _) => ui.set_attaching_flag(i(1) != 0),
            ("resetcapture", _) => ui.reset_capture(),
            ("enable", Some(w)) => ui.button_enable(w),
            ("disable", Some(w)) => ui.button_disable(w),
            ("btn", Some(w)) => match a[2] {
                "up" => ui.button_press_up(host, w),
                "down" => ui.button_press_down(host, w),
                "setup" => ui.button_set_up(w),
                "flash" => ui.button_flash(w),
                _ => {}
            },
            ("restrict", Some(w)) => ui.drag_button_set_restrict_area(w, i(2), i(3), i(4), i(5)),
            ("anidelay", Some(w)) => ui.set_ani_delay(w, i(2) as i32),
            ("pickwin", Some(w)) => {
                let r = ui.pick_window(w, i(2), i(3));
                writeln!(out, "pickwin {} -> {}", name_in(&self.names, &self.internal, Some(w)), name_in(&self.names, &self.internal, r)).unwrap();
            }
            ("picktop", Some(w)) => {
                let r = ui.pick_top_window(w, i(2), i(3));
                writeln!(out, "picktop {} -> {}", name_in(&self.names, &self.internal, Some(w)), name_in(&self.names, &self.internal, r)).unwrap();
            }
            ("mgrpick", _) => {
                let r = ui.pick_at(i(1), i(2));
                writeln!(out, "mgrpick -> {}", name_in(&self.names, &self.internal, r)).unwrap();
            }
            ("query", Some(w)) => {
                let (lx, ly) = ui.get_mouse_local_position(w);
                let ischild = if a.len() > 2 && w2.is_some() { ui.is_child(w, w2.unwrap()) as i32 } else { -1 };
                writeln!(
                    out,
                    "q {} rendering={} root={} parent={} kids={} in={} local={lx},{ly} ischild={ischild}",
                    name_in(&self.names, &self.internal, Some(w)),
                    ui.is_rendering(w) as i32,
                    name_in(&self.names, &self.internal, Some(ui.get_root(w))),
                    name_in(&self.names, &self.internal, ui.window(w).parent()),
                    ui.child_count(w),
                    ui.is_in(w) as i32
                )
                .unwrap();
            }
            ("dumptree", _) => self.dump_window(self.ui.root(), 0, out),
            ("dump", Some(w)) => self.dump_window(w, 0, out),
            ("dumpmgr", _) => self.dump_mgr(out),
            ("dumpslot", Some(w)) => self.dump_slot(w, out),
            _ if c.starts_with("slot_") && is_slot => self.slot_command(c, w.unwrap(), a),
            ("grid", Some(w)) if is_slot => {
                ui.arrange_grid_slot(host, w, i(2) as u32, i(3) as u32, i(4) as u32, i(5), i(6), i(7), i(8));
                self.name_internals();
            }
            ("font", Some(w)) if is_text => {
                ui.text_set_font(w, true);
                self.fonts = true;
            }
            ("text", Some(w)) if is_text => {
                let bytes = unhex(a[2]);
                ui.text_set_text_bytes(host, w, &bytes, Some(65001));
                self.raw.insert(w, bytes);
            }
            ("textalign", Some(w)) if is_text => {
                let t = ui.text_mut(w);
                t.h_align = i(2) as u8;
                t.v_align = i(3) as u8;
            }
            ("limitwidth", Some(w)) if is_text => ui.text_mut(w).limit_width = f(2),
            ("secret", Some(w)) if is_text => ui.text_mut(w).secret = i(2) != 0,
            ("multiline", Some(w)) if is_text => ui.text_mut(w).multi_line = i(2) != 0,
            ("fontcolor", Some(w)) if is_text => ui.text_mut(w).set_color(u32::from_str_radix(a[2], 16).unwrap()),
            ("dumptext", Some(w)) if is_text => {
                let mut t = ui.text_mut(w).clone();
                t.update(host);
                let label = format!("text {}", self.name_of(Some(w)));
                let raw = self.raw.get(&w).cloned().unwrap_or_default();
                dump_text_instance(&label, &t, &raw, out);
                let cursor = self.ui.text_cursor_position(w);
                writeln!(out, "textpos {:08x},{:08x} cursor={cursor}", t.position.0.to_bits(), t.position.1.to_bits()).unwrap();
            }
            _ => writeln!(out, "!! unknown or inapplicable command").unwrap(),
        }
    }

    fn slot_command(&mut self, c: &str, w: WindowId, a: &[&str]) {
        let i = |n: usize| -> i64 { a[n].parse().unwrap() };
        let f = |n: usize| -> f32 { a[n].parse::<f64>().unwrap() as f32 };
        let ui = &mut self.ui;
        let host = &mut self.host;
        match c {
            "slot_append" => ui.append_slot(w, i(2) as u32, i(3), i(4), i(5), i(6)),
            "slot_set" => ui.set_slot(w, i(2) as u32, i(3) as u32, i(4) as u8, i(5) as u8, false),
            "slot_clear" => ui.clear_slot(w, i(2) as u32),
            "slot_clearall" => ui.clear_all_slot(host, w),
            "slot_count" => ui.set_slot_count(w, i(2) as u32, i(3) as u32),
            "slot_countnew" => ui.set_slot_count_new(w, i(2) as u32, i(3) as u32, i(4) as u32),
            "slot_cooltime" => {
                let now = host.clock.sec;
                ui.set_slot_cool_time(w, i(2) as u32, f(3), f(4), now)
            }
            "slot_lock" => ui.lock_slot(w, i(2) as u32),
            "slot_unlock" => ui.unlock_slot(w, i(2) as u32),
            "slot_cantuse" => ui.set_cant_use_slot(w, i(2) as u32),
            "slot_use" => ui.set_use_slot(w, i(2) as u32),
            "slot_enable" => ui.enable_slot(w, i(2) as u32),
            "slot_disable" => ui.disable_slot(w, i(2) as u32),
            "slot_activate" => ui.activate_slot(w, i(2) as u32),
            "slot_deactivate" => ui.deactivate_slot(w, i(2) as u32),
            "slot_select" => ui.select_slot(w, i(2) as u32),
            "slot_clearselected" => ui.clear_selected(w),
            "slot_style" => ui.set_slot_style(w, i(2) as u32),
            "slot_type" => ui.set_slot_type(w, i(2) as u32),
            "slot_usemode" => ui.set_use_mode(w, i(2) != 0),
            "slot_usable" => ui.set_usable_item(w, i(2) != 0),
            "slot_refresh" => ui.refresh_slot(host, w),
            "slot_cover" => {
                ui.set_cover_button(host, w, i(2) as u32, "a.sub", "b.sub", "c.sub", "d.sub", i(3) != 0, i(4) != 0);
                self.name_internals();
            }
            "slot_coverenable" => ui.enable_cover_button(w, i(2) as u32),
            "slot_coverdisable" => ui.disable_cover_button(w, i(2) as u32),
            "slot_alwayscover" => ui.set_always_render_cover_button(w, i(2) as u32, i(3) != 0),
            "slot_showbase" => ui.show_slot_base_image(w, i(2) as u32),
            "slot_hidebase" => ui.hide_slot_base_image(w, i(2) as u32),
            "slot_button" => {
                ui.append_slot_button(host, w, "a.sub", "b.sub", "c.sub");
                self.name_internals();
            }
            "slot_showbutton" => ui.show_slot_button(w, i(2) as u32),
            "slot_hidebuttons" => ui.hide_all_slot_button(w),
            "slot_sign" => {
                ui.append_requirement_sign_image(host, w, "a.sub");
                self.name_internals();
            }
            "slot_showsign" => ui.show_requirement_sign(w, i(2) as u32),
            "slot_hidesign" => ui.hide_requirement_sign(w, i(2) as u32),
            "slot_internalsize" => {
                let st = ui.slot_state(w);
                if let Some(k) = st.find(i(3) as u32) {
                    let sl = &st.slots[k];
                    let t = match a[2] {
                        "cover" => sl.cover,
                        "btn" => sl.button,
                        _ => sl.sign,
                    };
                    if let Some(t) = t {
                        ui.set_size(t, i(4), i(5));
                    }
                }
            }
            _ => {}
        }
    }
}

pub fn dump_text_instance(label: &str, ti: &TextInstance, value: &[u8], out: &mut String) {
    let (tw, th) = ti.text_size();
    write!(out, "{label} w={tw} h={th} lines={} value={} chars=", ti.line_count(), hex(value)).unwrap();
    let n = ti.glyph_count();
    for k in 0..n {
        let (g, color) = ti.glyph(k).unwrap();
        write!(out, "{}{}:{:08x}:{}:{:x}", if k > 0 { "," } else { "" }, g.width, g.advance.to_bits(), g.height, color).unwrap();
    }
    if n == 0 {
        write!(out, "-").unwrap();
    }
    write!(out, " links=").unwrap();
    for (k, l) in ti.hyperlinks().iter().enumerate() {
        write!(out, "{}{}:{}:{}", if k > 0 { ";" } else { "" }, l.sx, l.ex, whex(&l.text)).unwrap();
    }
    if ti.hyperlinks().is_empty() {
        write!(out, "-").unwrap();
    }
    write!(out, " pix=").unwrap();
    let mut p = 0;
    while p < tw + 5 {
        write!(out, "{}{}", if p > 0 { "," } else { "" }, ti.pixel_position_to_character_position(p)).unwrap();
        p += 3;
    }
    write!(out, " tp=").unwrap();
    for k in 0..=n + 1 {
        let (x, y) = ti.text_pos(k);
        write!(out, "{}{:08x}/{:08x}", if k > 0 { "," } else { "" }, x.to_bits(), y.to_bits()).unwrap();
    }
    writeln!(out).unwrap();
}
