//! `CEffectData` (`EffectData.cpp`): an `.mse` effect script.

use super::{
    Cursor, EffectMeshScript, Error, LightData, ParticleSystemData, SoundInstance, load_sound_information, string_path,
};
use crate::pack::PackManager;
use crate::script::{self, Script};

/// `CEffectData` after `LoadScript`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EffectData {
    /// `m_strFileName`: the requested name through `StringPath`.
    pub file_name: Vec<u8>,
    pub bounding_sphere_radius: f32,
    pub bounding_sphere_position: [f32; 3],
    /// `Group Particle` elements, in file order.
    pub particles: Vec<ParticleSystemData>,
    /// `Group Mesh` elements, in file order.
    pub meshes: Vec<EffectMeshScript>,
    /// `Group Light` elements, in file order.
    pub lights: Vec<LightData>,
    /// From `sound/<path under d:/ymir work/>.mss`, if that loads.
    pub sounds: Vec<SoundInstance>,
}

/// `CEffectData::LoadScript` on a parsed script. `name` is the name the
/// script was loaded by (relative texture and mesh names resolve against
/// its directory); `manager` reads the sound script.
pub fn parse_effect(manager: &PackManager, name: &[u8], script: &Script) -> Result<EffectData, Error> {
    if !script.complete {
        return Err(Error::Unbalanced);
    }
    let mut data = EffectData { file_name: string_path(name), ..EffectData::default() };

    let mut cursor = Cursor::new(&script.root);
    let root = cursor.node();
    data.bounding_sphere_radius = root.token_float("boundingsphereradius").unwrap_or(0.0);
    data.bounding_sphere_position = root.token_vector3("boundingsphereposition").unwrap_or([0.0; 3]);

    // The count is re-read from wherever the cursor is, as in C++: a broken
    // element can leave it one level up.
    let mut i = 0;
    while i < cursor.child_count() {
        if cursor.enter_index(i) && let Some(kind) = cursor.name() {
                match kind {
                    b"mesh" => {
                        let mut mesh = EffectMeshScript::default();
                        mesh.load(&mut cursor, name)?;
                        data.meshes.push(mesh);
                    }
                    b"particle" => {
                        let mut particle = ParticleSystemData::default();
                        particle.load(&mut cursor, name)?;
                        data.particles.push(particle);
                    }
                    b"light" => {
                        let mut light = LightData::default();
                        light.load(&mut cursor)?;
                        data.lights.push(light);
                    }
                    _ => {}
                }
                cursor.leave();
        }
        i += 1;
    }

    // C++ finds the header anywhere but always cuts its length off the
    // front of the name.
    const HEADER: &[u8] = b"d:/ymir work/";
    let stem = match data.file_name.iter().rposition(|&b| b == b'.') {
        Some(dot) => &data.file_name[..dot],
        None => &data.file_name[..],
    };
    if stem.windows(HEADER.len()).any(|w| w == HEADER) && stem.len() > HEADER.len() {
        let sound = [b"sound/", &stem[HEADER.len()..], b".mss"].concat();
        if let Some(sounds) = load_sound_information(manager, &sound)? {
            data.sounds = sounds;
        }
    }
    Ok(data)
}

/// `CEffectData::LoadScript`: reads and parses `name` through the pack
/// manager.
pub fn load_effect(manager: &PackManager, name: impl AsRef<[u8]>) -> Result<EffectData, Error> {
    let name = name.as_ref();
    let script = script::load(manager, name)?;
    parse_effect(manager, name, &script)
}
