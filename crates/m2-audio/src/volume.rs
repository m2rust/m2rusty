//! The volume part of `CSoundManager` (`SoundManager.cpp`) and the two
//! `metin2.cfg` keys `CPythonSystem::LoadConfig` reads for it.

use m2_formats::script::{atof_f32, atoi};

/// `__ConvertRatioVolumeToApplyVolume`: below 0.1 the ratio is used as is,
/// above it is `10^(ratio - 1)`. The curve jumps at 0.1 (0.0999 stays 0.0999,
/// 0.1 becomes 0.126); kept.
pub fn ratio_to_volume(ratio: f32) -> f32 {
    if ratio < 0.1 { ratio } else { 10f32.powf(-1.0 + ratio) }
}

/// `__ConvertGradeVolumeToApplyVolume`: grade 0..=5, as the options window
/// and `VOICE_VOLUME` store it.
pub fn grade_to_volume(grade: i32) -> f32 {
    ratio_to_volume(grade as f32 / 5.0)
}

/// `m_fSoundVolume`, `m_fMusicVolume`, their backups and `m_isSoundDisable`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Volumes {
    sound: f32,
    music: f32,
    backup_sound: f32,
    backup_music: f32,
    disabled: bool,
}

impl Default for Volumes {
    fn default() -> Self {
        Volumes { sound: 1.0, music: 1.0, backup_sound: 0.0, backup_music: 0.0, disabled: false }
    }
}

impl Volumes {
    /// `GetSoundVolume`.
    pub fn sound(&self) -> f32 {
        self.sound
    }

    /// `GetMusicVolume`.
    pub fn music(&self) -> f32 {
        self.music
    }

    /// Between [`save`](Self::save) and [`restore`](Self::restore).
    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    /// `SetSoundVolume`: clamped to `0..=1`. While disabled only the backup
    /// changes (unclamped, as in C++).
    pub fn set_sound(&mut self, volume: f32) {
        if self.disabled {
            self.backup_sound = volume;
            return;
        }
        self.sound = volume.clamp(0.0, 1.0);
        self.backup_sound = self.sound;
    }

    /// The volume part of `__SetMusicVolume`: the clamped volume the playing
    /// music takes, or `None` while disabled (only the backup changes).
    pub fn set_music(&mut self, volume: f32) -> Option<f32> {
        if self.disabled {
            self.backup_music = volume;
            return None;
        }
        self.music = volume.clamp(0.0, 1.0);
        self.backup_music = self.music;
        Some(self.music)
    }

    /// `SaveVolume`: silences sound and music, remembering both. A second
    /// call before [`restore`](Self::restore) does nothing.
    pub fn save(&mut self) -> bool {
        if self.disabled {
            return false;
        }
        let (music, sound) = (self.music, self.sound);
        self.set_music(0.0);
        self.set_sound(0.0);
        self.backup_music = music;
        self.backup_sound = sound;
        self.disabled = true;
        true
    }

    /// `RestoreVolume`: the volumes saved (or set while disabled). Returns
    /// the music volume to apply to the playing music.
    pub fn restore(&mut self) -> f32 {
        self.disabled = false;
        let music = self.set_music(self.backup_music).unwrap_or(self.music);
        self.set_sound(self.backup_sound);
        music
    }
}

/// `MUSIC_VOLUME` and `VOICE_VOLUME` from `metin2.cfg`
/// (`CPythonSystem::LoadConfig`); the defaults are `SetDefaultConfig`'s.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AudioConfig {
    /// Linear music volume (`SetMusicVolume`).
    pub music_volume: f32,
    /// Sound grade 0..=5 (`SetSoundVolumeGrade`).
    pub sound_grade: i32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        AudioConfig { music_volume: 1.0, sound_grade: 5 }
    }
}

