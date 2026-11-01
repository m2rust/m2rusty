//! Slot windows: `PythonSlotWindow.cpp` and `PythonGridSlotWindow.cpp`.
//!
//! A slot window holds a list of slots (index, position, cell size, item
//! state, cool time...). Grid slot windows lay the slots out on a grid and
//! pick several cells at once while an icon is attached to the mouse. The
//! window's buttons (cover, "+" and requirement sign) are real child windows
//! in C++ and are real windows in the [`Ui`] arena here, so they take part in
//! hit testing exactly like the originals.

use crate::event::UiHost;
use crate::window::{FLAG_FLOAT, SlotButtonInfo, Ui, WindowId, WindowType};

pub const ITEM_WIDTH: i64 = 32;
pub const ITEM_HEIGHT: i64 = 32;
pub const SLOT_NUMBER_NONE: u32 = 0xffff_ffff;

pub const SLOT_STYLE_NONE: u32 = 0;
pub const SLOT_STYLE_PICK_UP: u32 = 1;
pub const SLOT_STYLE_SELECT: u32 = 2;

pub const SLOT_STATE_LOCK: u32 = 1 << 0;
pub const SLOT_STATE_CANT_USE: u32 = 1 << 1;
pub const SLOT_STATE_DISABLE: u32 = 1 << 2;
pub const SLOT_STATE_ALWAYS_RENDER_COVER: u32 = 1 << 3;

/// `CSlotWindow::TSlot`.
#[derive(Debug, Clone)]
pub struct Slot {
    pub state: u32,
    pub number: u32,
    /// For items bigger than one cell: the slot that holds the item.
    pub center: u32,
    pub item_index: u32,
    pub is_item: bool,
    pub cool_time: f32,
    pub start_cool_time: f32,
    pub active: bool,
    pub x: i64,
    pub y: i64,
    pub cell_w: i64,
    pub cell_h: i64,
    pub placed_w: u8,
    pub placed_h: u8,
    pub render_base_image: bool,
    /// Has an icon instance (the renderer owns the image).
    pub has_instance: bool,
    /// The count shown in the corner (`CNumberLine`), if any.
    pub number_line: Option<String>,
    pub cover: Option<WindowId>,
    pub button: Option<WindowId>,
    pub sign: Option<WindowId>,
}

