//! The music part of `CSoundManager` (`SoundManager.cpp`): three stream
//! slots (`CSoundManagerStream::MUSIC_INSTANCE_MAX_NUM`) that fade in, fade
//! out or fade down to a limit. Only the state lives here; the plugin plays
//! one looping sound entity per slot and gives it the slot's volume.

use m2_assets::paths;

use crate::volume::ratio_to_volume;

/// `CSoundManagerStream::MUSIC_INSTANCE_MAX_NUM`.
pub const MUSIC_SLOTS: usize = 3;

/// Volume step per frame of `FadeInMusic`/`FadeOutMusic`/`FadeLimitOutMusic`.
pub const DEFAULT_FADE_SPEED: f32 = 0.016;
/// The step `FadeOutAllMusic` uses.
pub const FADE_OUT_ALL_SPEED: f32 = 0.01;

/// `EMusicState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MusicState {
    #[default]
    Off,
    FadeIn,
    FadeLimitOut,
    FadeOut,
    Play,
}

/// `TMusicInstance`. `file` replaces `dwMusicFileNameCRC`: the canonical
/// path (`StringPath` + `GetCaseCRC32` compare names case-insensitively
/// with either slash).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MusicSlot {
    pub file: Option<String>,
    pub state: MusicState,
    pub volume: f32,
    pub speed: f32,
    pub limit: f32,
}

/// What the player has to do for a slot.
#[derive(Debug, Clone, PartialEq)]
pub enum MusicAction {
    /// `ms_SoundManagerStream.SetInstance` + `Play(0)`: (re)start `file`
    /// looping in `slot`, replacing what played there.
    Start { slot: usize, file: String },
    /// `StopMusic`.
    Stop { slot: usize },
}

/// `m_MusicInstances`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MusicSlots {
    pub slots: [MusicSlot; MUSIC_SLOTS],
}

impl MusicSlots {
    /// `GetMusicIndex`: the playing slot of `file`.
    pub fn index(&self, file: &str) -> Option<usize> {
        let file = paths::canonical_asset_path(file);
        self.slots.iter().position(|s| s.state != MusicState::Off && s.file.as_deref() == Some(file.as_str()))
    }

    /// `PlayMusic(dwIndex, ...)`.
    pub fn play(&mut self, slot: usize, file: &str, volume: f32, speed: f32) -> Option<MusicAction> {
        let s = self.slots.get_mut(slot)?;
        let file = paths::canonical_asset_path(file);
        *s = MusicSlot { file: Some(file.clone()), state: MusicState::FadeIn, volume, speed, limit: s.limit };
        Some(MusicAction::Start { slot, file })
    }

    /// `FadeInMusic`: fades the music back in if it plays, otherwise fades
    /// everything else out and starts it silent in the first free slot. With
    /// no free slot nothing starts (the C++ fallback is commented out).
    pub fn fade_in(&mut self, file: &str, speed: f32) -> Option<MusicAction> {
        if let Some(i) = self.index(file) {
            self.slots[i].state = MusicState::FadeIn;
            self.slots[i].speed = speed;
            return None;
        }
        // Slots that were off before this call, i.e. still off afterwards.
        self.fade_out_all();
        let free = self.slots.iter().position(|s| s.state == MusicState::Off)?;
        self.play(free, file, 0.0, speed)
    }

    /// `FadeLimitOutMusic`: fades down to `ratio_to_volume(limit_ratio)`.
    pub fn fade_limit_out(&mut self, file: &str, limit_ratio: f32, speed: f32) -> bool {
        let Some(i) = self.index(file) else { return false };
        let s = &mut self.slots[i];
        s.state = MusicState::FadeLimitOut;
        s.speed = speed;
        s.limit = ratio_to_volume(limit_ratio);
        true
    }

    /// `FadeOutMusic`.
    pub fn fade_out(&mut self, file: &str, speed: f32) -> bool {
        let Some(i) = self.index(file) else { return false };
        self.slots[i].state = MusicState::FadeOut;
        self.slots[i].speed = speed;
        true
    }

    /// `FadeOutAllMusic` (and `FadeAll`).
    pub fn fade_out_all(&mut self) {
        for s in self.slots.iter_mut().filter(|s| s.state != MusicState::Off) {
            s.state = MusicState::FadeOut;
            s.speed = FADE_OUT_ALL_SPEED;
        }
    }

    /// The slot part of `__SetMusicVolume`: every slot that is neither off
    /// nor fading out jumps to `volume`.
    pub fn set_volume(&mut self, volume: f32) {
        for s in self.slots.iter_mut() {
            if !matches!(s.state, MusicState::Off | MusicState::FadeOut) {
                s.volume = volume;
            }
        }
    }