impl AudioConfig {
    /// Reads `metin2.cfg`. Like the C++ loop (`sscanf(" %s %s") == EOF`),
    /// reading stops at the first line without a token.
    pub fn parse(text: &[u8]) -> Self {
        let mut config = AudioConfig::default();
        for line in text.split(|&b| b == b'\n') {
            let mut tokens = line.split(|b| b.is_ascii_whitespace()).filter(|t| !t.is_empty());
            let Some(command) = tokens.next() else { break };
            let value = tokens.next().unwrap_or_default();
            if command.eq_ignore_ascii_case(b"MUSIC_VOLUME") {
                config.music_volume = parse_music_volume(value);
            } else if command.eq_ignore_ascii_case(b"VOICE_VOLUME") || command.eq_ignore_ascii_case(b"SOUND_VOLUME") {
                config.sound_grade = parse_sound_grade(value);
            }
        }
        config
    }
}

/// `MUSIC_VOLUME`: a linear volume, or (without a `.`) an old grade 0..=5 or
/// 0..=10.
fn parse_music_volume(value: &[u8]) -> f32 {
    if value.contains(&b'.') {
        return atof_f32(value);
    }
    let mut grade = atof_f32(value);
    if grade > 5.0 {
        grade *= 0.5;
    }
    if grade <= 0.0 { 0.0 } else { 10f32.powf(-1.0 + grade / 5.0) }
}

/// `VOICE_VOLUME`: a grade 0..=5 (0..=10 is halved), or a linear volume
/// with a `.`, turned into the nearest grade (at least 1 when not silent).
fn parse_sound_grade(value: &[u8]) -> i32 {
    if !value.contains(&b'.') {
        let grade = atoi(value);
        return if grade > 5 { (grade + 1) / 2 } else { grade };
    }
    let volume = atof_f32(value);
    // `static_cast<char>`: the grade is stored in a `char`.
    let grade = i32::from((volume * 5.0 + 0.5) as i8);
    if grade == 0 && volume > 0.0 { 1 } else { grade }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grades_follow_the_log_curve() {
        assert_eq!(grade_to_volume(0), 0.0);
        assert!((grade_to_volume(5) - 1.0).abs() < 1e-6);
        assert!((grade_to_volume(3) - 10f32.powf(-0.4)).abs() < 1e-6);
        // The jump at 0.1.
        assert_eq!(ratio_to_volume(0.09), 0.09);
        assert!((ratio_to_volume(0.1) - 10f32.powf(-0.9)).abs() < 1e-6);
    }

    #[test]
    fn save_and_restore_round_trip() {
        let mut v = Volumes::default();
        v.set_sound(0.5);
        assert_eq!(v.set_music(2.0), Some(1.0));
        assert!(v.save());
        assert!(!v.save());
        assert_eq!((v.sound(), v.music()), (0.0, 0.0));
        // Set while disabled: only the backup changes.
        v.set_sound(0.25);
        assert_eq!(v.set_music(0.75), None);
        assert_eq!(v.sound(), 0.0);
        assert_eq!(v.restore(), 0.75);
        assert_eq!((v.sound(), v.music()), (0.25, 0.75));
    }

    #[test]
    fn config_reads_both_keys_and_stops_at_a_blank_line() {
        let cfg = b"WIDTH 1024\nMUSIC_VOLUME\t\t1.000\nVOICE_VOLUME 5\n";
        assert_eq!(AudioConfig::parse(cfg), AudioConfig { music_volume: 1.0, sound_grade: 5 });
        let cfg = b"music_volume 3\nsound_volume 0.5\n";
        let c = AudioConfig::parse(cfg);
        assert!((c.music_volume - 10f32.powf(-0.4)).abs() < 1e-6);
        assert_eq!(c.sound_grade, 3);
        assert_eq!(AudioConfig::parse(b"VOICE_VOLUME 10").sound_grade, 5);
        assert_eq!(AudioConfig::parse(b"VOICE_VOLUME 0.01").sound_grade, 1);
        assert_eq!(AudioConfig::parse(b"MUSIC_VOLUME 0").music_volume, 0.0);
        assert_eq!(AudioConfig::parse(b"\nVOICE_VOLUME 1").sound_grade, 5);
    }
}