impl Slot {
    fn cleared(number: u32, x: i64, y: i64, cell_w: i64, cell_h: i64) -> Self {
        Self {
            state: 0,
            number,
            center: number,
            item_index: 0,
            is_item: false,
            cool_time: 0.0,
            start_cool_time: 0.0,
            active: false,
            x,
            y,
            cell_w,
            cell_h,
            placed_w: 1,
            placed_h: 1,
            render_base_image: true,
            has_instance: false,
            number_line: None,
            cover: None,
            button: None,
            sign: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Grid {
    pub x_count: u32,
    pub y_count: u32,
    /// `m_SlotVector`: slot indices in row-major order.
    pub cells: Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct SlotState {
    pub slot_type: u32,
    pub style: u32,
    pub selected: Vec<u32>,
    pub slots: Vec<Slot>,
    pub tooltip: u32,
    pub use_mode: bool,
    pub usable_item: bool,
    pub grid: Option<Grid>,
}

impl SlotState {
    pub(crate) fn new() -> Self {
        Self {
            slot_type: 0,
            style: SLOT_STYLE_PICK_UP,
            selected: Vec::new(),
            slots: Vec::new(),
            tooltip: SLOT_NUMBER_NONE,
            use_mode: false,
            usable_item: false,
            grid: None,
        }
    }

    pub fn find(&self, number: u32) -> Option<usize> {
        self.slots.iter().position(|s| s.number == number)
    }
}

impl Ui {
    fn ss(&self, s: WindowId) -> &SlotState {
        self.w(s).slot.as_deref().expect("not a slot window")
    }

    fn ss_mut(&mut self, s: WindowId) -> &mut SlotState {
        self.w_mut(s).slot.as_deref_mut().expect("not a slot window")
    }

    pub fn slot_state(&self, s: WindowId) -> &SlotState {
        self.ss(s)
    }

    fn slot_at(&mut self, s: WindowId, number: u32) -> Option<&mut Slot> {
        let i = self.ss(s).find(number)?;
        Some(&mut self.ss_mut(s).slots[i])
    }

    // ---- manage ---------------------------------------------------------------

    pub fn set_slot_type(&mut self, s: WindowId, t: u32) {
        self.ss_mut(s).slot_type = t;
    }

    pub fn set_slot_style(&mut self, s: WindowId, style: u32) {
        self.ss_mut(s).style = style;
    }

    pub fn set_use_mode(&mut self, s: WindowId, flag: bool) {
        self.ss_mut(s).use_mode = flag;
    }

    pub fn set_usable_item(&mut self, s: WindowId, flag: bool) {
        self.ss_mut(s).usable_item = flag;
    }

    pub fn append_slot(&mut self, s: WindowId, index: u32, x: i64, y: i64, cell_w: i64, cell_h: i64) {
        self.ss_mut(s).slots.push(Slot::cleared(index, x, y, cell_w, cell_h));
    }

    pub fn has_slot(&self, s: WindowId, index: u32) -> bool {
        self.ss(s).find(index).is_some()
    }

    pub fn slot_count(&self, s: WindowId) -> u32 {
        self.ss(s).slots.len() as u32
    }

    /// `ClearSlot(TSlot*)`.
    fn clear_slot_at(&mut self, s: WindowId, i: usize) {
        let (cover, button, sign) = {
            let sl = &mut self.ss_mut(s).slots[i];
            sl.active = false;
            sl.placed_w = 1;
            sl.placed_h = 1;
            sl.is_item = false;
            sl.state = 0;
            sl.cool_time = 0.0;
            sl.start_cool_time = 0.0;
            sl.center = 0xffff_ffff;
            sl.item_index = 0;
            sl.render_base_image = true;
            sl.has_instance = false;
            (sl.cover, sl.button, sl.sign)
        };
        for w in [cover, button, sign].into_iter().flatten() {
            self.hide(w);
        }
    }

    pub fn clear_slot(&mut self, s: WindowId, index: u32) {
        if let Some(i) = self.ss(s).find(index) {
            self.clear_slot_at(s, i);
        }
    }

    /// `CSlotWindow::Destroy` (also what `ClearAllSlot` runs, even for grid
    /// slot windows: the C++ call is non-virtual and leaves the grid's cell
    /// table dangling; here the table is emptied with it).
    pub fn slot_destroy(&mut self, _host: &mut dyn UiHost, s: WindowId) {
        for i in 0..self.ss(s).slots.len() {
            self.clear_slot_at(s, i);
            let (cover, button, sign) = {
                let sl = &mut self.ss_mut(s).slots[i];
                sl.number_line = None;
                (sl.cover.take(), sl.button.take(), sl.sign.take())
            };
            for w in [cover, button, sign].into_iter().flatten() {
                self.destroy_window(w);
            }
        }
        let st = self.ss_mut(s);
        st.slots.clear();
        st.grid = None;
        // __Initialize()
        st.slot_type = 0;
        st.style = SLOT_STYLE_PICK_UP;
        st.tooltip = SLOT_NUMBER_NONE;
        st.use_mode = false;
        st.usable_item = false;
    }

    pub fn clear_all_slot(&mut self, host: &mut dyn UiHost, s: WindowId) {
        self.slot_destroy(host, s);
    }

    /// `SetSlot` (no image: `has_image` says whether an icon instance exists).
    pub fn set_slot(&mut self, s: WindowId, index: u32, item: u32, w: u8, h: u8, has_image: bool) {
        let Some(i) = self.ss(s).find(index) else { return };
        {
            let sl = &mut self.ss_mut(s).slots[i];
            if sl.is_item && sl.item_index == item {
                sl.state = 0;
                sl.is_item = true;
                return;
            }
        }
        self.clear_slot_at(s, i);
        let cover = {
            let sl = &mut self.ss_mut(s).slots[i];
            sl.state = 0;
            sl.is_item = true;
            sl.item_index = item;
            sl.has_instance = has_image;
            sl.placed_w = w;
            sl.placed_h = h;
            sl.cover
        };
        if let Some(c) = cover {
            self.show(c);
        }
    }

    pub fn set_slot_count(&mut self, s: WindowId, index: u32, count: u32) {
        let Some(sl) = self.slot_at(s, index) else { return };
        if count == 0 {
            sl.number_line = None;
        } else {
            sl.number_line = Some(count.to_string());
        }
    }

    pub fn set_slot_count_new(&mut self, s: WindowId, index: u32, grade: u32, count: u32) {
        let Some(sl) = self.slot_at(s, index) else { return };
        if count == 0 {
            sl.number_line = None;
            return;
        }
        // Grades above 3 leave the C++ buffer uninitialised; ignore them.
        let text = match grade {
            0 => count.to_string(),
            1 => format!("m{count}"),
            2 => format!("g{count}"),
            3 => "p".to_string(),
            _ => return,
        };
        sl.number_line = Some(text);
    }

    /// `SetSlotCoolTime`: `now` is `CTimer::GetCurrentSecond()`.
    pub fn set_slot_cool_time(&mut self, s: WindowId, index: u32, cool_time: f32, elapsed: f32, now: f32) {
        if let Some(sl) = self.slot_at(s, index) {
            sl.cool_time = cool_time;
            sl.start_cool_time = now - elapsed;
        }
    }

    pub fn activate_slot(&mut self, s: WindowId, index: u32) {
        if let Some(sl) = self.slot_at(s, index) {
            sl.active = true;
        }
    }

    pub fn deactivate_slot(&mut self, s: WindowId, index: u32) {
        if let Some(sl) = self.slot_at(s, index) {
            sl.active = false;
        }
    }

    pub fn lock_slot(&mut self, s: WindowId, index: u32) {
        if let Some(sl) = self.slot_at(s, index) {
            sl.state |= SLOT_STATE_LOCK;
        }
    }

    /// Toggles the lock bit (`^=` in C++, not "clear").
    pub fn unlock_slot(&mut self, s: WindowId, index: u32) {
        if let Some(sl) = self.slot_at(s, index) {
            sl.state ^= SLOT_STATE_LOCK;
        }
    }

    pub fn set_cant_use_slot(&mut self, s: WindowId, index: u32) {
        if let Some(sl) = self.slot_at(s, index) {
            sl.state |= SLOT_STATE_CANT_USE;
        }
    }

    /// Toggles the can't-use bit (`^=` in C++).
    pub fn set_use_slot(&mut self, s: WindowId, index: u32) {
        if let Some(sl) = self.slot_at(s, index) {
            sl.state ^= SLOT_STATE_CANT_USE;
        }
    }

    pub fn enable_slot(&mut self, s: WindowId, index: u32) {
        if let Some(sl) = self.slot_at(s, index) {
            sl.state &= !SLOT_STATE_DISABLE;
        }
    }

    pub fn disable_slot(&mut self, s: WindowId, index: u32) {
        if let Some(sl) = self.slot_at(s, index) {
            sl.state |= SLOT_STATE_DISABLE;
        }
    }

    pub fn show_slot_base_image(&mut self, s: WindowId, index: u32) {
        if let Some(sl) = self.slot_at(s, index) {
            sl.render_base_image = true;
        }
    }

    pub fn hide_slot_base_image(&mut self, s: WindowId, index: u32) {
        if let Some(sl) = self.slot_at(s, index) {
            sl.render_base_image = false;
        }
    }

    pub fn set_always_render_cover_button(&mut self, s: WindowId, index: u32, always: bool) {
        if let Some(sl) = self.slot_at(s, index) {
            if always {
                sl.state |= SLOT_STATE_ALWAYS_RENDER_COVER;
            } else {
                sl.state &= !SLOT_STATE_ALWAYS_RENDER_COVER;
            }
        }
    }

    // ---- selection ------------------------------------------------------------

    /// `SelectSlot`: toggles `index` in the selection (only items can be
    /// selected).
    pub fn select_slot(&mut self, s: WindowId, index: u32) {
        let st = self.ss_mut(s);
        if let Some(p) = st.selected.iter().position(|v| *v == index) {
            st.selected.remove(p);
            return;
        }
        if let Some(i) = st.find(index)
            && st.slots[i].is_item
        {
            st.selected.push(index);
        }
    }

    pub fn is_selected_slot(&self, s: WindowId, index: u32) -> bool {
        self.ss(s).selected.contains(&index)
    }

    pub fn clear_selected(&mut self, s: WindowId) {
        self.ss_mut(s).selected.clear();
    }

    pub fn selected_slot_count(&self, s: WindowId) -> u32 {
        self.ss(s).selected.len() as u32
    }

    /// `GetSelectedSlotNumber`: `u32::MAX` when out of range.
    pub fn selected_slot_number(&self, s: WindowId, i: u32) -> u32 {
        self.ss(s).selected.get(i as usize).copied().unwrap_or(u32::MAX)
    }

    // ---- buttons --------------------------------------------------------------

    fn make_slot_button(&mut self, ty: WindowType, owner: WindowId, number: u32, left: bool, right: bool) -> WindowId {
        let b = self.create_internal(ty);
        self.w_mut(b).slot_button = Some(SlotButtonInfo { owner, number, left_enable: left, right_enable: right });
        b
    }

    fn visual_size(&mut self, host: &mut dyn UiHost, id: WindowId, path: &str) -> bool {
        match host.image_size(path) {
            Some((w, h)) => {
                self.set_size(id, w as i64, h as i64);
                true
            }
            None => false,
        }
    }

    /// `SetCoverButton`.
    #[allow(clippy::too_many_arguments)]
    pub fn set_cover_button(
        &mut self,
        host: &mut dyn UiHost,
        s: WindowId,
        index: u32,
        up: &str,
        over: &str,
        down: &str,
        disable: &str,
        left: bool,
        right: bool,
    ) {
        let Some(i) = self.ss(s).find(index) else { return };
        let number = self.ss(s).slots[i].number;
        let cover = match self.ss(s).slots[i].cover {
            Some(c) => c,
            None => {
                let c = self.make_slot_button(WindowType::CoverButton, s, number, left, right);
                self.set_parent(c, s);
                self.ss_mut(s).slots[i].cover = Some(c);
                c
            }
        };
        if let Some(info) = self.w_mut(cover).slot_button.as_mut() {
            info.left_enable = left;
            info.right_enable = right;
        }
        for p in [up, over, down, disable] {
            self.visual_size(host, cover, p);
        }
        self.button_enable(cover);
        self.show(cover);
        if let Some(b) = self.ss(s).slots[i].button {
            self.set_top_child(host, s, b);
        }
    }

    pub fn enable_cover_button(&mut self, s: WindowId, index: u32) {
        if let Some(c) = self.slot_at(s, index).and_then(|sl| sl.cover) {
            self.button_enable(c);
        }
    }

    pub fn disable_cover_button(&mut self, s: WindowId, index: u32) {
        if let Some(c) = self.slot_at(s, index).and_then(|sl| sl.cover) {
            self.button_disable(c);
        }
    }

    /// `IsDisableCoverButton` (sic: returns `m_bEnable`).
    pub fn is_disable_cover_button(&mut self, s: WindowId, index: u32) -> bool {
        match self.slot_at(s, index).and_then(|sl| sl.cover) {
            Some(c) => self.w(c).button.enable,
            None => false,
        }
    }

    /// `AppendSlotButton`: a "+" button for every slot.
    pub fn append_slot_button(&mut self, host: &mut dyn UiHost, s: WindowId, up: &str, over: &str, down: &str) {
        for i in 0..self.ss(s).slots.len() {
            let (number, x, y, existing) = {
                let sl = &self.ss(s).slots[i];
                (sl.number, sl.x, sl.y, sl.button)
            };
            let b = match existing {
                Some(b) => b,
                None => {
                    let b = self.make_slot_button(WindowType::SlotButton, s, number, true, true);
                    self.add_flag(b, FLAG_FLOAT);
                    self.set_parent(b, s);
                    self.ss_mut(s).slots[i].button = Some(b);
                    b
                }
            };
            for p in [up, over, down] {
                self.visual_size(host, b, p);
            }
            self.set_position(host, b, x + 1, y + 19);
            self.hide(b);
        }
    }

    pub fn show_slot_button(&mut self, s: WindowId, index: u32) {
        if let Some(b) = self.slot_at(s, index).and_then(|sl| sl.button) {
            self.show(b);
        }
    }

    pub fn hide_all_slot_button(&mut self, s: WindowId) {
        let bs: Vec<_> = self.ss(s).slots.iter().filter_map(|sl| sl.button).collect();
        for b in bs {
            self.hide(b);
        }
    }

    pub fn append_requirement_sign_image(&mut self, host: &mut dyn UiHost, s: WindowId, path: &str) {
        for i in 0..self.ss(s).slots.len() {
            let sign = match self.ss(s).slots[i].sign {
                Some(b) => b,
                None => {
                    let b = self.create_internal(WindowType::ImageBox);
                    self.set_parent(b, s);
                    self.ss_mut(s).slots[i].sign = Some(b);
                    b
                }
            };
            self.visual_size(host, sign, path);
            self.hide(sign);
        }
    }

    pub fn show_requirement_sign(&mut self, s: WindowId, index: u32) {
        if let Some(b) = self.slot_at(s, index).and_then(|sl| sl.sign) {
            self.show(b);
        }
    }

    pub fn hide_requirement_sign(&mut self, s: WindowId, index: u32) {
        if let Some(b) = self.slot_at(s, index).and_then(|sl| sl.sign) {
            self.hide(b);
        }
    }

    // ---- refresh ----------------------------------------------------------------

    /// `RefreshSlot`.
    pub fn refresh_slot(&mut self, host: &mut dyn UiHost, s: WindowId) {
        if self.w(s).ty == WindowType::GridSlot {
            self.grid_refresh(s);
        }
        if self.is_rendering(s)
            && let Some(i) = self.picked_slot(s)
        {
            let number = self.ss(s).slots[i].number;
            self.slot_over_out_item(host, s);
            self.slot_over_in_item(host, s, number);
        }
    }

    // ---- picking ----------------------------------------------------------------

    /// `GetSelectedSlotPointer`: the first slot whose plain cell contains the
    /// mouse.
    pub fn selected_slot_pointer(&self, s: WindowId) -> Option<usize> {
        let (lx, ly) = self.get_mouse_local_position(s);
        self.ss(s)
            .slots
            .iter()
            .position(|sl| lx >= sl.x && ly >= sl.y && lx <= sl.x + sl.cell_w && ly <= sl.y + sl.cell_h)
    }

    /// `GetPickedSlotPointer` (virtual; the grid window overrides it).
    pub fn picked_slot(&self, s: WindowId) -> Option<usize> {
        if self.w(s).ty == WindowType::GridSlot {
            self.grid_picked_slot(s)
        } else {
            self.base_picked_slot(s)
        }
    }

    fn base_picked_slot(&self, s: WindowId) -> Option<usize> {
        let r = self.w(s).rect;
        let lx = self.mouse.0 - r.left;
        let ly = self.mouse.1 - r.top;
        self.ss(s).slots.iter().position(|sl| {
            let (mut cw, mut ch) = (sl.cell_w, sl.cell_h);
            if sl.is_item {
                cw = cw.max(sl.placed_w as i64 * ITEM_WIDTH);
                ch = ch.max(sl.placed_h as i64 * ITEM_HEIGHT);
            }
            lx >= sl.x && ly >= sl.y && lx <= sl.x + cw && ly <= sl.y + ch
        })
    }

    // ---- script-visible callbacks ---------------------------------------------------

    pub(crate) fn slot_over_in_item(&mut self, host: &mut dyn UiHost, s: WindowId, number: u32) -> bool {
        let Some(i) = self.ss(s).find(number) else { return false };
        let sl = &self.ss(s).slots[i];
        if !sl.is_item {
            return false;
        }
        if sl.number == self.ss(s).tooltip {
            return true;
        }
        self.ss_mut(s).tooltip = number;
        self.fire(host, s, "OnOverInItem", &[number as i64]);
        true
    }

    pub(crate) fn slot_over_out_item(&mut self, host: &mut dyn UiHost, s: WindowId) {
        if self.ss(s).tooltip == SLOT_NUMBER_NONE {
            return;
        }
        self.ss_mut(s).tooltip = SLOT_NUMBER_NONE;
        self.fire(host, s, "OnOverOutItem", &[]);
    }

    pub(crate) fn slot_over(&mut self, host: &mut dyn UiHost, s: WindowId) {
        if self.point == Some(s)
            && let Some(i) = self.picked_slot(s)
        {
            let number = self.ss(s).slots[i].number;
            if self.slot_over_in_item(host, s, number) {
                return;
            }
        }
        self.slot_over_out_item(host, s);
    }

    fn on_select_item_slot(&mut self, host: &mut dyn UiHost, s: WindowId, number: u32) {
        self.fire(host, s, "OnSelectItemSlot", &[number as i64]);
        if self.attaching {
            self.slot_over_out_item(host, s);
        }
    }

    fn on_select_empty_slot(&mut self, host: &mut dyn UiHost, s: WindowId, number: u32) {
        self.fire(host, s, "OnSelectEmptySlot", &[number as i64]);
    }

    pub(crate) fn slot_left_down(&mut self, host: &mut dyn UiHost, s: WindowId) -> bool {
        let Some(i) = self.picked_slot(s) else {
            self.deattach_icon(host);
            return true;
        };
        let (is_item, state, number) = {
            let sl = &self.ss(s).slots[i];
            (sl.is_item, sl.state, sl.number)
        };
        if is_item && state & SLOT_STATE_LOCK == 0 {
            self.on_select_item_slot(host, s, number);
        } else {
            self.on_select_empty_slot(host, s, number);
        }
        true
    }

    pub(crate) fn slot_left_up(&mut self, host: &mut dyn UiHost, s: WindowId) -> bool {
        if self.attaching && self.is_dragging() && self.is_in(s) {
            if let Some(p) = self.point
                && self.w(p).ty.is_slot()
            {
                let Some(i) = self.picked_slot(p) else {
                    self.deattach_icon(host);
                    return true;
                };
                let (is_item, number) = {
                    let sl = &self.ss(p).slots[i];
                    (sl.is_item, sl.number)
                };
                if is_item {
                    self.on_select_item_slot(host, p, number);
                } else {
                    self.on_select_empty_slot(host, p, number);
                }
                return true;
            }
            return false;
        }
        false
    }

    pub(crate) fn slot_right_down(&mut self, host: &mut dyn UiHost, s: WindowId) -> bool {
        let Some(i) = self.picked_slot(s) else { return true };
        let (is_item, number) = {
            let sl = &self.ss(s).slots[i];
            (sl.is_item, sl.number)
        };
        if is_item {
            self.fire(host, s, "OnUnselectItemSlot", &[number as i64]);
        } else {
            self.fire(host, s, "OnUnselectEmptySlot", &[number as i64]);
        }
        true
    }

    pub(crate) fn slot_use(&mut self, host: &mut dyn UiHost, s: WindowId) {
        if let Some(i) = self.picked_slot(s) {
            let (is_item, number) = {
                let sl = &self.ss(s).slots[i];
                (sl.is_item, sl.number)
            };
            if is_item {
                self.fire(host, s, "OnUseSlot", &[number as i64]);
            }
        }
    }

    /// `OnPressedSlotButton(type, number)` with `isLeft = TRUE`.
    fn on_pressed_slot_button(&mut self, host: &mut dyn UiHost, s: WindowId, cover: bool, number: u32) {
        if !cover {
            self.fire(host, s, "OnPressedSlotButton", &[number as i64]);
        } else {
            self.slot_left_down(host, s);
        }
    }

    // ---- slot / cover buttons -------------------------------------------------------

    fn sb(&self, id: WindowId) -> SlotButtonInfo {
        self.w(id).slot_button.expect("slot button")
    }

    pub(crate) fn slot_button_left_down(&mut self, host: &mut dyn UiHost, id: WindowId) -> bool {
        if !self.w(id).button.enable {
            return true;
        }
        if self.attaching {
            return true;
        }
        self.w_mut(id).button.pressed = true;
        self.button_down(host, id);
        true
    }

    pub(crate) fn slot_button_left_up(&mut self, host: &mut dyn UiHost, id: WindowId) -> bool {
        let b = self.w(id).button;
        if !b.enable || !b.pressed {
            return true;
        }
        let info = self.sb(id);
        let cover = self.w(id).ty == WindowType::CoverButton;
        self.on_pressed_slot_button(host, info.owner, cover, info.number);
        true
    }

    pub(crate) fn cover_left_down(&mut self, host: &mut dyn UiHost, id: WindowId) -> bool {
        if !self.w(id).button.enable {
            return true;
        }
        let info = self.sb(id);
        if info.left_enable {
            self.slot_button_left_down(host, id);
        } else {
            self.slot_left_down(host, info.owner);
        }
        true
    }

    pub(crate) fn cover_left_up(&mut self, host: &mut dyn UiHost, id: WindowId) -> bool {
        if !self.w(id).button.enable {
            return true;
        }
        let info = self.sb(id);
        if info.left_enable {
            self.slot_button_left_up(host, id);
            self.slot_left_down(host, info.owner);
        } else {
            self.slot_left_up(host, info.owner);
        }
        true
    }

    pub(crate) fn cover_right_down(&mut self, host: &mut dyn UiHost, id: WindowId) -> bool {
        if !self.w(id).button.enable {
            return true;
        }
        if self.sb(id).right_enable {
            self.slot_button_left_down(host, id);
        }
        true
    }

    pub(crate) fn cover_right_up(&mut self, host: &mut dyn UiHost, id: WindowId) -> bool {
        if !self.w(id).button.enable {
            return true;
        }
        let info = self.sb(id);
        self.slot_right_down(host, info.owner);
        if info.right_enable {
            self.slot_button_left_up(host, id);
        }
        true
    }

    // ---- grid -------------------------------------------------------------------------

    /// `ArrangeGridSlot`.
    #[allow(clippy::too_many_arguments)]
    pub fn arrange_grid_slot(
        &mut self,
        host: &mut dyn UiHost,
        s: WindowId,
        start: u32,
        x_count: u32,
        y_count: u32,
        x_size: i64,
        y_size: i64,
        x_gap: i64,
        y_gap: i64,
    ) {
        self.slot_destroy(host, s);
        let mut cells = vec![0usize; (x_count * y_count) as usize];
        for x in 0..x_count {
            for y in 0..y_count {
                let index = start.wrapping_add(x).wrapping_add(y.wrapping_mul(x_count));
                let px = x as i64 * (x_size + x_gap);
                let py = y as i64 * (y_size + y_gap);
                self.append_slot(s, index, px, py, x_size, y_size);
                cells[(x + y * x_count) as usize] = self.ss(s).slots.len() - 1;
            }
        }
        self.ss_mut(s).grid = Some(Grid { x_count, y_count, cells });
        let width = x_count as i64 * (x_size + x_gap);
        let height = y_count as i64 * (y_size + y_gap);
        self.set_size(s, width, height);
    }

    fn grid_cell(&self, s: WindowId, ix: i32, iy: i32) -> Option<usize> {
        let g = self.ss(s).grid.as_ref()?;
        let idx = (ix as u32).wrapping_add((iy as u32).wrapping_mul(g.x_count));
        g.cells.get(idx as usize).copied()
    }

    fn grid_refresh(&mut self, s: WindowId) {
        let Some((xc, yc)) = self.ss(s).grid.as_ref().map(|g| (g.x_count, g.y_count)) else { return };
        for x in 0..xc {
            for y in 0..yc {
                if let Some(i) = self.grid_cell(s, x as i32, y as i32) {
                    let n = self.ss(s).slots[i].number;
                    self.ss_mut(s).slots[i].center = n;
                }
            }
        }
        for x in 0..xc {
            for y in 0..yc {
                let Some(i) = self.grid_cell(s, x as i32, y as i32) else { continue };
                let (is_item, n, item, pw, ph) = {
                    let sl = &self.ss(s).slots[i];
                    (sl.is_item, sl.number, sl.item_index, sl.placed_w, sl.placed_h)
                };
                if is_item {
                    for xs in 0..pw as u32 {
                        for ys in 0..ph as u32 {
                            if let Some(j) = self.grid_cell(s, x.wrapping_add(xs) as i32, y.wrapping_add(ys) as i32) {
                                let sub = &mut self.ss_mut(s).slots[j];
                                sub.center = n;
                                sub.item_index = item;
                            }
                        }
                    }
                } else {
                    self.ss_mut(s).slots[i].item_index = 0;
                }
            }
        }
    }

    fn grid_picked_position(&self, s: WindowId, lx: i64, ly: i64) -> Option<(i32, i32)> {
        let g = self.ss(s).grid.as_ref()?;
        for x in 0..g.x_count {
            for y in 0..g.y_count {
                let Some(i) = self.grid_cell(s, x as i32, y as i32) else { continue };
                let sl = &self.ss(s).slots[i];
                if lx >= sl.x && ly >= sl.y && lx <= sl.x + sl.cell_w && ly <= sl.y + sl.cell_h {
                    return Some((x as i32, y as i32));
                }
            }
        }
        None
    }

    /// `GetPickedSlotList`: the cells an attached icon of `iw` x `ih` cells
    /// would cover at the mouse position.
    pub fn grid_picked_slot_list(&self, s: WindowId, iw: i32, ih: i32) -> Option<Vec<usize>> {
        let (lx, ly) = self.get_mouse_local_position(s);
        let (wd, ht) = self.w(s).size();
        if lx < 0 || ly < 0 || lx >= wd || ly >= ht {
            return None;
        }
        let (xc, yc) = self.ss(s).grid.as_ref().map(|g| (g.x_count, g.y_count))?;
        let (ix, iy) = self.grid_picked_position(s, lx, ly)?;
        let half = iw / 2;
        let mut x_start = ix - (half - half % 2);
        let mut x_end = ix + half;
        let (mut y_start, mut y_end) = match ih {
            1 => (iy, iy),
            2 => (iy, iy + 1),
            3 => (iy - 1, iy + 1),
            _ => (0, 0),
        };
        if x_start < 0 {
            x_end += -x_start;
            x_start = 0;
        }
        if y_start < 0 {
            y_end += -y_start;
            y_start = 0;
        }
        if (x_end as u32) >= xc {
            let t = (x_end as u32).wrapping_sub(xc).wrapping_add(1) as i32;
            x_start = x_start.wrapping_sub(t);
            x_end = x_end.wrapping_sub(t);
        }
        if (y_end as u32) >= yc {
            let t = (y_end as u32).wrapping_sub(yc).wrapping_add(1) as i32;
            y_start = y_start.wrapping_sub(t);
            y_end = y_end.wrapping_sub(t);
        }
        let mut list = Vec::new();
        for i in x_start..=x_end {
            for j in y_start..=y_end {
                if let Some(c) = self.grid_cell(s, i, j) {
                    list.push(c);
                }
            }
        }
        let st = self.ss(s);
        if st.use_mode && list.len() == 1 {
            let main = list[0];
            let main_center = st.slots[main].center;
            for &c in &st.grid.as_ref().unwrap().cells {
                if st.slots[c].center == main_center && !list.contains(&c) {
                    list.push(c);
                }
            }
        }
        if list.is_empty() { None } else { Some(list) }
    }

    fn grid_picked_slot(&self, s: WindowId) -> Option<usize> {
        if !self.attaching {
            return self.base_picked_slot(s);
        }
        let list = self.grid_picked_slot_list(s, self.attach_size.0 as i32, self.attach_size.1 as i32)?;
        let st = self.ss(s);
        let mut min: Option<usize> = None;
        for &i in &list {
            match min {
                None => min = Some(i),
                Some(m) => {
                    if st.slots[i].number < st.slots[m].number || (!st.slots[m].is_item && st.slots[i].is_item) {
                        // C++: `if (n < min.n) min = s; else if (!min.isItem && s.isItem) min = s;`
                        min = Some(i);
                    }
                }
            }
        }
        let min = min?;
        let center = st.find(st.slots[min].center)?;
        if self.attach_slot == st.slots[center].number {
            Some(min)
        } else {
            Some(center)
        }
    }

    /// `CheckMoving`: can the attached item be dropped on `list`?
    pub fn grid_check_moving(&self, s: WindowId, slot_number: u32, item_index: u32, list: &[usize]) -> bool {
        let st = self.ss(s);
        if st.style != SLOT_STYLE_PICK_UP {
            return true;
        }
        for &i in list {
            let sl = &st.slots[i];
            if slot_number != sl.center && (sl.item_index != 0 || sl.center != sl.number) && item_index != sl.item_index {
                return false;
            }
        }
        true
    }
}
