//! Window tree and window manager: `PythonWindow.cpp` +
//! `PythonWindowManager.cpp`.
//!
//! Windows live in an arena addressed by [`WindowId`] (index + generation);
//! parent/child links are ids, there is no shared ownership. C++ virtual
//! dispatch (`OnMouseLeftButtonDown`, `OnUpdate`, ...) becomes a `match` on
//! [`WindowType`]. Drawing is not ported; what the C++ `OnRender` bodies do
//! besides drawing (nothing the scripts can observe) is left to the renderer.

use std::collections::BTreeMap;

use crate::event::{UiEvent, UiHost};
use crate::slot::SlotState;
use crate::text::TextInstance;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WindowId {
    index: u32,
    generation: u32,
}

impl WindowId {
    pub fn index(self) -> u32 {
        self.index
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindowType {
    Window,
    Layer,
    Slot,
    GridSlot,
    TextLine,
    MarkBox,
    ImageBox,
    ExpandedImageBox,
    AniImageBox,
    Button,
    RadioButton,
    ToggleButton,
    DragButton,
    Box,
    Bar,
    Line,
    Bar3D,
    NumberLine,
    /// The "+" button of a slot (`CSlotWindow::CSlotButton`).
    SlotButton,
    /// The cover button of a slot (`CSlotWindow::CCoverButton`).
    CoverButton,
}

impl WindowType {
    pub fn name(self) -> &'static str {
        match self {
            Self::Window => "Window",
            Self::Layer => "Layer",
            Self::Slot => "Slot",
            Self::GridSlot => "GridSlot",
            Self::TextLine => "TextLine",
            Self::MarkBox => "MarkBox",
            Self::ImageBox => "ImageBox",
            Self::ExpandedImageBox => "ExpandedImageBox",
            Self::AniImageBox => "AniImageBox",
            Self::Button | Self::SlotButton | Self::CoverButton => "Button",
            Self::RadioButton => "RadioButton",
            Self::ToggleButton => "ToggleButton",
            Self::DragButton => "DragButton",
            Self::Box => "Box",
            Self::Bar => "Bar",
            Self::Line => "Line",
            Self::Bar3D => "Bar3D",
            Self::NumberLine => "NumberLine",
        }
    }

    fn is_button(self) -> bool {
        matches!(
            self,
            Self::Button
                | Self::RadioButton
                | Self::ToggleButton
                | Self::DragButton
                | Self::SlotButton
                | Self::CoverButton
        )
    }

    pub fn is_slot(self) -> bool {
        matches!(self, Self::Slot | Self::GridSlot)
    }
}

pub const FLAG_MOVABLE: u32 = 1 << 0;
pub const FLAG_LIMIT: u32 = 1 << 1;
pub const FLAG_SNAP: u32 = 1 << 2;
pub const FLAG_DRAGABLE: u32 = 1 << 3;
pub const FLAG_ATTACH: u32 = 1 << 4;
pub const FLAG_RESTRICT_X: u32 = 1 << 5;
pub const FLAG_RESTRICT_Y: u32 = 1 << 6;
pub const FLAG_NOT_CAPTURE: u32 = 1 << 7;
pub const FLAG_FLOAT: u32 = 1 << 8;
pub const FLAG_NOT_PICK: u32 = 1 << 9;
pub const FLAG_IGNORE_SIZE: u32 = 1 << 10;
pub const FLAG_RTL: u32 = 1 << 11;

pub const HORIZONTAL_ALIGN_LEFT: i32 = 0;
pub const HORIZONTAL_ALIGN_CENTER: i32 = 1;
pub const HORIZONTAL_ALIGN_RIGHT: i32 = 2;
pub const VERTICAL_ALIGN_TOP: i32 = 0;
pub const VERTICAL_ALIGN_CENTER: i32 = 1;
pub const VERTICAL_ALIGN_BOTTOM: i32 = 2;

/// Layer names, bottom to top (`layerTbl` in the manager constructor).
pub const LAYERS: [&str; 5] = ["GAME", "UI_BOTTOM", "UI", "TOP_MOST", "CURTAIN"];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Rect {
    pub left: i64,
    pub top: i64,
    pub right: i64,
    pub bottom: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ButtonState {
    pub enable: bool,
    pub pressed: bool,
    pub flash: bool,
}

/// `CAniImageBox` frame counters (`BYTE`s, they wrap).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AniState {
    pub cur_delay: u8,
    pub delay: u8,
    pub cur_index: u8,
    /// Frames loaded (`m_ImageVector.size()`).
    pub images: usize,
}

/// Identity of a slot button: which slot window and slot it belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SlotButtonInfo {
    pub owner: WindowId,
    pub number: u32,
    pub left_enable: bool,
    pub right_enable: bool,
}

#[derive(Debug, Clone)]
pub struct Window {
    pub(crate) ty: WindowType,
    pub(crate) name: String,
    pub(crate) h_align: i32,
    pub(crate) v_align: i32,
    pub(crate) x: i64,
    pub(crate) y: i64,
    pub(crate) width: i64,
    pub(crate) height: i64,
    pub(crate) rect: Rect,
    pub(crate) bias: Rect,
    pub(crate) show: bool,
    pub(crate) flags: u32,
    /// `m_poHandler != nullptr`.
    pub(crate) handler: bool,
    pub(crate) parent: Option<WindowId>,
    pub(crate) children: Vec<WindowId>,
    pub(crate) updating: bool,
    pub(crate) reserve_children: Vec<WindowId>,
    pub(crate) button: ButtonState,
    pub(crate) restrict: Rect,
    pub(crate) ani: AniState,
    pub(crate) slot: Option<Box<SlotState>>,
    pub(crate) text: Option<Box<TextInstance>>,
    pub(crate) slot_button: Option<SlotButtonInfo>,
}

impl Window {
    fn new(ty: WindowType, handler: bool) -> Self {
        let mut flags = 0;
        if ty.is_button() && ty != WindowType::DragButton {
            flags |= FLAG_NOT_CAPTURE;
        }
        let mut text = None;
        if ty == WindowType::TextLine {
            let mut t = TextInstance::new();
            t.set_color(0xffc7_c7c7); // SetColor(0.78f, 0.78f, 0.78f)
            text = Some(Box::new(t));
        }
        let slot = ty.is_slot().then(|| Box::new(SlotState::new()));
        Self {
            ty,
            name: String::new(),
            h_align: 0,
            v_align: 0,
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            rect: Rect::default(),
            bias: Rect::default(),
            show: false,
            flags,
            handler,
            parent: None,
            children: Vec::new(),
            updating: false,
            reserve_children: Vec::new(),
            button: ButtonState { enable: true, pressed: false, flash: false },
            restrict: Rect::default(),
            ani: AniState { cur_delay: 0, delay: 4, cur_index: 0, images: 0 },
            slot,
            text,
            slot_button: None,
        }
    }

