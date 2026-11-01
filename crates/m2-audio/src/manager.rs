//! `CSoundManager` (`SoundManager.cpp`) on `bevy_audio`.
//!
//! Every sound is an entity with an [`AudioPlayer<M2Sound>`]; Bevy creates its
//! sink once the asset is in and despawns it when it ends. Where C++ keeps
//! fixed instance arrays, the port counts entities with the same limits
//! (`CSoundManager2D::INSTANCE_MAX_COUNT` = 4, `CSoundManager3D` = 32): a
//! sound asked for while every instance is busy is not played. Where C++
//! returns a 3D instance index, the port returns the sound's [`Entity`].
//!
//! # 3D sounds
//!
//! C++ places the emitter at `(position - listener) / scale` (`m_fSoundScale`
//! 200 for characters, `m_fAmbienceSoundScale` 1000 for ambience) around a
//! listener at the origin turned like the camera. Here the emitter sits at its
//! world position, the [`SoundListener`] entity at the camera's centre, and
//! the scale is the sound's `SpatialScale`, which gives the same geometry.
//!
//! The Miles shim (`MSSCompat.h`) attenuates with miniaudio's inverse model:
//! `1 / clamp(d, 1, 100)` in audio units. Bevy's spatial sink is `1 / d²`
//! (clamped to 1) per ear, so [`falloff_compensation`] rescales the sink
//! volume every frame to the inverse curve.

use std::collections::HashMap;

use bevy::audio::{AudioPlayer, AudioSink, AudioSinkPlayback, PlaybackMode, PlaybackSettings, SpatialAudioSink, SpatialScale, Volume};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use m2_assets::paths;

use crate::music::{DEFAULT_FADE_SPEED, MUSIC_SLOTS, MusicAction, MusicSlots};
use crate::source::M2Sound;
use crate::volume::{Volumes, grade_to_volume, ratio_to_volume};

/// `CSoundManager2D::INSTANCE_MAX_COUNT`.
pub const MAX_2D_SOUNDS: usize = 4;
/// `CSoundManager3D::INSTANCE_MAX_COUNT`.
pub const MAX_3D_SOUNDS: usize = 32;
/// `m_fSoundScale`: world units per audio unit for character and effect sounds.
pub const DEFAULT_SOUND_SCALE: f32 = 200.0;
/// `m_fAmbienceSoundScale`.
pub const DEFAULT_AMBIENCE_SCALE: f32 = 1000.0;
/// `MilesCompatInternal::kMin3DDistance` / `kMax3DDistance`, audio units.
pub const MIN_3D_DISTANCE: f32 = 1.0;
pub const MAX_3D_DISTANCE: f32 = 100.0;
/// `PlayCharacterSound3D` with `bCheckFrequency`: sounds further than this
/// (horizontally, world units) are skipped...
pub const CHARACTER_SOUND_RANGE: f32 = 5000.0;
/// ...and so is the same file again within this many seconds.
pub const CHARACTER_SOUND_REPEAT: f32 = 0.3;
/// Distance between the listener's ears, world units. Only the direction
/// matters for the panning; small keeps both ears at the listener.
pub const EAR_GAP: f32 = 4.0;
/// A sound that has not started this long after it was asked for (no audio
/// device) is dropped so it does not hold an instance forever.
const START_TIMEOUT: f32 = 10.0;

/// The ears 3D sounds are heard with: `CSoundManager::SetPosition` (the
/// camera's centre) and `SetDirection` (the camera's rotation). The plugin
/// spawns one; the game moves it.
#[derive(Component, Debug, Default, Clone, Copy)]
pub struct SoundListener;

/// A sound from `PlaySound2D`.
#[derive(Component, Debug)]
pub struct Sound2d;

/// A sound from one of the `Play*Sound3D` calls.
#[derive(Component, Debug)]
pub struct Sound3d {
    /// World units per audio unit.
    pub scale: f32,
}

/// The instance volume (`ISoundInstance::SetVolume`), before 3D attenuation.
#[derive(Component, Debug, Clone, Copy)]
pub struct SoundVolume(pub f32);

/// The looping sound of a music slot.
#[derive(Component, Debug)]
pub struct Music {
    pub slot: usize,
}

