//! The sound script of an effect: `NSound::LoadSoundInformationPiece` and
//! `NSound::DataToInstance` (`neoeter/src/audio/Type.cpp`), which
//! `CEffectData::LoadScript` calls.

use super::{Error, float_to_dword};
use crate::pack::PackManager;
use crate::script::{self, atof_f32};

/// `NSound::TSoundInstance`: a sound to play at a frame (60 per second).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SoundInstance {
    pub frame: u32,
    pub file_name: Vec<u8>,
}

/// `LoadSoundInformationPiece` (without a path header) followed by
/// `DataToInstance`: `SoundDataCount n` and `SoundDataNN time "file"`.
///
/// `Ok(None)` where `LoadSoundInformationPiece` fails: a missing file, a
/// missing count, a missing entry, or an entry without exactly two values.
/// Unbalanced braces do not matter (`CTextFileLoader::Cache` keeps the
/// tree). A negative count makes C++ `resize` throw `std::length_error`.
pub fn load_sound_information(manager: &PackManager, name: &[u8]) -> Result<Option<Vec<SoundInstance>>, Error> {
    let Ok(script) = script::load(manager, name) else {
        return Ok(None);
    };
    let root = &script.root;
    let Some(count) = root.token_int("sounddatacount") else {
        return Ok(None);
    };
    let count = u32::try_from(count).map_err(|_| Error::Throws)?;

    let mut data = Vec::new();
    for i in 0..count {
        let Some([time, file]) = root.tokens(format!("sounddata{i:02}")) else {
            return Ok(None);
        };
        data.push((atof_f32(time), file.clone()));
    }

    // DataToInstance: the frame at 60 fps, truncated.
    let frame_time = 1.0f32 / 60.0;
    Ok(Some(
        data.into_iter()
            .map(|(time, file_name)| SoundInstance { frame: float_to_dword(time / frame_time), file_name })
            .collect(),
    ))
}
