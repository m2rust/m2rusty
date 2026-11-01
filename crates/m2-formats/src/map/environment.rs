//! `Environment_Init`/`Environment_Load` (`client/src/game/MapUtil.cpp`): the
//! `.msenv` environment of an outdoor map, under
//! `d:/ymir work/environment/`.
//!
//! The client builds one `TEnvironmentData` per map
//! (`CMapManager::AllocEnvironmentData` calls `Environment_Init`, then
//! `Environment_Load` fills it), and `CMapManager::SetEnvironmentData`
//! applies the fog, the directional lights and the sky. [`EnvironmentData`]
//! is that struct after the two calls; the values C++ never overwrites keep
//! the `Environment_Init` defaults.
//!
//! As in C++, a missing token leaves the field at its previous value, and a
//! vector token whose length is not exactly 2, 3 or 4 leaves it untouched.

use crate::pack::{self, PackManager};
use crate::script::{self, Node};

/// `ENV_DIRLIGHT_NUM`.
pub const ENV_DIRLIGHT_NUM: usize = 2;
/// `ENV_DIRLIGHT_BACKGROUND`.
pub const ENV_DIRLIGHT_BACKGROUND: usize = 0;
/// `ENV_DIRLIGHT_CHARACTER`.
pub const ENV_DIRLIGHT_CHARACTER: usize = 1;

/// `D3DLIGHT9` as `Environment_Load` uses it: a direction, a diffuse and an
/// ambient colour. The other D3D fields are not read from `.msenv`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DirectionalLight {
    pub enable: bool,
    pub direction: [f32; 3],
    pub diffuse: [f32; 4],
    pub ambient: [f32; 4],
}

impl Default for DirectionalLight {
    /// `Environment_Init` for one entry of `DirLights`.
    fn default() -> Self {
        Self { enable: false, direction: [0.5, 0.5, -0.5], diffuse: [1.0; 4], ambient: [0.5, 0.5, 0.5, 1.0] }
    }
}

/// `D3DMATERIAL9`'s three colours (`diffuse`, `ambient`, `emissive`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnvironmentMaterial {
    pub diffuse: [f32; 4],
    pub ambient: [f32; 4],
    pub emissive: [f32; 4],
}

impl Default for EnvironmentMaterial {
    fn default() -> Self {
        Self { diffuse: [0.8, 0.8, 0.8, 1.0], ambient: [0.8, 0.8, 0.8, 1.0], emissive: [0.8, 0.8, 0.8, 1.0] }
    }
}

/// The `Fog` group.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fog {
    pub enable: bool,
    /// `bDensityFog`: exponential fog at a fixed 0.00015 density.
    pub density: bool,
    pub near_distance: f32,
    pub far_distance: f32,
    /// r, g, b, a.
    pub color: [f32; 4],
}

impl Default for Fog {
    fn default() -> Self {
        Self {
            enable: false,
            density: false,
            near_distance: 25600.0 * 0.5,
            far_distance: 25600.0 * 0.7,
            color: [0.5, 0.5, 0.5, 1.0],
        }
    }
}

/// The `Filter` group (a full-screen colour filter, not applied by the
/// viewer yet).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Filter {
    pub enable: bool,
    pub color: [f32; 4],
    pub alpha_src: u8,
    pub alpha_dest: u8,
}

impl Default for Filter {
    /// `Environment_Init`: `D3DBLEND_ONE` is 2.
    fn default() -> Self {
        Self { enable: false, color: [0.3, 0.1, 0.1, 0.0], alpha_src: 2, alpha_dest: 2 }
    }
}

/// A `TGradientColor`: two colours.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GradientColor {
    pub first: [f32; 4],
    pub second: [f32; 4],
}

/// The `SkyBox` group, including its cloud layer.
#[derive(Debug, Clone, PartialEq)]
pub struct SkyBox {
    pub scale: [f32; 3],
    pub texture_render_mode: bool,
    pub gradient_level_upper: u8,
    pub gradient_level_lower: u8,
    /// `front`, `back`, `left`, `right`, `top`, `bottom`.
    pub face_file_names: [String; 6],
    pub cloud_scale: [f32; 2],
    pub cloud_height: f32,
    pub cloud_texture_scale: [f32; 2],
    pub cloud_speed: [f32; 2],
    pub cloud_texture_file_name: String,
    pub cloud_color: GradientColor,
    pub gradient: Vec<GradientColor>,
}

impl Default for SkyBox {
    fn default() -> Self {
        Self {
            scale: [3500.0; 3],
            texture_render_mode: false,
            gradient_level_upper: 0,
            gradient_level_lower: 0,
            face_file_names: std::array::from_fn(|_| String::new()),
            cloud_scale: [200_000.0; 2],
            cloud_height: 30_000.0,
            cloud_texture_scale: [4.0; 2],
            cloud_speed: [0.001; 2],
            cloud_texture_file_name: String::new(),
            cloud_color: GradientColor::default(),
            gradient: Vec::new(),
        }
    }
}