/// A sound played more than once (`Play(iLoopCount > 1)`): replayed while
/// `remaining` is positive.
#[derive(Component, Debug)]
pub(crate) struct Replays {
    sound: Handle<M2Sound>,
    settings: PlaybackSettings,
    remaining: u32,
}

/// When the sound was (re)queued, to drop sounds that never start.
#[derive(Component, Debug)]
pub(crate) struct Queued(f32);

/// The sound manager's state (`CSoundManager`'s members).
#[derive(Resource, Debug)]
pub struct SoundState {
    pub volumes: Volumes,
    pub music: MusicSlots,
    pub sound_scale: f32,
    pub ambience_scale: f32,
    music_entities: [Option<Entity>; MUSIC_SLOTS],
    /// `m_PlaySoundHistoryMap`: canonical file name, play time.
    history: HashMap<String, f32>,
    /// Sounds spawned this frame, not yet seen by the instance queries.
    spawned_2d: usize,
    spawned_3d: usize,
}

impl Default for SoundState {
    fn default() -> Self {
        SoundState {
            volumes: Volumes::default(),
            music: MusicSlots::default(),
            sound_scale: DEFAULT_SOUND_SCALE,
            ambience_scale: DEFAULT_AMBIENCE_SCALE,
            music_entities: [None; MUSIC_SLOTS],
            history: HashMap::new(),
            spawned_2d: 0,
            spawned_3d: 0,
        }
    }
}

/// The miniaudio inverse model over Bevy's inverse-square spatial sink: the
/// factor that turns `min(1, 1/d²)` into `1 / clamp(d, 1, 100)` (`d` in audio
/// units).
pub fn falloff_compensation(distance: f32) -> f32 {
    let wanted = 1.0 / distance.clamp(MIN_3D_DISTANCE, MAX_3D_DISTANCE);
    let bevy = (1.0 / (distance * distance)).min(1.0);
    wanted / bevy
}

/// `PlaybackSettings` for `Play(count)`: 0 loops, 1 plays once, more replays.
fn playback(count: u32, volume: f32) -> PlaybackSettings {
    let base = match count {
        0 => PlaybackSettings::LOOP,
        1 => PlaybackSettings::DESPAWN,
        _ => PlaybackSettings::REMOVE,
    };
    base.with_volume(Volume::Linear(volume))
}

/// The client's sound calls (`CSoundManager::Instance()`), for systems.
///
/// Positions are Bevy world positions (see `m2_assets::coords`); file names
/// are client paths (`sound/ui/click.wav`, `BGM/login_window.mp3`).
#[derive(SystemParam)]
pub struct SoundManager<'w, 's> {
    commands: Commands<'w, 's>,
    state: ResMut<'w, SoundState>,
    server: Res<'w, AssetServer>,
    time: Res<'w, Time>,
    sounds_2d: Query<'w, 's, (), With<Sound2d>>,
    sounds_3d: Query<'w, 's, Entity, With<Sound3d>>,
    listener: Query<'w, 's, &'static GlobalTransform, With<SoundListener>>,
}

impl SoundManager<'_, '_> {
    fn load(&self, file: &str) -> Handle<M2Sound> {
        self.server.load(paths::asset_source_uri(file))
    }

    fn listener_position(&self) -> Vec3 {
        self.listener.iter().next().map_or(Vec3::ZERO, |t| t.translation())
    }

    fn now(&self) -> f32 {
        self.time.elapsed_secs()
    }

    /// The state: volumes, music slots and scales.
    pub fn state(&self) -> &SoundState {
        &self.state
    }

    /// `PlaySound2D`: once, at the sound volume. Nothing while the volume is 0
    /// or four 2D sounds are playing.
    pub fn play_2d(&mut self, file: &str) {
        let volume = self.state.volumes.sound();
        if volume == 0.0 || self.sounds_2d.iter().len() + self.state.spawned_2d >= MAX_2D_SOUNDS {
            return;
        }
        self.state.spawned_2d += 1;
        let sound = self.load(file);
        let now = self.now();
        self.commands.spawn((
            Name::new(format!("sound {file}")),
            Sound2d,
            SoundVolume(volume),
            AudioPlayer(sound),
            playback(1, volume),
            Queued(now),
        ));
    }