    pub fn window_type(&self) -> WindowType {
        self.ty
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn is_show(&self) -> bool {
        self.show
    }
    pub fn flags(&self) -> u32 {
        self.flags
    }
    pub fn is_flag(&self, f: u32) -> bool {
        self.flags & f != 0
    }
    pub fn position(&self) -> (i64, i64) {
        (self.x, self.y)
    }
    pub fn size(&self) -> (i64, i64) {
        (self.width, self.height)
    }
    pub fn rect(&self) -> Rect {
        self.rect
    }
    pub fn limit_bias(&self) -> Rect {
        self.bias
    }
    pub fn h_align(&self) -> i32 {
        self.h_align
    }
    pub fn v_align(&self) -> i32 {
        self.v_align
    }
    pub fn parent(&self) -> Option<WindowId> {
        self.parent
    }
    pub fn children(&self) -> &[WindowId] {
        &self.children
    }
    pub fn button(&self) -> ButtonState {
        self.button
    }
    pub fn restrict_area(&self) -> Rect {
        self.restrict
    }
    pub fn ani(&self) -> AniState {
        self.ani
    }
    pub fn slot(&self) -> Option<&SlotState> {
        self.slot.as_deref()
    }
    pub fn text(&self) -> Option<&TextInstance> {
        self.text.as_deref()
    }
    /// Is `m_poHandler` still set (cleared by `Clear`).
    pub fn has_handler(&self) -> bool {
        self.handler
    }
}

struct Entry {
    generation: u32,
    win: Option<Window>,
}

/// The window arena plus `CWindowManager`'s state.
pub struct Ui {
    entries: Vec<Entry>,
    free: Vec<u32>,
    pub(crate) root: WindowId,
    layers: Vec<(&'static str, WindowId)>,

    pub(crate) width: i64,
    pub(crate) height: i64,
    hres: i32,
    vres: i32,
    pub(crate) mouse: (i64, i64),
    drag: (i64, i64),
    picked: (i64, i64),
    once_ignore_left_up: bool,
    ignore_end_time: u32,

    pub(crate) mouse_handler: bool,
    pub(crate) attaching: bool,
    pub(crate) attach_type: u32,
    pub(crate) attach_index: u32,
    pub(crate) attach_slot: u32,
    pub(crate) attach_size: (u8, u8),

    pub(crate) active: Option<WindowId>,
    pub(crate) active_list: Vec<WindowId>,
    pub(crate) lock: Option<WindowId>,
    pub(crate) lock_list: Vec<WindowId>,
    pub(crate) point: Option<WindowId>,
    left_capture: Option<WindowId>,
    right_capture: Option<WindowId>,
    middle_capture: Option<WindowId>,
    key_capture: BTreeMap<i32, WindowId>,
    pub(crate) reserve_delete: Vec<WindowId>,
    pick_always: Vec<WindowId>,
}

impl Default for Ui {
    fn default() -> Self {
        Self::new()
    }
}

impl Ui {
    /// `CWindowManager::CWindowManager`: root plus the five layers.
    pub fn new() -> Self {
        let mut ui = Self {
            entries: Vec::new(),
            free: Vec::new(),
            root: WindowId { index: 0, generation: 0 },
            layers: Vec::new(),
            width: 0,
            height: 0,
            hres: 0,
            vres: 0,
            mouse: (0, 0),
            drag: (0, 0),
            picked: (0, 0),
            once_ignore_left_up: false,
            ignore_end_time: 0,
            mouse_handler: false,
            attaching: false,
            attach_type: 0,
            attach_index: 0,
            attach_slot: 0,
            attach_size: (0, 0),
            active: None,
            active_list: Vec::new(),
            lock: None,
            lock_list: Vec::new(),
            point: None,
            left_capture: None,
            right_capture: None,
            middle_capture: None,
            key_capture: BTreeMap::new(),
            reserve_delete: Vec::new(),
            pick_always: Vec::new(),
        };
        let root = ui.alloc(Window::new(WindowType::Window, false));
        ui.w_mut(root).name = "root".into();
        ui.w_mut(root).show = true;
        ui.root = root;
        for name in LAYERS {
            let l = ui.alloc(Window::new(WindowType::Layer, false));
            ui.w_mut(l).name = name.into();
            ui.w_mut(l).show = true;
            ui.add_child(root, l);
            ui.layers.push((name, l));
        }
        ui
    }

    fn alloc(&mut self, win: Window) -> WindowId {
        if let Some(index) = self.free.pop() {
            let e = &mut self.entries[index as usize];
            e.win = Some(win);
            WindowId { index, generation: e.generation }
        } else {
            self.entries.push(Entry { generation: 0, win: Some(win) });
            WindowId { index: self.entries.len() as u32 - 1, generation: 0 }
        }
    }

    pub fn is_alive(&self, id: WindowId) -> bool {
        self.entries.get(id.index as usize).is_some_and(|e| e.generation == id.generation && e.win.is_some())
    }

    pub fn window(&self, id: WindowId) -> &Window {
        self.w(id)
    }

    pub(crate) fn w(&self, id: WindowId) -> &Window {
        let e = &self.entries[id.index as usize];
        assert!(e.generation == id.generation, "stale WindowId");
        e.win.as_ref().expect("freed window")
    }

    pub(crate) fn w_mut(&mut self, id: WindowId) -> &mut Window {
        let e = &mut self.entries[id.index as usize];
        assert!(e.generation == id.generation, "stale WindowId");
        e.win.as_mut().expect("freed window")
    }

    pub fn root(&self) -> WindowId {
        self.root
    }

    pub fn layer(&self, name: &str) -> Option<WindowId> {
        self.layers.iter().find(|(n, _)| *n == name).map(|(_, id)| *id)
    }

    // ---- events to the script layer -----------------------------------------

    pub(crate) fn fire(&self, host: &mut dyn UiHost, id: WindowId, name: &'static str, args: &[i64]) -> Option<i64> {
        if !self.w(id).handler || !host.has_handler(Some(id), name) {
            return None;
        }
        host.call(&UiEvent { window: Some(id), name, args: args.to_vec() })
    }

    /// `PyCallClassMemberFunc(..., &lValue)` followed by `0 != lValue`.
    pub(crate) fn fire_bool(&self, host: &mut dyn UiHost, id: WindowId, name: &'static str, args: &[i64]) -> bool {
        self.fire(host, id, name, args).is_some_and(|v| v != 0)
    }

    pub(crate) fn fire_mouse(&self, host: &mut dyn UiHost, name: &'static str) {
        if self.mouse_handler && host.has_handler(None, name) {
            host.call(&UiEvent { window: None, name, args: Vec::new() });
        }
    }

    // ---- registration ---------------------------------------------------------

    /// `RegisterWindow` / `RegisterTypeWindow`: creates a window with a script
    /// handler in `layer`.
    pub fn register(&mut self, ty: WindowType, layer: &str) -> WindowId {
        let l = self.layer(layer).expect("unknown layer");
        let mut win = Window::new(ty, true);
        if ty == WindowType::DragButton {
            win.restrict = Rect { left: 0, top: 0, right: self.width, bottom: self.height };
        }
        let id = self.alloc(win);
        self.add_child(l, id);
        id
    }

