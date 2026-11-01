//! Small value types behind the image windows: ratio blending
//! (`GrpRatioInstance`), colour transitions (`ColorTransitionHelper`), the
//! mark image cell (`GrpMarkInstance`), the expanded image's rendering rect
//! (`GrpExpandedImageInstance`), and the cool-time wedge of
//! `CPythonGraphic::RenderCoolTimeBox`. Time is always passed in (milliseconds),
//! never read from a global clock.

/// `CGraphicRatioInstance`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RatioInstance {
    cur: f32,
    src: f32,
    dst: f32,
    base_time: u32,
    blend_time: u32,
}

impl RatioInstance {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn set_ratio(&mut self, ratio: f32) {
        self.cur = ratio;
        self.src = ratio;
        self.dst = ratio;
    }

    pub fn blend_ratio(&mut self, now_ms: u32, blend_time: u32, dst: f32) {
        self.base_time = now_ms;
        self.blend_time = blend_time;
        self.src = self.cur;
        self.dst = dst;
    }

    pub fn update(&mut self, now_ms: u32) {
        let elapsed = now_ms.wrapping_sub(self.base_time);
        if elapsed < self.blend_time {
            let diff = self.dst - self.src;
            let rate = elapsed as f32 / self.blend_time as f32;
            self.cur = diff * rate + self.src;
        } else {
            self.cur = self.dst;
        }
    }

    pub fn current(&self) -> f32 {
        self.cur
    }
}

/// `CColorTransitionHelper`. Colours are floats in 0..1; `cur_color` is ARGB.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ColorTransition {
    cur_color: u32,
    start_time: u32,
    duration: u32,
    started: bool,
    src: [f32; 4],
    dst: [f32; 4],
}

impl ColorTransition {
    /// `Clear(r, g, b, a)`.
    pub fn clear(&mut self, r: f32, g: f32, b: f32, a: f32) {
        self.src = [r, g, b, a];
        self.dst = [r, g, b, a];
        self.cur_color = 0;
        self.start_time = 0;
        self.duration = 0;
    }

    pub fn set_src_color(&mut self, r: f32, g: f32, b: f32, a: f32) {
        self.src = [r, g, b, a];
    }

    pub fn set_transition(&mut self, r: f32, g: f32, b: f32, a: f32, duration: u32) {
        self.dst = [r, g, b, a];
        self.duration = duration;
    }

    pub fn start(&mut self, now_ms: u32) {
        self.started = true;
        self.start_time = now_ms;
    }

    pub fn is_started(&self) -> bool {
        self.started
    }

    pub fn color(&self) -> u32 {
        self.cur_color
    }

    /// `Update`: returns whether the transition is still running.
    pub fn update(&mut self, now_ms: u32) -> bool {
        let elapsed = now_ms.wrapping_sub(self.start_time);
        let mut p = elapsed as f32 / self.duration as f32;
        p = p.max(0.0);
        p = p.min(1.0);
        let cur: Vec<f32> = (0..4).map(|i| self.src[i] + (self.dst[i] - self.src[i]) * p).collect();
        let byte = |v: f32| (v * 255.0) as u32 & 0xff;
        self.cur_color = (byte(cur[3]) << 24) | (byte(cur[0]) << 16) | (byte(cur[1]) << 8) | byte(cur[2]);
        if p == 1.0 && (0..4).all(|i| cur[i] == self.dst[i]) {
            self.started = false;
            return false;
        }
        true
    }
}

pub const MARK_WIDTH: i32 = 16;
pub const MARK_HEIGHT: i32 = 12;

/// Source rectangle `(left, top, right, bottom)` of mark `index` in a mark
/// sheet `image_width` pixels wide (`CGraphicMarkInstance::OnRender`);
/// `None` when the sheet is narrower than one mark.
pub fn mark_cell(image_width: i32, index: u32) -> Option<(i32, i32, i32, i32)> {
    let cols = (image_width / MARK_WIDTH).max(0) as u32;
    if cols == 0 {
        return None;
    }
    let col = (index % cols) as i32;
    let row = (index / cols) as i32;
    let left = col * MARK_WIDTH;
    let top = row * MARK_HEIGHT;
    Some((left, top, left + MARK_WIDTH, top + MARK_HEIGHT))
}

/// `CGraphicExpandedImageInstance::SetRenderingRect`: fractions of the image
/// size become pixel offsets.
pub fn expanded_rendering_rect(width: i32, height: i32, left: f32, top: f32, right: f32, bottom: f32) -> [f32; 4] {
    let (w, h) = (width as f32, height as f32);
    [w * left, h * top, w * right, h * bottom]
}

/// `CExpandedImageBox::SetScale` resizes the window to the scaled image.
pub fn expanded_window_size(width: i32, height: i32, sx: f32, sy: f32) -> (i64, i64) {
    ((width as f32 * sx) as i64, (height as f32 * sy) as i64)
}

/// The fan of points of `RenderCoolTimeBox(cx, cy, radius, time)`: the
/// triangle fan around `(cx, cy)` that darkens the not-yet-cooled part of a
/// slot. Empty when `time >= 1`.
pub fn cool_time_fan(cx: f32, cy: f32, radius: f32, time: f32) -> Vec<(f32, f32)> {
    if time >= 1.0 {
        return Vec::new();
    }
    let time = time.max(0.0);
    const BOX: [(f32, f32); 8] =
        [(-1.0, -1.0), (-1.0, 0.0), (-1.0, 1.0), (0.0, 1.0), (1.0, 1.0), (1.0, 0.0), (1.0, -1.0), (0.0, -1.0)];
    let mut tri_count = (8.0 - 8.0 * time) as i32;
    let last = (8.0 - 8.0 * time) - tri_count as f32;
    let mut v = vec![(cx, cy), (cx, cy - radius)];
    for p in BOX.iter().take(tri_count.max(0) as usize) {
        v.push((cx + p.0 * radius, cy + p.1 * radius));
    }
    if last > 0.0 {
        let prev = BOX[((tri_count - 1 + 8) % 8) as usize];
        let cur = BOX[((tri_count + 8) % 8) as usize];
        v.push((
            cx + ((cur.0 - prev.0) * last + prev.0) * radius,
            cy + ((cur.1 - prev.1) * last + prev.1) * radius,
        ));
        tri_count += 1;
    }
    let _ = tri_count;
    v
}