/// The `LensFlare` group.
#[derive(Debug, Clone, PartialEq)]
pub struct LensFlare {
    pub enable: bool,
    pub brightness_color: [f32; 4],
    pub max_brightness: f32,
    pub main_enable: bool,
    pub main_texture_file_name: String,
    pub main_size: f32,
}

impl Default for LensFlare {
    fn default() -> Self {
        Self {
            enable: false,
            brightness_color: [1.0; 4],
            max_brightness: 1.0,
            main_enable: false,
            main_texture_file_name: String::new(),
            main_size: 0.2,
        }
    }
}

/// `TEnvironmentData` after `Environment_Init` + `Environment_Load`.
#[derive(Debug, Clone, PartialEq)]
pub struct EnvironmentData {
    pub lights: [DirectionalLight; ENV_DIRLIGHT_NUM],
    pub material: EnvironmentMaterial,
    pub fog: Fog,
    pub filter: Filter,
    pub wind_strength: f32,
    pub wind_random: f32,
    pub sky_box: SkyBox,
    pub lens_flare: LensFlare,
    /// `bReserve`: the map keeps its own environment instead of a blended one.
    pub reserve: bool,
}

impl Default for EnvironmentData {
    /// `Environment_Init`. `Environment_Load` never reads a `Wind` group, so
    /// these two values are always the init ones.
    fn default() -> Self {
        Self {
            lights: [DirectionalLight::default(); ENV_DIRLIGHT_NUM],
            material: EnvironmentMaterial::default(),
            fog: Fog::default(),
            filter: Filter::default(),
            wind_strength: 0.2,
            wind_random: 0.0,
            sky_box: SkyBox::default(),
            lens_flare: LensFlare::default(),
            reserve: false,
        }
    }
}

/// Why `Environment_Load` returned false.
#[derive(Debug)]
pub enum EnvironmentError {
    /// `CTextFileLoader::Load` failed (the file is missing).
    Pack(pack::Error),
    /// The braces did not balance (`Load` returned false).
    Unbalanced,
}

impl std::fmt::Display for EnvironmentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pack(e) => write!(f, "{e}"),
            Self::Unbalanced => f.write_str("unbalanced braces"),
        }
    }
}

impl std::error::Error for EnvironmentError {}

/// `Environment_Load`: reads and parses `name` through the pack manager.
pub fn load_environment(manager: &PackManager, name: impl AsRef<[u8]>) -> Result<EnvironmentData, EnvironmentError> {
    let script = script::load(manager, name).map_err(EnvironmentError::Pack)?;
    parse_environment(&script)
}

/// `Environment_Load` on an already parsed script.
pub fn parse_environment(script: &script::Script) -> Result<EnvironmentData, EnvironmentError> {
    if !script.complete {
        return Err(EnvironmentError::Unbalanced);
    }
    let mut env = EnvironmentData::default();
    let root = &script.root;
    if let Some(reserve) = root.token_bool("reserved") {
        env.reserve = reserve;
    }

    if let Some(light) = root.child("directionallight") {
        // `D3DVECTOR v3Dir{}`: a missing Direction leaves it zero.
        let direction = light.token_vector3("direction").unwrap_or([0.0; 3]);
        for (index, name) in [(ENV_DIRLIGHT_BACKGROUND, "background"), (ENV_DIRLIGHT_CHARACTER, "character")] {
            if let Some(node) = light.child(name) {
                let entry = &mut env.lights[index];
                entry.direction = direction;
                if let Some(enable) = node.token_bool("enable") {
                    entry.enable = enable;
                }
                if let Some(color) = node.token_vector4("diffuse") {
                    entry.diffuse = color;
                }
                if let Some(color) = node.token_vector4("ambient") {
                    entry.ambient = color;
                }
            }
        }
    }

    if let Some(material) = root.child("material") {
        if let Some(color) = material.token_vector4("diffuse") {
            env.material.diffuse = color;
        }
        if let Some(color) = material.token_vector4("ambient") {
            env.material.ambient = color;
        }
        if let Some(color) = material.token_vector4("emissive") {
            env.material.emissive = color;
        }
    }

    if let Some(fog) = root.child("fog") {
        if let Some(enable) = fog.token_bool("enable") {
            env.fog.enable = enable;
        }
        if let Some(density) = fog.token_bool("isdensity") {
            env.fog.density = density;
        }
        if let Some(near) = fog.token_float("neardistance") {
            env.fog.near_distance = near;
        }
        if let Some(far) = fog.token_float("fardistance") {
            env.fog.far_distance = far;
        }
        if let Some(color) = fog.token_vector4("color") {
            env.fog.color = color;
        }
    }

    if let Some(filter) = root.child("filter") {
        if let Some(enable) = filter.token_bool("enable") {
            env.filter.enable = enable;
        }
        if let Some(color) = filter.token_vector4("color") {
            env.filter.color = color;
        }
        if let Some(alpha) = filter.token_byte("alphasrc") {
            env.filter.alpha_src = alpha;
        }
        if let Some(alpha) = filter.token_byte("alphadest") {
            env.filter.alpha_dest = alpha;
        }
    }

    if let Some(sky) = root.child("skybox") {
        load_sky_box(&mut env.sky_box, sky);
    }

    if let Some(flare) = root.child("lensflare") {
        if let Some(enable) = flare.token_bool("enable") {
            env.lens_flare.enable = enable;
        }
        if let Some(color) = flare.token_vector4("brightnesscolor") {
            env.lens_flare.brightness_color = color;
        }
        if let Some(max) = flare.token_float("maxbrightness") {
            env.lens_flare.max_brightness = max;
        }
        if let Some(enable) = flare.token_bool("mainflareenable") {
            env.lens_flare.main_enable = enable;
        }
        if let Some(name) = flare.token_string("mainflaretexturefilename") {
            env.lens_flare.main_texture_file_name = String::from_utf8_lossy(name).into_owned();
        }
        if let Some(size) = flare.token_float("mainflaresize") {
            env.lens_flare.main_size = size;
        }
    }

    Ok(env)
}