    /// A window created by C++ itself (slot buttons...): no script handler.
    pub(crate) fn create_internal(&mut self, ty: WindowType) -> WindowId {
        let win = Window::new(ty, false);
        self.alloc(win)
    }

    // ---- CWindow ---------------------------------------------------------------

    pub fn set_name(&mut self, id: WindowId, name: &str) {
        self.w_mut(id).name = name.to_string();
    }

    pub fn show(&mut self, id: WindowId) {
        self.w_mut(id).show = true;
    }

    pub fn hide(&mut self, id: WindowId) {
        self.w_mut(id).show = false;
    }

    pub fn add_flag(&mut self, id: WindowId, flag: u32) {
        self.w_mut(id).flags |= flag;
    }

    pub fn remove_flag(&mut self, id: WindowId, flag: u32) {
        self.w_mut(id).flags &= !flag;
    }

    pub fn set_limit_bias(&mut self, id: WindowId, l: i64, r: i64, t: i64, b: i64) {
        self.w_mut(id).bias = Rect { left: l, right: r, top: t, bottom: b };
    }

    /// `IsRendering`: shown and every ancestor shown.
    pub fn is_rendering(&self, id: WindowId) -> bool {
        let w = self.w(id);
        if !w.show {
            return false;
        }
        match w.parent {
            None => true,
            Some(p) => self.is_rendering(p),
        }
    }

    /// `CWindow::SetSize` (does not touch children or call `UpdateRect`).
    pub fn set_size(&mut self, id: WindowId, width: i64, height: i64) {
        let w = self.w_mut(id);
        w.width = width;
        w.height = height;
        w.rect.right = w.rect.left + width;
        w.rect.bottom = w.rect.top + height;
    }

    pub fn set_horizontal_align(&mut self, host: &mut dyn UiHost, id: WindowId, align: i32) {
        self.w_mut(id).h_align = align;
        self.update_rect(host, id);
    }

    pub fn set_vertical_align(&mut self, host: &mut dyn UiHost, id: WindowId, align: i32) {
        self.w_mut(id).v_align = align;
        self.update_rect(host, id);
    }

    pub fn set_position(&mut self, host: &mut dyn UiHost, id: WindowId, x: i64, y: i64) {
        let w = self.w_mut(id);
        w.x = x;
        w.y = y;
        self.update_rect(host, id);
    }

    /// `UpdateRect`: recomputes the global rectangle from the parent's, then
    /// the children's, then runs the type's `OnChangePosition`.
    pub fn update_rect(&mut self, host: &mut dyn UiHost, id: WindowId) {
        let (x, y, width, height, va, ha, parent) = {
            let w = self.w(id);
            (w.x, w.y, w.width, w.height, w.v_align, w.h_align, w.parent)
        };
        let mut top = y;
        let mut left = x;
        if let Some(p) = parent {
            let (pw, ph, pr) = {
                let pw = self.w(p);
                (pw.width, pw.height, pw.rect)
            };
            match va {
                VERTICAL_ALIGN_BOTTOM => top = ph - top,
                VERTICAL_ALIGN_CENTER => top = ((ph - height) / 2) + top,
                _ => {}
            }
            top += pr.top;
            match ha {
                HORIZONTAL_ALIGN_RIGHT => left = pw.abs() - left,
                HORIZONTAL_ALIGN_CENTER => left = (pw / 2) - (width / 2) + left,
                _ => {}
            }
            left += if 0 < pw { pr.left } else { pr.right + pw.abs() };
        }
        {
            let w = self.w_mut(id);
            w.rect.top = top;
            w.rect.bottom = top + height;
            w.rect.left = left;
            w.rect.right = left + width;
        }
        for c in self.w(id).children.clone() {
            self.update_rect(host, c);
        }
        self.on_change_position(host, id);
    }

    pub fn add_child(&mut self, parent: WindowId, child: WindowId) {
        self.w_mut(parent).children.push(child);
        self.w_mut(child).parent = Some(parent);
    }

    /// `GetRoot`: climbs while the parent is a real window (not a layer).
    pub fn get_root(&self, id: WindowId) -> WindowId {
        if let Some(p) = self.w(id).parent
            && self.w(p).ty != WindowType::Layer
        {
            return self.get_root(p);
        }
        id
    }

    pub fn is_child(&self, parent: WindowId, id: WindowId) -> bool {
        self.w(parent).children.contains(&id)
    }

    pub fn child_count(&self, id: WindowId) -> usize {
        self.w(id).children.len()
    }

    /// `DeleteChild`: deferred while the parent is updating its children.
    pub fn delete_child(&mut self, parent: WindowId, child: WindowId) {
        let p = self.w_mut(parent);
        if p.updating {
            p.reserve_children.push(child);
        } else {
            p.children.retain(|c| *c != child);
        }
    }

    /// `CWindow::SetTop(pWin)` called on the parent: moves a FLOAT child to the
    /// end of the list (drawn and picked last) and fires `OnTop`.
    pub fn set_top_child(&mut self, host: &mut dyn UiHost, parent: WindowId, child: WindowId) {
        if !self.w(child).is_flag(FLAG_FLOAT) {
            return;
        }
        let p = self.w_mut(parent);
        if let Some(pos) = p.children.iter().position(|c| *c == child) {
            let c = p.children.remove(pos);
            p.children.push(c);
            self.fire(host, child, "OnTop", &[]);
        }
    }

    /// `CWindow::Clear`: detaches the whole subtree's links and handlers.
    pub fn clear(&mut self, id: WindowId) {
        for c in std::mem::take(&mut self.w_mut(id).children) {
            self.clear(c);
        }
        let w = self.w_mut(id);
        w.parent = None;
        w.handler = false;
        w.show = false;
    }

    /// `IsIn(x, y)`: inclusive rectangle test.
    pub fn is_in_point(&self, id: WindowId, x: i64, y: i64) -> bool {
        let r = self.w(id).rect;
        x >= r.left && x <= r.right && y >= r.top && y <= r.bottom
    }

    /// `IsIn()`: is the mouse inside.
    pub fn is_in(&self, id: WindowId) -> bool {
        self.is_in_point(id, self.mouse.0, self.mouse.1)
    }

    pub fn get_mouse_local_position(&self, id: WindowId) -> (i64, i64) {
        let r = self.w(id).rect;
        (self.mouse.0 - r.left, self.mouse.1 - r.top)
    }

    /// `PickWindow`: the topmost pickable descendant containing the point,
    /// children first (last child is on top), else the window itself.
    pub fn pick_window(&self, id: WindowId, x: i64, y: i64) -> Option<WindowId> {
        for &c in self.w(id).children.iter().rev() {
            let cw = self.w(c);
            if cw.show {
                if !cw.is_flag(FLAG_IGNORE_SIZE) && !self.is_in_point(c, x, y) && cw.width >= 0 {
                    continue;
                }
                if let Some(r) = self.pick_window(c, x, y) {
                    return Some(r);
                }
            }
        }
        if self.w(id).is_flag(FLAG_NOT_PICK) {
            return None;
        }
        Some(id)
    }

