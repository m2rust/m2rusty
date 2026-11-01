//! `.mss` sound scripts: `NSound::LoadSoundInformationPiece` and
//! `NSound::DataToInstance` (`neoeter/src/audio/Type.cpp`).
//!
//! ```text
//! ScriptType        CharacterSoundInformation
//! SoundDataCount    2
//! SoundData00       0.100000 "sound/pc/attack.wav"
//! SoundData01       0.500000 "sound/pc/step.wav"
//! ```

use super::float_to_dword;
use crate::pack::PackManager;
use crate::script::{self, Script, atof_f32};

/// `NSound::SSoundData`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SoundData {
    /// `fTime`, in seconds.
    pub time: f32,
    /// `strSoundFileName`.
    pub file_name: Vec<u8>,
}

/// `NSound::SSoundInstance`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SoundInstance {
    /// `dwFrame`: the time in 60 FPS frames, truncated.
    pub frame: u32,
    /// `strSoundFileName`.
    pub file_name: Vec<u8>,
}

/// `NSound::LoadSoundInformationPiece`: reads `name` (through
/// `CTextFileLoader::Cache`, which uses the tree even when its braces do not
/// balance) and parses it with [`parse_sound_information`]. `None` for a
/// missing file.
pub fn load_sound_information_piece(
    manager: &PackManager,
    name: impl AsRef<[u8]>,
    path_header: Option<&[u8]>,
) -> Option<Vec<SoundData>> {
    parse_sound_information(&script::load(manager, name).ok()?, path_header)
}

/// The body of `NSound::LoadSoundInformationPiece`: `SoundDataCount`
/// entries `SoundDataNN <time> <file>`, each exactly two tokens, the file
/// name prefixed with `path_header`. `None` where C++ returns `false`.
///
/// Deviation: a negative `SoundDataCount` makes C++ `resize` throw (the
/// process aborts); here it is a failure.
pub fn parse_sound_information(script: &Script, path_header: Option<&[u8]>) -> Option<Vec<SoundData>> {
    let root = &script.root;
    let count = usize::try_from(root.token_int("sounddatacount")?).ok()?;

    let mut out = Vec::with_capacity(count.min(1024));
    for i in 0..count {
        // `_snprintf` into a 33-byte buffer.
        let mut key = format!("sounddata{i:02}");
        key.truncate(32);
        let tokens = root.tokens(&key)?;
        let [time, file] = tokens else {
            return None;
        };
        let mut file_name = path_header.unwrap_or_default().to_vec();
        file_name.extend_from_slice(file);
        out.push(SoundData { time: atof_f32(time), file_name });
    }
    Some(out)
}

/// `NSound::DataToInstance`. An empty `data` leaves `out` as it is;
/// otherwise `out` is replaced.
pub fn data_to_instance(data: &[SoundData], out: &mut Vec<SoundInstance>) {
    if data.is_empty() {
        return;
    }
    let frame_time = 1.0f32 / 60.0;
    *out = data
        .iter()
        .map(|d| SoundInstance {
            frame: float_to_dword(d.time / frame_time),
            file_name: d.file_name.clone(),
        })
        .collect();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script::parse;

    #[test]
    fn parses_entries() {
        let script = parse(b"SoundDataCount 2\nSoundData00 0.5 \"a.wav\"\nSoundData01 1 b.wav\n");
        let data = parse_sound_information(&script, Some(b"p/")).unwrap();
        assert_eq!(data[0], SoundData { time: 0.5, file_name: b"p/a.wav".to_vec() });
        let mut out = vec![SoundInstance::default(); 5];
        data_to_instance(&data, &mut out);
        assert_eq!(out.len(), 2);
        // 1/60 rounds up as a float, so whole frames land just below.
        assert_eq!(out[0].frame, 29);
        assert_eq!(out[1].frame, 59);

        let mut kept = vec![SoundInstance::default()];
        data_to_instance(&[], &mut kept);
        assert_eq!(kept.len(), 1);
    }

    #[test]
    fn failures() {
        assert!(parse_sound_information(&parse(b"x 1\n"), None).is_none());
        assert!(parse_sound_information(&parse(b"SoundDataCount 2\nSoundData00 0 a\n"), None).is_none());
        assert!(parse_sound_information(&parse(b"SoundDataCount 1\nSoundData00 0 a b\n"), None).is_none());
        assert!(parse_sound_information(&parse(b"SoundDataCount -1\n"), None).is_none());
        assert_eq!(parse_sound_information(&parse(b"SoundDataCount 0\n"), None), Some(vec![]));
    }
}