    /// `StopMusic`.
    pub fn stop(&mut self, slot: usize) -> Option<MusicAction> {
        let s = self.slots.get_mut(slot)?;
        *s = MusicSlot { limit: s.limit, ..MusicSlot::default() };
        Some(MusicAction::Stop { slot })
    }

    /// The music part of `CSoundManager::Update`, which C++ runs once per
    /// frame. `frames` is the elapsed time in 60 FPS frames, so the fades take
    /// as long as they did at the client's frame rate on any frame rate.
    /// Returns the slots that faded out completely.
    pub fn update(&mut self, frames: f32, music_volume: f32) -> Vec<MusicAction> {
        let mut stopped = Vec::new();
        for i in 0..MUSIC_SLOTS {
            let s = &mut self.slots[i];
            match s.state {
                MusicState::Off | MusicState::Play => {}
                MusicState::FadeIn => {
                    s.volume += s.speed * frames;
                    if s.volume >= music_volume {
                        s.volume = music_volume;
                        s.speed = 0.0;
                        s.state = MusicState::Play;
                    }
                }
                MusicState::FadeLimitOut => {
                    s.volume -= s.speed * frames;
                    if s.volume <= s.limit {
                        s.volume = s.limit;
                        s.speed = 0.0;
                        s.state = MusicState::Play;
                    }
                }
                MusicState::FadeOut => {
                    s.volume -= s.speed * frames;
                    if s.volume <= 0.0 {
                        stopped.extend(self.stop(i));
                    }
                }
            }
        }
        stopped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fade_in_takes_a_free_slot_and_fades_the_rest_out() {
        let mut m = MusicSlots::default();
        assert_eq!(
            m.fade_in("BGM/login_window.mp3", DEFAULT_FADE_SPEED),
            Some(MusicAction::Start { slot: 0, file: "bgm/login_window.mp3".into() })
        );
        assert_eq!(m.slots[0].volume, 0.0);
        // Same file, other spelling: it is the playing one.
        assert_eq!(m.fade_in("bgm\\LOGIN_WINDOW.mp3", DEFAULT_FADE_SPEED), None);
        assert_eq!(m.index("bgm/login_window.mp3"), Some(0));

        assert_eq!(
            m.fade_in("BGM/characterselect.mp3", DEFAULT_FADE_SPEED),
            Some(MusicAction::Start { slot: 1, file: "bgm/characterselect.mp3".into() })
        );
        assert_eq!(m.slots[0].state, MusicState::FadeOut);
        assert_eq!(m.slots[0].speed, FADE_OUT_ALL_SPEED);
    }

    #[test]
    fn fades_reach_their_targets() {
        let mut m = MusicSlots::default();
        m.fade_in("a.mp3", 0.016);
        // 1 / 0.016 = 62.5 frames to full volume.
        assert!(m.update(62.0, 1.0).is_empty());
        assert_eq!(m.slots[0].state, MusicState::FadeIn);
        m.update(1.0, 1.0);
        assert_eq!((m.slots[0].state, m.slots[0].volume), (MusicState::Play, 1.0));

        assert!(m.fade_limit_out("a.mp3", 0.05, 0.1));
        m.update(100.0, 1.0);
        assert_eq!(m.slots[0].state, MusicState::Play);
        assert_eq!(m.slots[0].volume, 0.05);

        assert!(m.fade_out("A.MP3", 0.01));
        assert_eq!(m.update(10.0, 1.0), vec![MusicAction::Stop { slot: 0 }]);
        assert_eq!(m.slots[0].state, MusicState::Off);
        assert_eq!(m.index("a.mp3"), None);
        assert!(!m.fade_out("a.mp3", 0.01));
    }

    #[test]
    fn no_free_slot_starts_nothing() {
        let mut m = MusicSlots::default();
        m.fade_in("a.mp3", 0.016);
        m.fade_in("b.mp3", 0.016);
        m.fade_in("c.mp3", 0.016);
        assert_eq!(m.fade_in("d.mp3", 0.016), None);
        assert!(m.slots.iter().all(|s| s.state == MusicState::FadeOut));
    }

    #[test]
    fn music_volume_skips_fading_out_slots() {
        let mut m = MusicSlots::default();
        m.fade_in("a.mp3", 0.016);
        m.fade_in("b.mp3", 0.016);
        m.set_volume(0.5);
        assert_eq!((m.slots[0].volume, m.slots[1].volume), (0.0, 0.5));
    }
}