    /// `PickTopWindow`: direct children only.
    pub fn pick_top_window(&self, id: WindowId, x: i64, y: i64) -> Option<WindowId> {
        for &c in self.w(id).children.iter().rev() {
            let cw = self.w(c);
            if cw.show && self.is_in_point(c, x, y) && !cw.is_flag(FLAG_NOT_PICK) {
                return Some(c);
            }
        }
        None
    }

    // ---- update ------------------------------------------------------------------

    fn remove_reserve_children(&mut self, id: WindowId) {
        let w = self.w_mut(id);
        if w.reserve_children.is_empty() {
            return;
        }
        let reserve = std::mem::take(&mut w.reserve_children);
        for r in reserve {
            w.children.retain(|c| *c != r);
        }
    }

    /// `CWindow::Update` for one window and its subtree.
    pub fn update_window(&mut self, host: &mut dyn UiHost, id: WindowId) {
        if !self.w(id).show {
            return;
        }
        self.remove_reserve_children(id);
        self.on_update(host, id);
        self.w_mut(id).updating = true;
        for c in self.w(id).children.clone() {
            if self.is_alive(c) {
                self.update_window(host, c);
            }
        }
        self.w_mut(id).updating = false;
    }

    /// `CWindowManager::Update`: frees windows destroyed since the last call,
    /// then updates the tree.
    pub fn update(&mut self, host: &mut dyn UiHost) {
        self.clear_reserve_delete_list(host);
        let root = self.root;
        self.update_window(host, root);
    }

    fn clear_reserve_delete_list(&mut self, host: &mut dyn UiHost) {
        let mut i = 0;
        while i < self.reserve_delete.len() {
            let id = self.reserve_delete[i];
            i += 1;
            // ~CSlotWindow() runs Destroy(), which destroys the slot's buttons
            // into this same list.
            if self.w(id).ty.is_slot() {
                self.slot_destroy(host, id);
            }
            let e = &mut self.entries[id.index as usize];
            e.win = None;
            e.generation += 1;
            self.free.push(id.index);
        }
        self.reserve_delete.clear();
    }

    fn on_update(&mut self, host: &mut dyn UiHost, id: WindowId) {
        match self.w(id).ty {
            WindowType::ImageBox
            | WindowType::ExpandedImageBox
            | WindowType::MarkBox
            | WindowType::Button
            | WindowType::RadioButton
            | WindowType::ToggleButton
            | WindowType::DragButton
            | WindowType::SlotButton
            | WindowType::CoverButton
            | WindowType::Slot
            | WindowType::GridSlot => {}
            WindowType::TextLine => {
                if self.w(id).show {
                    let mut t = self.w_mut(id).text.take().unwrap();
                    t.update(host);
                    self.w_mut(id).text = Some(t);
                }
            }
            WindowType::AniImageBox => self.ani_update(host, id),
            _ => {
                if self.w(id).show {
                    self.fire(host, id, "OnUpdate", &[]);
                }
            }
        }
    }

    /// `CAniImageBox::OnUpdate`.
    fn ani_update(&mut self, host: &mut dyn UiHost, id: WindowId) {
        let a = &mut self.w_mut(id).ani;
        a.cur_delay = a.cur_delay.wrapping_add(1);
        if a.cur_delay < a.delay {
            return;
        }
        a.cur_delay = 0;
        a.cur_index = a.cur_index.wrapping_add(1);
        if a.cur_index as usize >= a.images {
            a.cur_index = 0;
            self.fire(host, id, "OnEndFrame", &[]);
        }
    }

    pub fn set_ani_delay(&mut self, id: WindowId, delay: i32) {
        self.w_mut(id).ani.delay = delay as u8;
    }

    fn on_change_position(&mut self, host: &mut dyn UiHost, id: WindowId) {
        match self.w(id).ty {
            WindowType::TextLine => {
                let r = self.w(id).rect;
                if let Some(t) = self.w_mut(id).text.as_mut() {
                    t.position.0 = r.left as f32;
                    t.position.1 = r.top as f32;
                }
            }
            WindowType::DragButton => self.drag_button_change_position(host, id),
            _ => {}
        }
    }

    // ---- manager ----------------------------------------------------------------

    pub fn set_screen_size(&mut self, width: i64, height: i64) {
        self.width = width;
        self.height = height;
        for (_, l) in self.layers.clone() {
            self.set_size(l, width, height);
        }
    }

    pub fn set_resolution(&mut self, hres: i32, vres: i32) {
        if hres <= 0 || vres <= 0 {
            return;
        }
        self.hres = hres;
        self.vres = vres;
    }

    pub fn screen_size(&self) -> (i64, i64) {
        (self.width, self.height)
    }

    pub fn mouse_position(&self) -> (i64, i64) {
        self.mouse
    }

    pub fn point_window(&self) -> Option<WindowId> {
        self.point
    }
    pub fn active_window(&self) -> Option<WindowId> {
        self.active
    }
    pub fn lock_window_current(&self) -> Option<WindowId> {
        self.lock
    }
    pub fn left_capture(&self) -> Option<WindowId> {
        self.left_capture
    }
    pub fn right_capture(&self) -> Option<WindowId> {
        self.right_capture
    }
    pub fn middle_capture(&self) -> Option<WindowId> {
        self.middle_capture
    }
    pub fn is_attaching(&self) -> bool {
        self.attaching
    }
    pub fn attaching(&self) -> (u32, u32, u32, u8, u8) {
        (self.attach_type, self.attach_index, self.attach_slot, self.attach_size.0, self.attach_size.1)
    }
    pub fn drag_origin(&self) -> (i64, i64) {
        self.drag
    }
    pub fn picked_origin(&self) -> (i64, i64) {
        self.picked
    }
    pub fn lock_list(&self) -> &[WindowId] {
        &self.lock_list
    }
    pub fn active_list(&self) -> &[WindowId] {
        &self.active_list
    }
    pub fn pick_always_list(&self) -> &[WindowId] {
        &self.pick_always
    }
    pub fn reserve_delete_list(&self) -> &[WindowId] {
        &self.reserve_delete
    }
    pub fn key_captures(&self) -> impl Iterator<Item = (i32, WindowId)> + '_ {
        self.key_capture.iter().map(|(k, v)| (*k, *v))
    }

    pub fn set_mouse_handler(&mut self, set: bool) {
        self.mouse_handler = set;
    }

    /// `IsDragging`: moved 10 pixels (Manhattan) since the button went down.
    pub fn is_dragging(&self) -> bool {
        let dx = (self.mouse.0 - self.picked.0).abs();
        let dy = (self.mouse.1 - self.picked.1).abs();
        dx + dy >= 10
    }

    pub fn attach_icon(&mut self, ty: u32, index: u32, slot: u32, w: u8, h: u8) {
        self.attaching = true;
        self.attach_type = ty;
        self.attach_index = index;
        self.attach_slot = slot;
        self.attach_size = (w, h);
    }

    pub fn set_attaching_flag(&mut self, flag: bool) {
        self.attaching = flag;
    }

    pub fn deattach_icon(&mut self, host: &mut dyn UiHost) {
        self.attaching = false;
        self.fire_mouse(host, "DeattachObject");
    }