    fn spawn_3d(&mut self, position: Vec3, file: &str, count: u32, scale: f32) -> Option<Entity> {
        let volume = self.state.volumes.sound();
        if volume == 0.0 || self.sounds_3d.iter().len() + self.state.spawned_3d >= MAX_3D_SOUNDS {
            return None;
        }
        self.state.spawned_3d += 1;
        let distance = position.distance(self.listener_position()) / scale;
        let mut settings = playback(count, volume * falloff_compensation(distance));
        settings.spatial = true;
        settings.spatial_scale = Some(SpatialScale::new(1.0 / scale));
        let sound = self.load(file);
        let now = self.now();
        let mut entity = self.commands.spawn((
            Name::new(format!("sound {file}")),
            Sound3d { scale },
            SoundVolume(volume),
            Transform::from_translation(position),
            AudioPlayer(sound.clone()),
            settings,
            Queued(now),
        ));
        if count > 1 {
            entity.insert(Replays { sound, settings, remaining: count - 1 });
        }
        Some(entity.id())
    }

    /// `PlaySound3D`: `count` plays (0 loops) at the sound scale.
    pub fn play_3d(&mut self, position: Vec3, file: &str, count: u32) -> Option<Entity> {
        let scale = self.state.sound_scale;
        self.spawn_3d(position, file, count, scale)
    }

    /// `PlayAmbienceSound3D`: like [`play_3d`](Self::play_3d) at the ambience
    /// scale; keep the entity to move ([`update_ambience_3d`](Self::update_ambience_3d)),
    /// fade ([`set_volume_3d`](Self::set_volume_3d)) or stop it.
    pub fn play_ambience_3d(&mut self, position: Vec3, file: &str, count: u32) -> Option<Entity> {
        let scale = self.state.ambience_scale;
        self.spawn_3d(position, file, count, scale)
    }

    /// `PlayCharacterSound3D`: once at the sound scale. With
    /// `check_frequency`, sounds more than [`CHARACTER_SOUND_RANGE`] away
    /// (horizontally) and repeats of a file within
    /// [`CHARACTER_SOUND_REPEAT`] seconds are skipped.
    pub fn play_character_3d(&mut self, position: Vec3, file: &str, check_frequency: bool) {
        if self.state.volumes.sound() == 0.0 {
            return;
        }
        if check_frequency {
            let d = (position - self.listener_position()).xz();
            if d.length_squared() > CHARACTER_SOUND_RANGE * CHARACTER_SOUND_RANGE {
                return;
            }
            let now = self.now();
            let key = paths::canonical_asset_path(file);
            if self.state.history.get(&key).is_some_and(|&t| now - t < CHARACTER_SOUND_REPEAT) {
                return;
            }
            self.state.history.insert(key, now);
        }
        self.play_3d(position, file, 1);
    }

    /// `UpdateSoundInstance` (3D): plays the motion or effect sounds scheduled
    /// for `frame` (60 per second) at `position`.
    pub fn play_sounds_at_frame_3d<'a>(
        &mut self,
        position: Vec3,
        frame: u32,
        sounds: impl IntoIterator<Item = (u32, &'a str)>,
        check_frequency: bool,
    ) {
        for (_, file) in sounds.into_iter().filter(|&(f, _)| f == frame) {
            self.play_character_3d(position, file, check_frequency);
        }
    }