/// The `SkyBox` group. Kept separate because the two vector tokens (`CloudColor`
/// and `Gradient`) have their own length rules.
fn load_sky_box(sky: &mut SkyBox, node: &Node) {
    if let Some(mode) = node.token_bool("btexturerendermode") {
        sky.texture_render_mode = mode;
    }
    if let Some(scale) = node.token_vector3("scale") {
        sky.scale = scale;
    }
    if let Some(level) = node.token_byte("gradientlevelupper") {
        sky.gradient_level_upper = level;
    }
    if let Some(level) = node.token_byte("gradientlevellower") {
        sky.gradient_level_lower = level;
    }
    for (index, key) in [
        "frontfacefilename",
        "backfacefilename",
        "leftfacefilename",
        "rightfacefilename",
        "topfacefilename",
        "bottomfacefilename",
    ]
    .into_iter()
    .enumerate()
    {
        if let Some(name) = node.token_string(key) {
            sky.face_file_names[index] = String::from_utf8_lossy(name).into_owned();
        }
    }
    if let Some(scale) = node.token_vector2("cloudscale") {
        sky.cloud_scale = scale;
    }
    if let Some(height) = node.token_float("cloudheight") {
        sky.cloud_height = height;
    }
    if let Some(scale) = node.token_vector2("cloudtexturescale") {
        sky.cloud_texture_scale = scale;
    }
    if let Some(speed) = node.token_vector2("cloudspeed") {
        sky.cloud_speed = speed;
    }
    if let Some(name) = node.token_string("cloudtexturefilename") {
        sky.cloud_texture_file_name = String::from_utf8_lossy(name).into_owned();
    }

    // `GetTokenVector` + `size() % 8 == 0`. C++ reads the first eight tokens
    // unconditionally, which throws on an empty list; an empty list leaves
    // the colours untouched here (D7).
    if let Some(tokens) = node.tokens("cloudcolor")
        && tokens.len() % 8 == 0
        && let Some(color) = gradient_at(tokens, 0)
    {
        sky.cloud_color = color;
    }

    let levels = sky.gradient_level_upper.wrapping_add(sky.gradient_level_lower) as usize;
    if let Some(tokens) = node.tokens("gradient")
        && tokens.len() % 8 == 0
        && tokens.len() / 8 == levels
    {
        sky.gradient = (0..levels).filter_map(|i| gradient_at(tokens, i * 8)).collect();
    }
}