    pub fn once_ignore_mouse_left_button_up_event(&mut self, host: &mut dyn UiHost) {
        self.once_ignore_left_up = true;
        self.ignore_end_time = host.time_ms().wrapping_add(500);
    }

    /// `SetParent`: moves `window` under `parent` (appended last).
    pub fn set_parent(&mut self, window: WindowId, parent: WindowId) {
        if let Some(old) = self.w(window).parent {
            if old == parent {
                return;
            }
            self.delete_child(old, window);
        }
        self.add_child(parent, window);
    }

    pub fn set_pick_always(&mut self, id: WindowId) {
        self.pick_always.push(id);
    }

    /// `NotifyDestroyWindow`: forget `id` everywhere in the manager.
    pub fn notify_destroy_window(&mut self, id: WindowId) {
        let un = |o: &mut Option<WindowId>| {
            if *o == Some(id) {
                *o = None;
            }
        };
        un(&mut self.active);
        un(&mut self.point);
        un(&mut self.left_capture);
        un(&mut self.middle_capture);
        un(&mut self.right_capture);
        un(&mut self.lock);
        self.lock_list.retain(|w| *w != id);
        self.active_list.retain(|w| *w != id);
        self.pick_always.retain(|w| *w != id);
        self.key_capture.retain(|_, w| *w != id);
    }

    /// `DestroyWindow`: unlinks now, frees at the next [`Ui::update`].
    pub fn destroy_window(&mut self, id: WindowId) {
        self.notify_destroy_window(id);
        if let Some(p) = self.w(id).parent {
            self.delete_child(p, id);
        }
        self.clear(id);
        self.reserve_delete.push(id);
    }

    pub fn lock_window(&mut self, host: &mut dyn UiHost, id: WindowId) {
        if let Some(a) = self.active {
            self.fire(host, a, "OnKillFocus", &[]);
        }
        self.lock_list.retain(|w| *w != id);
        if let Some(cur) = self.lock {
            if cur == id {
                return;
            }
            self.lock_list.push(cur);
        }
        self.lock = Some(id);
    }

    pub fn unlock_window(&mut self, host: &mut dyn UiHost) {
        if self.lock.is_some() {
            if self.lock_list.is_empty() {
                self.lock = None;
                if let Some(a) = self.active {
                    self.fire(host, a, "OnSetFocus", &[]);
                }
            } else {
                self.lock = self.lock_list.pop();
            }
        }
    }

    pub fn activate_window(&mut self, host: &mut dyn UiHost, id: WindowId) {
        self.active_list.retain(|w| *w != id);
        if Some(id) == self.active {
            return;
        }
        if let Some(a) = self.active {
            self.active_list.push(a);
            self.fire(host, a, "OnKillFocus", &[]);
        }
        self.active = Some(id);
        self.fire(host, id, "OnSetFocus", &[]);
    }

    pub fn deactivate_window(&mut self, host: &mut dyn UiHost) {
        if let Some(a) = self.active {
            if self.active_list.is_empty() {
                self.fire(host, a, "OnKillFocus", &[]);
                self.active = None;
            } else {
                self.fire(host, a, "OnKillFocus", &[]);
                let n = self.active_list.pop().unwrap();
                self.active = Some(n);
                self.fire(host, n, "OnSetFocus", &[]);
            }
        }
    }

    pub fn is_focus_window(&self, id: WindowId) -> bool {
        self.active == Some(id)
    }

    /// `CWindowManager::SetTop`.
    pub fn set_top(&mut self, host: &mut dyn UiHost, id: WindowId) {
        if let Some(p) = self.w(id).parent {
            self.set_top_child(host, p, id);
        }
    }

    fn set_top_ui_window(&mut self, host: &mut dyn UiHost) {
        if self.lock.is_some() {
            return;
        }
        let Some(ui) = self.layer("UI") else { return };
        if let Some(top) = self.pick_top_window(ui, self.mouse.0, self.mouse.1) {
            self.set_top(host, top);
        }
    }

    pub fn reset_capture(&mut self) {
        self.left_capture = None;
        self.middle_capture = None;
        self.right_capture = None;
    }

    /// `__PickWindow`.
    pub fn pick_at(&self, x: i64, y: i64) -> Option<WindowId> {
        if let Some(l) = self.lock {
            return self.pick_window(l, x, y);
        }
        for &w in &self.pick_always {
            if self.is_rendering(w) && self.is_in_point(w, x, y) {
                return Some(w);
            }
        }
        for &(_, layer) in self.layers.iter().rev() {
            let picked = self.pick_window(layer, x, y);
            if picked != Some(layer) {
                return picked;
            }
        }
        None
    }

    fn set_mouse_position(&mut self, x: i64, y: i64) {
        if self.hres == 0 || self.vres == 0 {
            return;
        }
        self.mouse.0 = self.width * x / self.hres as i64;
        self.mouse.1 = self.height * y / self.vres as i64;
    }

    // ---- mouse -------------------------------------------------------------------

    pub fn run_mouse_move(&mut self, host: &mut dyn UiHost, mut x: i64, mut y: i64) {
        if self.attaching {
            x = x.min(self.width);
            y = y.min(self.height);
        }
        self.set_mouse_position(x, y);
        let point = self.pick_at(self.mouse.0, self.mouse.1);

        if let Some(win) = self.left_capture {
            let (flags, parent, wd, ht, bias) = {
                let w = self.w(win);
                (w.flags, w.parent, w.width, w.height, w.bias)
            };
            if flags & FLAG_MOVABLE != 0 {
                let mut nx = self.mouse.0 - self.drag.0;
                let mut ny = self.mouse.1 - self.drag.1;
                if let Some(p) = parent {
                    let pr = self.w(p).rect;
                    nx -= pr.left;
                    ny -= pr.top;
                }
                let (lx, ly) = (self.w(win).x, self.w(win).y);
                if flags & FLAG_RESTRICT_X != 0 {
                    nx = lx;
                }
                if flags & FLAG_RESTRICT_Y != 0 {
                    ny = ly;
                }
                if flags & FLAG_LIMIT != 0 {
                    let mut lim = bias;
                    lim.right = self.width - lim.right;
                    lim.bottom = self.height - lim.bottom;
                    if nx < lim.left {
                        nx = lim.left;
                    } else if nx + wd >= lim.right {
                        nx = lim.right - wd;
                    }
                    if ny < lim.top {
                        ny = lim.top;
                    } else if ny + ht >= lim.bottom {
                        ny = lim.bottom - ht;
                    }
                }
                self.set_position(host, win, nx, ny);
                self.fire(host, win, "OnMoveWindow", &[nx, ny]);
            } else if flags & FLAG_DRAGABLE != 0 {
                let dx = self.mouse.0 - self.drag.0;
                let dy = self.mouse.1 - self.drag.1;
                self.fire(host, win, "OnMouseDrag", &[dx, dy]);
            }
        }

        if self.point != point {
            if let Some(old) = self.point {
                self.on_mouse_over_out(host, old);
            }
            self.point = point;
            if let Some(new) = self.point {
                self.on_mouse_over_in(host, new);
            }
        }
        if let Some(p) = self.point {
            self.on_mouse_over(host, p);
        }
    }