    /// `UpdateSoundInstance` (2D).
    pub fn play_sounds_at_frame_2d<'a>(&mut self, frame: u32, sounds: impl IntoIterator<Item = (u32, &'a str)>) {
        for (_, file) in sounds.into_iter().filter(|&(f, _)| f == frame) {
            self.play_2d(file);
        }
    }

    /// `StopSound3D`.
    pub fn stop_3d(&mut self, sound: Entity) {
        if let Ok(mut entity) = self.commands.get_entity(sound) {
            entity.try_despawn();
        }
    }

    /// `StopAllSound3D`.
    pub fn stop_all_3d(&mut self) {
        for sound in self.sounds_3d.iter() {
            self.commands.entity(sound).try_despawn();
        }
    }

    /// `SetSoundVolume3D`: the instance volume (clamped to `0..=1`), which
    /// replaces the sound volume the sound started with.
    pub fn set_volume_3d(&mut self, sound: Entity, volume: f32) {
        if let Ok(mut entity) = self.commands.get_entity(sound) {
            entity.try_insert(SoundVolume(volume.clamp(0.0, 1.0)));
        }
    }

    /// `UpdateAmbienceSound3D`: moves the emitter.
    pub fn update_ambience_3d(&mut self, sound: Entity, position: Vec3) {
        if let Ok(mut entity) = self.commands.get_entity(sound) {
            entity.try_insert(Transform::from_translation(position));
        }
    }

    /// `SetSoundScale`.
    pub fn set_sound_scale(&mut self, scale: f32) {
        self.state.sound_scale = scale;
    }

    /// `SetAmbienceSoundScale`.
    pub fn set_ambience_scale(&mut self, scale: f32) {
        self.state.ambience_scale = scale;
    }

    /// `SetSoundVolume`. Playing sounds keep their volume.
    pub fn set_sound_volume(&mut self, volume: f32) {
        self.state.volumes.set_sound(volume);
    }

    /// `SetSoundVolumeGrade` (0..=5).
    pub fn set_sound_volume_grade(&mut self, grade: i32) {
        self.set_sound_volume(grade_to_volume(grade));
    }

    /// `SetSoundVolumeRatio`.
    pub fn set_sound_volume_ratio(&mut self, ratio: f32) {
        self.set_sound_volume(ratio_to_volume(ratio));
    }

    /// `SetMusicVolume`: also the playing music that is not fading out.
    pub fn set_music_volume(&mut self, volume: f32) {
        if let Some(volume) = self.state.volumes.set_music(volume) {
            self.state.music.set_volume(volume);
        }
    }

    /// `SaveVolume`: silence until [`restore_volume`](Self::restore_volume).
    pub fn save_volume(&mut self) {
        if self.state.volumes.save() {
            self.state.music.set_volume(0.0);
        }
    }

    /// `RestoreVolume`.
    pub fn restore_volume(&mut self) {
        let music = self.state.volumes.restore();
        self.state.music.set_volume(music);
    }

    /// `PlayMusic(file)`: slot 0 at the music volume, looping.
    pub fn play_music(&mut self, file: &str) {
        let volume = self.state.volumes.music();
        let action = self.state.music.play(0, file, volume, 0.0);
        self.apply(action);
    }

    /// `FadeInMusic` with the default speed.
    pub fn fade_in_music(&mut self, file: &str) {
        self.fade_in_music_with(file, DEFAULT_FADE_SPEED);
    }

    /// `FadeInMusic`: `speed` is the volume step per 60 FPS frame.
    pub fn fade_in_music_with(&mut self, file: &str, speed: f32) {
        let action = self.state.music.fade_in(file, speed);
        self.apply(action);
    }

    /// `FadeOutMusic` with the default speed.
    pub fn fade_out_music(&mut self, file: &str) {
        if !self.state.music.fade_out(file, DEFAULT_FADE_SPEED) {
            warn!("FadeOutMusic: {file} - ERROR NOT EXIST");
        }
    }

    /// `FadeLimitOutMusic`: fades `file` down to `ratio_to_volume(limit_ratio)`.
    pub fn fade_limit_out_music(&mut self, file: &str, limit_ratio: f32) {
        if !self.state.music.fade_limit_out(file, limit_ratio, DEFAULT_FADE_SPEED) {
            warn!("FadeOutMusic: {file} - ERROR NOT EXIST");
        }
    }

    /// `FadeOutAllMusic`.
    pub fn fade_out_all_music(&mut self) {
        self.state.music.fade_out_all();
    }

    fn apply(&mut self, action: Option<MusicAction>) {
        let Some(action) = action else { return };
        apply_music_action(&mut self.commands, &mut self.state, &self.server, self.time.elapsed_secs(), action);
    }
}

fn apply_music_action(commands: &mut Commands, state: &mut SoundState, server: &AssetServer, now: f32, action: MusicAction) {
    let slot = match &action {
        MusicAction::Start { slot, .. } | MusicAction::Stop { slot } => *slot,
    };
    if let Some(old) = state.music_entities[slot].take() {
        commands.entity(old).try_despawn();
    }
    if let MusicAction::Start { slot, file } = action {
        let volume = state.music.slots[slot].volume;
        let entity = commands
            .spawn((
                Name::new(format!("music {file}")),
                Music { slot },
                AudioPlayer::<M2Sound>(server.load(paths::asset_source_uri(&file))),
                playback(0, volume),
                Queued(now),
            ))
            .id();
        state.music_entities[slot] = Some(entity);
    }
}

