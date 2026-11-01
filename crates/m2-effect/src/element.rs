//! `CEffectElementBaseInstance` (start delay, local time, active flag) and
//! `CFrameController`.

/// The state `CEffectElementBaseInstance` keeps for every element.
#[derive(Debug, Clone, PartialEq)]
pub struct ElementState {
    pub active: bool,
    pub local_time: f32,
    pub elapsed_time: f32,
    pub remaining_time: f32,
    pub started: bool,
}

impl ElementState {
    /// `SetDataPointer`: the element waits `start_time` seconds.
    pub fn new(start_time: f32) -> Self {
        Self { active: true, local_time: 0.0, elapsed_time: 0.0, remaining_time: start_time, started: start_time <= 0.0 }
    }

    /// `CEffectElementBaseInstance::Initialize` (what `Destroy` ends with):
    /// everything back to zero, not started, active.
    pub fn reset(&mut self) {
        *self = Self { active: true, local_time: 0.0, elapsed_time: 0.0, remaining_time: 0.0, started: false };
    }

    /// The part of `Update` before `OnUpdate`: `Some(true)` when the element
    /// is still waiting (the frame is consumed), `None` when `OnUpdate`
    /// must run.
    pub(crate) fn begin(&mut self, dt: f32) -> Option<bool> {
        if self.started {
            self.elapsed_time = dt;
            self.local_time += dt;
            return None;
        }
        self.remaining_time -= dt;
        if self.remaining_time <= 0.0 {
            self.started = true;
        }
        Some(true)
    }
}

/// `CFrameController`: steps a frame counter at a fixed rate.
#[derive(Debug, Clone, PartialEq)]
pub struct FrameController {
    active: bool,
    cur_frame: u32,
    last_frame_time: f32,
    loop_count: i32,
    is_loop: bool,
    max_frame: u32,
    frame_time: f32,
    start_frame: u32,
}

impl Default for FrameController {
    fn default() -> Self {
        Self {
            active: true,
            cur_frame: 0,
            last_frame_time: 0.0,
            loop_count: 0,
            is_loop: false,
            max_frame: 0,
            frame_time: 0.0,
            start_frame: 0,
        }
    }
}

impl FrameController {
    /// `Clear`.
    pub fn clear(&mut self) {
        self.active = true;
        self.cur_frame = 0;
        self.last_frame_time = 0.0;
    }

    /// `Update`.
    pub fn update(&mut self, elapsed: f32) {
        self.last_frame_time -= elapsed;
        for _ in 0..20 {
            if self.last_frame_time < 0.0 {
                self.last_frame_time += self.frame_time;
                self.cur_frame = self.cur_frame.wrapping_add(1);
                if self.cur_frame >= self.max_frame {
                    // `m_isLoop && --m_iLoopCount != 0` short-circuits.
                    let again = self.is_loop && {
                        self.loop_count = self.loop_count.wrapping_sub(1);
                        self.loop_count != 0
                    };
                    if again {
                        self.loop_count = self.loop_count.max(0);
                        self.cur_frame = 0;
                    } else {
                        self.loop_count = 1;
                        self.cur_frame = 0;
                        self.active = false;
                        return;
                    }
                }
            } else {
                break;
            }
        }
    }

    /// `GetCurrentFrame`: a `BYTE`, so frames past 255 wrap.
    pub fn current_frame(&self) -> u8 {
        self.cur_frame as u8
    }

    pub fn set_max_frame(&mut self, v: u32) {
        self.max_frame = v;
    }

    /// `SetFrameTime` (also restarts the countdown).
    pub fn set_frame_time(&mut self, t: f32) {
        self.frame_time = t;
        self.last_frame_time = t;
    }

    pub fn set_start_frame(&mut self, v: u32) {
        self.start_frame = v;
    }

    pub fn set_loop(&mut self, v: bool) {
        self.is_loop = v;
    }

    pub fn set_loop_count(&mut self, v: i32) {
        self.loop_count = v;
    }

    /// `isActive(dwMainFrame)`.
    pub fn is_active(&self, main_frame: u32) -> bool {
        if main_frame < self.start_frame {
            return false;
        }
        self.active
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waits_for_start_time() {
        let mut s = ElementState::new(0.5);
        assert_eq!(s.begin(0.3), Some(true));
        assert!(!s.started);
        assert_eq!(s.begin(0.3), Some(true));
        assert!(s.started);
        assert_eq!(s.begin(0.1), None);
        assert_eq!(s.local_time, 0.1);
    }

    #[test]
    fn frame_controller_stops_after_last_frame() {
        let mut c = FrameController::default();
        c.set_max_frame(2);
        c.set_frame_time(0.1);
        c.update(0.15);
        assert_eq!(c.current_frame(), 1);
        c.update(0.1);
        assert!(!c.is_active(0));
    }
}