    pub fn run_mouse_left_button_down(&mut self, host: &mut dyn UiHost, x: i64, y: i64) {
        self.set_top_ui_window(host);
        self.set_mouse_position(x, y);
        let Some(mut win) = self.point else { return };
        if self.w(win).is_flag(FLAG_ATTACH) {
            win = self.get_root(win);
        }
        if !self.w(win).is_flag(FLAG_NOT_CAPTURE) {
            self.left_capture = Some(win);
        }
        let r = self.w(win).rect;
        self.drag = (self.mouse.0 - r.left, self.mouse.1 - r.top);
        self.picked = self.mouse;
        self.on_mouse_left_button_down(host, win);
    }

    pub fn run_mouse_left_button_up(&mut self, host: &mut dyn UiHost, x: i64, y: i64) {
        if self.once_ignore_left_up {
            self.once_ignore_left_up = false;
            if host.time_ms() < self.ignore_end_time {
                return;
            }
        }
        self.set_mouse_position(x, y);
        if let Some(c) = self.left_capture
            && self.on_mouse_left_button_up(host, c)
        {
            self.left_capture = None;
            return;
        }
        if let Some(p) = self.point {
            self.on_mouse_left_button_up(host, p);
        }
        self.left_capture = None;
    }

    pub fn run_mouse_left_button_double_click(&mut self, host: &mut dyn UiHost, x: i64, y: i64) {
        self.set_mouse_position(x, y);
        if let Some(p) = self.point {
            self.on_mouse_left_button_double_click(host, p);
        }
    }

    pub fn run_mouse_right_button_down(&mut self, host: &mut dyn UiHost, x: i64, y: i64) {
        self.set_top_ui_window(host);
        self.set_mouse_position(x, y);
        let Some(mut win) = self.point else { return };
        if self.w(win).is_flag(FLAG_ATTACH) {
            win = self.get_root(win);
        }
        if !self.w(win).is_flag(FLAG_NOT_CAPTURE) {
            self.right_capture = Some(win);
        }
        self.on_mouse_right_button_down(host, win);
    }

    pub fn run_mouse_right_button_up(&mut self, host: &mut dyn UiHost, _x: i64, _y: i64) {
        if let Some(c) = self.right_capture
            && self.on_mouse_right_button_up(host, c)
        {
            self.right_capture = None;
            return;
        }
        if let Some(p) = self.point {
            self.on_mouse_right_button_up(host, p);
        }
        self.right_capture = None;
        self.deattach_icon(host);
    }

    pub fn run_mouse_right_button_double_click(&mut self, host: &mut dyn UiHost, x: i64, y: i64) {
        self.set_mouse_position(x, y);
        if let Some(p) = self.point {
            self.on_mouse_right_button_double_click(host, p);
        }
    }

    pub fn run_mouse_middle_button_down(&mut self, host: &mut dyn UiHost, x: i64, y: i64) {
        self.set_mouse_position(x, y);
        let Some(p) = self.point else { return };
        self.middle_capture = Some(p);
        self.on_mouse_middle_button_down(host, p);
    }

    pub fn run_mouse_middle_button_up(&mut self, host: &mut dyn UiHost, x: i64, y: i64) {
        self.set_mouse_position(x, y);
        if let Some(c) = self.middle_capture
            && self.on_mouse_middle_button_up(host, c)
        {
            self.middle_capture = None;
            return;
        }
        let Some(p) = self.point else { return };
        self.on_mouse_middle_button_up(host, p);
        self.middle_capture = None;
    }

    // ---- IME / keys ----------------------------------------------------------------

    pub fn run_ime_update(&mut self, host: &mut dyn UiHost) {
        if let Some(l) = self.lock {
            self.fire(host, l, "OnIMEUpdate", &[]);
            return;
        }
        let Some(a) = self.active else { return };
        if !self.is_rendering(a) {
            return;
        }
        self.fire(host, a, "OnIMEUpdate", &[]);
    }

    fn run_ime_tab_event(&self, host: &mut dyn UiHost, id: WindowId) -> bool {
        if !self.is_rendering(id) {
            return false;
        }
        if self.fire_bool(host, id, "OnIMETab", &[]) {
            return true;
        }
        for c in self.w(id).children.clone().into_iter().rev() {
            if self.run_ime_tab_event(host, c) {
                return true;
            }
        }
        false
    }

    pub fn run_ime_tab_event_all(&mut self, host: &mut dyn UiHost) {
        if let Some(l) = self.lock {
            self.fire_bool(host, l, "OnIMETab", &[]);
            return;
        }
        if let Some(a) = self.active
            && self.is_rendering(a)
            && self.fire_bool(host, a, "OnIMETab", &[])
        {
            return;
        }
        if !self.run_ime_tab_event(host, self.root) && !self.active_list.is_empty() {
            let w = self.active_list[0];
            self.activate_window(host, w);
            let mut parent = Some(w);
            let mut current = self.w(w).parent;
            for _ in 0..20 {
                let Some(p) = parent else { break };
                if Some(p) == self.layer("GAME") {
                    if let Some(c) = current {
                        self.set_top(host, c);
                    }
                    break;
                }
                current = Some(p);
                parent = self.w(p).parent;
            }
        }
    }

    fn run_ime_return_event(&self, host: &mut dyn UiHost, id: WindowId) -> bool {
        if !self.is_rendering(id) {
            return false;
        }
        if self.fire_bool(host, id, "OnIMEReturn", &[]) {
            return true;
        }
        for c in self.w(id).children.clone().into_iter().rev() {
            if self.run_ime_return_event(host, c) {
                return true;
            }
        }
        false
    }

    pub fn run_ime_return_event_all(&mut self, host: &mut dyn UiHost) {
        if let Some(l) = self.lock {
            self.fire_bool(host, l, "OnIMEReturn", &[]);
            return;
        }
        if let Some(a) = self.active
            && self.is_rendering(a)
            && self.fire_bool(host, a, "OnIMEReturn", &[])
        {
            return;
        }
        self.run_ime_return_event(host, self.root);
    }

    fn run_ime_key_down_event(&self, host: &mut dyn UiHost, id: WindowId, key: i64) -> bool {
        if !self.is_rendering(id) {
            return false;
        }
        if self.fire_bool(host, id, "OnIMEKeyDown", &[key]) {
            return true;
        }
        for c in self.w(id).children.clone().into_iter().rev() {
            if self.run_ime_key_down_event(host, c, key) {
                return true;
            }
        }
        false
    }

    pub fn run_ime_key_down(&mut self, host: &mut dyn UiHost, key: i64) {
        if let Some(l) = self.lock
            && self.run_ime_key_down_event(host, l, key)
        {
            return;
        }
        if let Some(a) = self.active
            && self.is_rendering(a)
            && self.run_ime_key_down_event(host, a, key)
        {
            return;
        }
        self.run_ime_key_down_event(host, self.root, key);
    }