pub(crate) fn reset_frame_counts(mut state: ResMut<SoundState>) {
    state.spawned_2d = 0;
    state.spawned_3d = 0;
}

/// The music fades of `CSoundManager::Update`, then each slot's volume onto
/// its sink.
pub(crate) fn update_music(
    mut commands: Commands,
    mut state: ResMut<SoundState>,
    server: Res<AssetServer>,
    time: Res<Time>,
    mut sinks: Query<(&Music, &mut AudioSink)>,
) {
    let music_volume = state.volumes.music();
    for action in state.music.update(time.delta_secs() * 60.0, music_volume) {
        apply_music_action(&mut commands, &mut state, &server, time.elapsed_secs(), action);
    }
    for (music, mut sink) in &mut sinks {
        let volume = state.music.slots[music.slot].volume.clamp(0.0, 1.0);
        if sink.volume() != Volume::Linear(volume) {
            sink.set_volume(Volume::Linear(volume));
        }
    }
}

/// The instance volume times [`falloff_compensation`] onto each 3D sink.
pub(crate) fn apply_falloff(
    listener: Query<&GlobalTransform, With<SoundListener>>,
    mut sounds: Query<(&GlobalTransform, &Sound3d, &SoundVolume, &mut SpatialAudioSink)>,
) {
    let ears = listener.iter().next().map_or(Vec3::ZERO, |t| t.translation());
    for (transform, sound, volume, mut sink) in &mut sounds {
        let distance = transform.translation().distance(ears) / sound.scale;
        sink.set_volume(Volume::Linear(volume.0 * falloff_compensation(distance)));
    }
}

/// Plays a [`Replays`] sound again once Bevy removed its finished player.
pub(crate) fn replay_finished(
    mut commands: Commands,
    time: Res<Time>,
    mut sounds: Query<(Entity, &mut Replays), Without<AudioPlayer<M2Sound>>>,
) {
    for (entity, mut replays) in &mut sounds {
        if replays.remaining == 0 {
            commands.entity(entity).despawn();
            continue;
        }
        replays.remaining -= 1;
        let mut settings = replays.settings;
        if replays.remaining == 0 {
            settings.mode = PlaybackMode::Despawn;
        }
        commands.entity(entity).insert((AudioPlayer(replays.sound.clone()), settings, Queued(time.elapsed_secs())));
    }
}

/// Drops sounds whose file failed to load (`SetSound` failing in C++),
/// freeing their instance or music slot, and effect sounds that never start
/// (no audio device). Music waits for a device: there are only three slots
/// and their fades should keep working.
pub(crate) fn drop_unplayable(
    mut commands: Commands,
    mut state: ResMut<SoundState>,
    server: Res<AssetServer>,
    time: Res<Time>,
    sounds: Query<(Entity, &AudioPlayer<M2Sound>, &Queued, Option<&Music>), (Without<AudioSink>, Without<SpatialAudioSink>)>,
) {
    let now = time.elapsed_secs();
    for (entity, player, queued, music) in &sounds {
        let failed = server.load_state(player.0.id()).is_failed();
        if !failed && (music.is_some() || now - queued.0 < START_TIMEOUT) {
            continue;
        }
        commands.entity(entity).despawn();
        if let Some(music) = music
            && state.music_entities[music.slot] == Some(entity)
        {
            state.music_entities[music.slot] = None;
            state.music.stop(music.slot);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falloff_matches_the_inverse_model() {
        for d in [0.0f32, 0.5, 1.0, 2.0, 10.0, 99.0, 150.0] {
            let bevy = (1.0 / (d * d)).min(1.0);
            let miniaudio = 1.0 / d.clamp(1.0, 100.0);
            assert!((bevy * falloff_compensation(d) - miniaudio).abs() < 1e-6, "{d}");
        }
    }

    #[test]
    fn play_counts_map_to_playback_modes() {
        assert!(matches!(playback(0, 1.0).mode, PlaybackMode::Loop));
        assert!(matches!(playback(1, 1.0).mode, PlaybackMode::Despawn));
        assert!(matches!(playback(3, 1.0).mode, PlaybackMode::Remove));
        assert_eq!(playback(1, 0.5).volume, Volume::Linear(0.5));
    }
}
