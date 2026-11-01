//! The sound asset: `CSoundData` (`SoundData.cpp`) without the Miles file
//! callbacks, since the `m2://` asset source already reads packs and loose
//! files.
//!
//! The client ships PCM `.wav` effects and `.mp3` music (plus a few `.mp3`
//! effects such as `sound/common/etc/salute.mp3`). The bytes are kept as they
//! are and decoded while playing, like Bevy's own `AudioSource`; the loader
//! decodes the header once so that a damaged file fails to load instead of
//! panicking inside `bevy_audio`'s playback system.

use std::io::Cursor;
use std::sync::Arc;

use bevy::asset::{Asset, AssetLoader, LoadContext, io::Reader};
use bevy::audio::Decodable;
use bevy::reflect::TypePath;

/// A loaded `.wav` or `.mp3`.
#[derive(Asset, TypePath, Debug, Clone)]
pub struct M2Sound {
    bytes: Arc<[u8]>,
    /// The extension, as the decoder's format hint.
    hint: &'static str,
}

impl AsRef<[u8]> for M2Sound {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}

impl M2Sound {
    /// Wraps `bytes` if a decoder accepts them. `hint` is the extension.
    pub fn new(bytes: impl Into<Arc<[u8]>>, hint: &'static str) -> Result<Self, rodio::decoder::DecoderError> {
        let sound = M2Sound { bytes: bytes.into(), hint };
        sound.try_decoder()?;
        Ok(sound)
    }

    fn try_decoder(&self) -> Result<rodio::Decoder<Cursor<M2Sound>>, rodio::decoder::DecoderError> {
        rodio::Decoder::builder()
            .with_byte_len(self.bytes.len() as u64)
            .with_hint(self.hint)
            .with_data(Cursor::new(self.clone()))
            .build()
    }
}

impl Decodable for M2Sound {
    type Decoder = rodio::Decoder<Cursor<M2Sound>>;

    fn decoder(&self) -> Self::Decoder {
        // `M2Sound::new` decoded the same bytes already.
        self.try_decoder().expect("M2Sound was validated on load")
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SoundLoadError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("{path}: not a playable sound: {error}")]
    Decode { path: String, error: rodio::decoder::DecoderError },
}

/// Loads `.wav` and `.mp3` into [`M2Sound`].
#[derive(Default, TypePath)]
pub struct M2SoundLoader;

impl AssetLoader for M2SoundLoader {
    type Asset = M2Sound;
    type Settings = ();
    type Error = SoundLoadError;

    async fn load(&self, reader: &mut dyn Reader, _: &(), load_context: &mut LoadContext<'_>) -> Result<M2Sound, SoundLoadError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let path = load_context.path().path();
        let hint = match path.extension().and_then(|e| e.to_str()) {
            Some(e) if e.eq_ignore_ascii_case("mp3") => "mp3",
            _ => "wav",
        };
        M2Sound::new(bytes, hint).map_err(|error| SoundLoadError::Decode { path: path.display().to_string(), error })
    }

    fn extensions(&self) -> &[&str] {
        &["wav", "mp3"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 16-bit mono PCM `.wav` of `samples` zero samples.
    fn wav(samples: u32) -> Vec<u8> {
        let data = samples * 2;
        let mut w = Vec::new();
        w.extend_from_slice(b"RIFF");
        w.extend_from_slice(&(36 + data).to_le_bytes());
        w.extend_from_slice(b"WAVEfmt ");
        w.extend_from_slice(&16u32.to_le_bytes());
        w.extend_from_slice(&1u16.to_le_bytes()); // PCM
        w.extend_from_slice(&1u16.to_le_bytes()); // mono
        w.extend_from_slice(&22050u32.to_le_bytes());
        w.extend_from_slice(&44100u32.to_le_bytes());
        w.extend_from_slice(&2u16.to_le_bytes());
        w.extend_from_slice(&16u16.to_le_bytes());
        w.extend_from_slice(b"data");
        w.extend_from_slice(&data.to_le_bytes());
        w.resize(w.len() + data as usize, 0);
        w
    }

    #[test]
    fn pcm_wav_decodes_and_garbage_does_not() {
        let sound = M2Sound::new(wav(2205), "wav").unwrap();
        let decoder = sound.decoder();
        assert_eq!(rodio::Source::channels(&decoder).get(), 1);
        assert_eq!(decoder.count(), 2205);
        assert!(M2Sound::new(b"not a sound".to_vec(), "wav").is_err());
    }
}