    fn run_ime_simple(&mut self, host: &mut dyn UiHost, name: &'static str, lock_only_when_set: bool) {
        if let Some(l) = self.lock {
            self.fire_bool(host, l, name, &[]);
            return;
        }
        let _ = lock_only_when_set;
        if let Some(a) = self.active
            && self.is_rendering(a)
        {
            self.fire_bool(host, a, name, &[]);
        }
    }

    pub fn run_change_code_page(&mut self, host: &mut dyn UiHost) {
        // Unlike the others, no lock window handling.
        if let Some(a) = self.active
            && self.is_rendering(a)
        {
            self.fire_bool(host, a, "OnIMEChangeCodePage", &[]);
        }
    }
    pub fn run_open_candidate(&mut self, host: &mut dyn UiHost) {
        self.run_ime_simple(host, "OnIMEOpenCandidateList", true);
    }
    pub fn run_close_candidate(&mut self, host: &mut dyn UiHost) {
        self.run_ime_simple(host, "OnIMECloseCandidateList", true);
    }
    pub fn run_open_reading(&mut self, host: &mut dyn UiHost) {
        self.run_ime_simple(host, "OnIMEOpenReadingWnd", true);
    }
    pub fn run_close_reading(&mut self, host: &mut dyn UiHost) {
        self.run_ime_simple(host, "OnIMECloseReadingWnd", true);
    }

    fn run_key_down_event(&self, host: &mut dyn UiHost, id: WindowId, key: i64) -> Option<WindowId> {
        if self.fire_bool(host, id, "OnKeyDown", &[key]) {
            return Some(id);
        }
        for c in self.w(id).children.clone().into_iter().rev() {
            if self.w(c).show
                && let Some(r) = self.run_key_down_event(host, c, key)
            {
                return Some(r);
            }
        }
        None
    }

    fn run_key_up_event(&self, host: &mut dyn UiHost, id: WindowId, key: i64) -> bool {
        if self.fire_bool(host, id, "OnKeyUp", &[key]) {
            return true;
        }
        for c in self.w(id).children.clone().into_iter().rev() {
            if self.w(c).show && self.run_key_up_event(host, c, key) {
                return true;
            }
        }
        false
    }

    fn run_press_escape_key_event(&self, host: &mut dyn UiHost, id: WindowId) -> bool {
        for c in self.w(id).children.clone().into_iter().rev() {
            if self.w(c).show && self.run_press_escape_key_event(host, c) {
                return true;
            }
        }
        self.fire_bool(host, id, "OnPressEscapeKey", &[])
    }

    fn run_press_exit_key_event(&self, host: &mut dyn UiHost, id: WindowId) -> bool {
        for c in self.w(id).children.clone().into_iter().rev() {
            if self.run_press_exit_key_event(host, c) {
                return true;
            }
            if self.w(c).show && self.fire_bool(host, c, "OnPressExitKey", &[]) {
                return true;
            }
        }
        false
    }

    pub fn run_key_down(&mut self, host: &mut dyn UiHost, key: i32) {
        let k = key as i64;
        if let Some(l) = self.lock {
            self.fire_bool(host, l, "OnKeyDown", &[k]);
            self.key_capture.entry(key).or_insert(l);
            return;
        }
        if let Some(a) = self.active
            && self.w(a).show
            && self.fire_bool(host, a, "OnKeyDown", &[k])
        {
            self.key_capture.entry(key).or_insert(a);
            return;
        }
        if let Some(c) = self.run_key_down_event(host, self.root, k)
            && !self.reserve_delete.contains(&c)
        {
            self.key_capture.entry(key).or_insert(c);
        }
    }

    pub fn run_key_up(&mut self, host: &mut dyn UiHost, key: i32) {
        let k = key as i64;
        if let Some(c) = self.key_capture.remove(&key) {
            self.fire_bool(host, c, "OnKeyUp", &[k]);
            return;
        }
        if let Some(l) = self.lock {
            self.fire_bool(host, l, "OnKeyUp", &[k]);
            return;
        }
        if let Some(a) = self.active
            && self.fire_bool(host, a, "OnKeyUp", &[k])
        {
            return;
        }
        self.run_key_up_event(host, self.root, k);
    }

    pub fn run_press_escape_key(&mut self, host: &mut dyn UiHost) {
        if let Some(l) = self.lock {
            self.fire_bool(host, l, "OnPressEscapeKey", &[]);
            return;
        }
        self.run_press_escape_key_event(host, self.root);
    }

    pub fn run_press_exit_key(&mut self, host: &mut dyn UiHost) {
        if let Some(l) = self.lock {
            self.fire_bool(host, l, "OnPressExitKey", &[]);
            return;
        }
        if let Some(a) = self.active
            && self.fire_bool(host, a, "OnPressExitKey", &[])
        {
            return;
        }
        self.run_press_exit_key_event(host, self.root);
    }

    // ---- per-type event handlers (C++ virtuals) -------------------------------------

    fn on_mouse_left_button_down(&mut self, host: &mut dyn UiHost, id: WindowId) -> bool {
        match self.w(id).ty {
            WindowType::Slot | WindowType::GridSlot => self.slot_left_down(host, id),
            WindowType::Button | WindowType::DragButton => self.button_left_down(host, id),
            WindowType::RadioButton => {
                if !self.w(id).button.enable {
                    return true;
                }
                if !self.w(id).button.pressed {
                    self.button_down(host, id);
                    self.fire(host, id, "CallEvent", &[]);
                }
                true
            }
            WindowType::ToggleButton => {
                if !self.w(id).button.enable {
                    return true;
                }
                if self.w(id).button.pressed {
                    self.w_mut(id).button.pressed = false; // SetUp
                    self.fire(host, id, "OnToggleUp", &[]);
                } else {
                    self.button_down(host, id);
                    self.fire(host, id, "OnToggleDown", &[]);
                }
                true
            }
            WindowType::SlotButton => self.slot_button_left_down(host, id),
            WindowType::CoverButton => self.cover_left_down(host, id),
            _ => self.fire_bool(host, id, "OnMouseLeftButtonDown", &[]),
        }
    }

    fn on_mouse_left_button_up(&mut self, host: &mut dyn UiHost, id: WindowId) -> bool {
        match self.w(id).ty {
            WindowType::Slot | WindowType::GridSlot => self.slot_left_up(host, id),
            WindowType::Button | WindowType::DragButton => self.button_left_up(host, id),
            WindowType::RadioButton | WindowType::ToggleButton => true,
            WindowType::SlotButton => self.slot_button_left_up(host, id),
            WindowType::CoverButton => self.cover_left_up(host, id),
            _ => {
                self.fire(host, id, "OnMouseLeftButtonUp", &[]);
                true
            }
        }
    }

    fn on_mouse_left_button_double_click(&mut self, host: &mut dyn UiHost, id: WindowId) -> bool {
        let ty = self.w(id).ty;
        if ty.is_slot() {
            self.slot_use(host, id);
            return true;
        }
        if ty.is_button() {
            if !self.w(id).button.enable {
                return true;
            }
            self.on_mouse_left_button_down(host, id);
            return true;
        }
        self.fire_bool(host, id, "OnMouseLeftButtonDoubleClick", &[])
    }