/// One `TGradientColor` from eight floats starting at `offset`.
fn gradient_at(tokens: &[Vec<u8>], offset: usize) -> Option<GradientColor> {
    let tokens = tokens.get(offset..offset + 8)?;
    let value = |i: usize| script::atof_f32(&tokens[i]);
    Some(GradientColor { first: std::array::from_fn(value), second: std::array::from_fn(|i| value(4 + i)) })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load(text: &str) -> EnvironmentData {
        parse_environment(&script::parse(text.as_bytes())).unwrap()
    }

    #[test]
    fn defaults_are_the_environment_init_values() {
        let env = EnvironmentData::default();
        assert!(!env.fog.enable);
        assert_eq!(env.fog.near_distance, 12800.0);
        assert_eq!(env.fog.far_distance, 17920.0);
        assert_eq!(env.fog.color, [0.5, 0.5, 0.5, 1.0]);
        assert_eq!(env.material.diffuse, [0.8, 0.8, 0.8, 1.0]);
        assert_eq!(env.lights[0].direction, [0.5, 0.5, -0.5]);
        assert_eq!(env.lights[0].ambient, [0.5, 0.5, 0.5, 1.0]);
        assert_eq!(env.filter.alpha_src, 2);
        assert_eq!(env.sky_box.scale, [3500.0; 3]);
        assert_eq!(env.sky_box.cloud_speed, [0.001; 2]);
        assert_eq!(env.lens_flare.main_size, 0.2);
        assert_eq!(env.wind_strength, 0.2);
    }

    #[test]
    fn parses_the_groups_the_client_uses() {
        let env = load(
            r#"
ScriptType EnvironmentData
Reserved 1
Group DirectionalLight
{
    Direction 0.35 0.56 -0.74
    Group Background
    {
        Enable 1
        Diffuse 1.0 0.97 0.97 1.0
        Ambient 0.0 0.0 0.0 1.0
    }
    Group Character
    {
        Enable 0
        Diffuse 0.5 0.5 0.5 1.0
    }
}
Group Fog
{
    Enable 1
    IsDensity 1
    NearDistance 100.0
    FarDistance 200.0
    Color 0.1 0.2 0.3 1.0
}
Group SkyBox
{
    Scale 1 2 3
    GradientLevelUpper 1
    GradientLevelLower 1
    FrontFaceFileName "d:/a.dds"
    CloudColor 1 2 3 4 5 6 7 8
    List Gradient
    {
        0 0 0 0 1 1 1 1
        2 2 2 2 3 3 3 3
    }
}
Group LensFlare
{
    Enable 1
    MainFlareEnable 1
    MainFlareTextureFileName "d:/flare.dds"
    MainFlareSize 0.5
}
"#,
        );
        assert!(env.reserve);
        assert_eq!(env.lights[0].direction, [0.35, 0.56, -0.74]);
        assert!(env.lights[0].enable);
        assert_eq!(env.lights[0].diffuse, [1.0, 0.97, 0.97, 1.0]);
        assert!(!env.lights[1].enable);
        assert_eq!(env.lights[1].diffuse, [0.5, 0.5, 0.5, 1.0]);
        // A missing Ambient keeps the Environment_Init default.
        assert_eq!(env.lights[1].ambient, [0.5, 0.5, 0.5, 1.0]);

        assert!(env.fog.enable && env.fog.density);
        assert_eq!(env.fog.near_distance, 100.0);
        assert_eq!(env.fog.color, [0.1, 0.2, 0.3, 1.0]);

        assert_eq!(env.sky_box.scale, [1.0, 2.0, 3.0]);
        assert_eq!(env.sky_box.face_file_names[0], "d:/a.dds");
        assert_eq!(env.sky_box.cloud_color.first, [1.0, 2.0, 3.0, 4.0]);
        assert_eq!(env.sky_box.cloud_color.second, [5.0, 6.0, 7.0, 8.0]);
        // Upper + lower = 2, so both gradient entries are kept.
        assert_eq!(env.sky_box.gradient.len(), 2);
        assert_eq!(env.sky_box.gradient[1].second, [3.0; 4]);

        assert!(env.lens_flare.enable && env.lens_flare.main_enable);
        assert_eq!(env.lens_flare.main_texture_file_name, "d:/flare.dds");
        assert_eq!(env.lens_flare.main_size, 0.5);
    }

    #[test]
    fn a_vector_with_the_wrong_length_is_ignored() {
        let env = load("Group Fog\n{\n    Color 0.1 0.2 0.3\n}\nGroup SkyBox\n{\n    Scale 1 2\n}\n");
        assert_eq!(env.fog.color, Fog::default().color);
        assert_eq!(env.sky_box.scale, SkyBox::default().scale);
    }

    #[test]
    fn gradients_need_one_entry_per_level() {
        // Two levels declared but one entry supplied: C++ clears the vector
        // only after the count check, so nothing is stored.
        let env = load(
            "Group SkyBox\n{\n    GradientLevelUpper 1\n    GradientLevelLower 1\n    List Gradient\n    {\n        0 0 0 0 1 1 1 1\n    }\n}\n",
        );
        assert!(env.sky_box.gradient.is_empty());
    }

    #[test]
    fn unbalanced_braces_fail_the_load() {
        let err = parse_environment(&script::parse(b"Group Fog\n{\n")).unwrap_err();
        assert!(matches!(err, EnvironmentError::Unbalanced));
    }
}