    fn on_mouse_right_button_down(&mut self, host: &mut dyn UiHost, id: WindowId) -> bool {
        match self.w(id).ty {
            WindowType::Slot | WindowType::GridSlot => self.slot_right_down(host, id),
            WindowType::CoverButton => self.cover_right_down(host, id),
            _ => self.fire_bool(host, id, "OnMouseRightButtonDown", &[]),
        }
    }

    fn on_mouse_right_button_up(&mut self, host: &mut dyn UiHost, id: WindowId) -> bool {
        match self.w(id).ty {
            WindowType::CoverButton => self.cover_right_up(host, id),
            _ => self.fire_bool(host, id, "OnMouseRightButtonUp", &[]),
        }
    }

    fn on_mouse_right_button_double_click(&mut self, host: &mut dyn UiHost, id: WindowId) -> bool {
        self.fire_bool(host, id, "OnMouseRightButtonDoubleClick", &[])
    }

    fn on_mouse_middle_button_down(&mut self, host: &mut dyn UiHost, id: WindowId) -> bool {
        self.fire_bool(host, id, "OnMouseMiddleButtonDown", &[])
    }

    fn on_mouse_middle_button_up(&mut self, host: &mut dyn UiHost, id: WindowId) -> bool {
        self.fire_bool(host, id, "OnMouseMiddleButtonUp", &[])
    }

    fn on_mouse_over_in(&mut self, host: &mut dyn UiHost, id: WindowId) {
        let b = self.w(id).button;
        match self.w(id).ty {
            WindowType::Button => {
                if b.enable {
                    self.fire(host, id, "ShowToolTip", &[]);
                }
            }
            WindowType::RadioButton | WindowType::ToggleButton => {
                if b.enable {
                    self.fire(host, id, "ShowToolTip", &[]);
                }
            }
            WindowType::DragButton => {
                if b.enable {
                    self.fire(host, id, "ShowToolTip", &[]);
                    self.fire(host, id, "OnMouseOverIn", &[]);
                }
            }
            WindowType::SlotButton | WindowType::CoverButton => {
                let info = self.w(id).slot_button.unwrap();
                self.slot_over_in_item(host, info.owner, info.number);
            }
            _ => {
                self.fire(host, id, "OnMouseOverIn", &[]);
            }
        }
    }

    fn on_mouse_over_out(&mut self, host: &mut dyn UiHost, id: WindowId) {
        let b = self.w(id).button;
        match self.w(id).ty {
            WindowType::Button => {
                if b.enable {
                    self.w_mut(id).button.pressed = false; // SetUp
                    self.fire(host, id, "HideToolTip", &[]);
                }
            }
            WindowType::RadioButton | WindowType::ToggleButton => {
                if b.enable {
                    self.fire(host, id, "HideToolTip", &[]);
                }
            }
            WindowType::DragButton => {
                if b.enable {
                    // C++ calls CButton::OnMouseOverIn() here (sic).
                    self.fire(host, id, "ShowToolTip", &[]);
                    self.fire(host, id, "OnMouseOverOut", &[]);
                }
            }
            WindowType::SlotButton | WindowType::CoverButton => {
                if b.enable {
                    self.w_mut(id).button.pressed = false; // SetUp
                }
                let info = self.w(id).slot_button.unwrap();
                self.slot_over_out_item(host, info.owner);
            }
            WindowType::Slot | WindowType::GridSlot => self.slot_over_out_item(host, id),
            _ => {
                self.fire(host, id, "OnMouseOverOut", &[]);
            }
        }
    }

    fn on_mouse_over(&mut self, host: &mut dyn UiHost, id: WindowId) {
        if self.w(id).ty.is_slot() {
            self.slot_over(host, id);
        }
    }

    // ---- CButton ---------------------------------------------------------------------

    pub(crate) fn button_down(&mut self, host: &mut dyn UiHost, id: WindowId) {
        self.w_mut(id).button.pressed = true;
        self.fire(host, id, "DownEvent", &[]);
    }

    /// `CButton::Up` (the visual part is renderer business).
    pub(crate) fn button_up(&mut self, host: &mut dyn UiHost, id: WindowId) {
        self.fire(host, id, "CallEvent", &[]);
    }

    fn button_left_down(&mut self, host: &mut dyn UiHost, id: WindowId) -> bool {
        if !self.w(id).button.enable {
            return true;
        }
        self.w_mut(id).button.pressed = true;
        self.button_down(host, id);
        true
    }

    fn button_left_up(&mut self, host: &mut dyn UiHost, id: WindowId) -> bool {
        let b = self.w(id).button;
        if !b.enable || !b.pressed {
            return true;
        }
        self.w_mut(id).button.pressed = false;
        self.button_up(host, id);
        true
    }

    pub fn button_enable(&mut self, id: WindowId) {
        let b = &mut self.w_mut(id).button;
        b.pressed = false; // SetUp
        b.enable = true;
    }

    pub fn button_disable(&mut self, id: WindowId) {
        self.w_mut(id).button.enable = false;
    }

    pub fn button_set_up(&mut self, id: WindowId) {
        self.w_mut(id).button.pressed = false;
    }

    pub fn button_flash(&mut self, id: WindowId) {
        self.w_mut(id).button.flash = true;
    }

    pub fn button_press_down(&mut self, host: &mut dyn UiHost, id: WindowId) {
        self.button_down(host, id);
    }

    pub fn button_press_up(&mut self, host: &mut dyn UiHost, id: WindowId) {
        self.button_up(host, id);
    }

    pub fn drag_button_set_restrict_area(&mut self, id: WindowId, x: i64, y: i64, w: i64, h: i64) {
        self.w_mut(id).restrict = Rect { left: x, top: y, right: x + w, bottom: y + h };
    }

    /// `CDragButton::OnChangePosition`: clamps into the restrict area; fires
    /// `OnMove` while pressed. Runs inside `update_rect`, after the generic
    /// rectangle computation, and recomputes the rectangle from the parent's.
    fn drag_button_change_position(&mut self, host: &mut dyn UiHost, id: WindowId) {
        let (parent, restrict, wd, ht) = {
            let w = self.w(id);
            (w.parent, w.restrict, w.width, w.height)
        };
        {
            let w = self.w_mut(id);
            w.x = w.x.max(restrict.left);
            w.y = w.y.max(restrict.top);
            w.x = w.x.min(0.max(restrict.right - wd));
            w.y = w.y.min(0.max(restrict.bottom - ht));
            w.rect.left = w.x;
            w.rect.top = w.y;
        }
        if let Some(p) = parent {
            let pr = self.w(p).rect;
            let w = self.w_mut(id);
            w.rect.left += pr.left;
            w.rect.top += pr.top;
        }
        {
            let w = self.w_mut(id);
            w.rect.right = w.rect.left + wd;
            w.rect.bottom = w.rect.top + ht;
        }
        for c in self.w(id).children.clone() {
            self.update_rect(host, c);
        }
        if self.w(id).button.pressed {
            self.fire(host, id, "OnMove", &[]);
        }
    }
}

