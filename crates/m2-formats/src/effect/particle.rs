//! `CParticleSystemData` (`ParticleSystemData.cpp`) with its
//! `CEmitterProperty` (`EmitterProperty.cpp`) and `CParticleProperty`
//! (`ParticleProperty.cpp`).

use super::{
    Cursor, ElementBase, Error, TimeEvent, TimeEventTableColor, TimeEventTableFloat, color_to_dword, resolve,
    time_event_blend_value, token_time_event_float,
};

/// `D3DBLEND_ONE`.
const D3DBLEND_ONE: u8 = 2;
/// `D3DBLEND_SRCALPHA`.
const D3DBLEND_SRCALPHA: u8 = 5;
/// `D3DTOP_MODULATE`.
const D3DTOP_MODULATE: u8 = 4;

/// `CEmitterProperty`: where, how many and how fast particles are emitted.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EmitterProperty {
    pub max_emission_count: u32,
    pub cycle_length: f32,
    /// `BOOL`: the `atoi` of the token, not just 0 or 1.
    pub cycle_loop: i32,
    pub loop_count: i32,
    /// `EMITTER_SHAPE_POINT`, `_ELLIPSE`, `_SQUARE`, `_SPHERE` (0–3).
    pub emitter_shape: u8,
    /// `EMITTER_ADVANCED_TYPE_FREE`, `_OUTER`, `_INNER` (0–2).
    pub emitter_advanced_type: u8,
    pub emit_from_edge: i32,
    pub emitting_size: [f32; 3],
    pub emitting_radius: f32,
    pub emitting_direction: [f32; 3],
    pub emitting_size_events: TimeEventTableFloat,
    pub emitting_angular_velocity: TimeEventTableFloat,
    pub emitting_direction_x: TimeEventTableFloat,
    pub emitting_direction_y: TimeEventTableFloat,
    pub emitting_direction_z: TimeEventTableFloat,
    pub emitting_velocity: TimeEventTableFloat,
    pub emission_count_per_second: TimeEventTableFloat,
    pub life_time: TimeEventTableFloat,
    pub size_x: TimeEventTableFloat,
    pub size_y: TimeEventTableFloat,
}

/// `CParticleProperty` (client build, without `WORLD_EDITOR`): how each
/// particle looks and moves.
#[derive(Debug, Clone, PartialEq)]
pub struct ParticleProperty {
    pub src_blend_type: u8,
    pub dest_blend_type: u8,
    pub color_operation_type: u8,
    /// `BILLBOARD_TYPE_*` (`Type.h`).
    pub billboard_type: u8,
    /// `ROTATION_TYPE_NONE`, `_TIME_EVENT`, `_CW`, `_CCW`,
    /// `_RANDOM_DIRECTION` (0–4).
    pub rotation_type: u8,
    pub rotation_speed: f32,
    pub rotation_random_starting_begin: u16,
    pub rotation_random_starting_end: u16,
    pub attach: i32,
    pub stretch: i32,
    /// `TEXTURE_ANIMATION_TYPE_*` (0–4).
    pub tex_ani_type: u8,
    pub tex_ani_delay: f32,
    pub tex_ani_random_start_frame: i32,
    pub gravity: TimeEventTableFloat,
    pub air_resistance: TimeEventTableFloat,
    pub scale_x: TimeEventTableFloat,
    pub scale_y: TimeEventTableFloat,
    /// The four `TimeEventColorRed/Green/Blue` and `TimeEventAlpha` tables
    /// merged at the union of their times, as `0xAARRGGBB`.
    pub color: TimeEventTableColor,
    pub rotation: TimeEventTableFloat,
    /// `m_ImageVector`: the requested texture names (one per frame).
    pub textures: Vec<Vec<u8>>,
}

impl Default for ParticleProperty {
    /// `CParticleProperty::Clear`.
    fn default() -> Self {
        Self {
            src_blend_type: D3DBLEND_SRCALPHA,
            dest_blend_type: D3DBLEND_ONE,
            color_operation_type: D3DTOP_MODULATE,
            billboard_type: 0,
            rotation_type: 0,
            rotation_speed: 0.0,
            rotation_random_starting_begin: 0,
            rotation_random_starting_end: 0,
            attach: 0,
            stretch: 0,
            tex_ani_type: 0,
            tex_ani_delay: 0.05,
            tex_ani_random_start_frame: 0,
            gravity: Vec::new(),
            air_resistance: Vec::new(),
            scale_x: Vec::new(),
            scale_y: Vec::new(),
            color: Vec::new(),
            rotation: Vec::new(),
            textures: Vec::new(),
        }
    }
}

/// `CParticleSystemData`: one `Group Particle` of an effect.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ParticleSystemData {
    pub base: ElementBase,
    pub emitter: EmitterProperty,
    pub particle: ParticleProperty,
}

/// A table with the single event `0 → 0`: the default of the optional
/// emitter tables.
fn zero_table() -> TimeEventTableFloat {
    vec![TimeEvent { time: 0.0, value: 0.0 }]
}

impl ParticleSystemData {
    /// `CEffectElementBase::LoadScript` + `CParticleSystemData::OnLoadScript`
    /// on a cleared element. The result (ignored by `CEffectData`) is
    /// dropped: what was read before a failure stays.
    pub(crate) fn load(&mut self, cursor: &mut Cursor, script_name: &[u8]) -> Result<(), Error> {
        if self.base.load(cursor)? {
            let ok = cursor.goto_child(b"emitterproperty", |c| self.load_emitter(c))?;
            if ok {
                cursor.goto_child(b"particleproperty", |c| self.load_particle(c, script_name))?;
            }
        }
        Ok(())
    }

    /// The `emitterproperty` block; `Ok(false)` where C++ returns `FALSE`.
    fn load_emitter(&mut self, cursor: &mut Cursor) -> Result<bool, Error> {
        let node = cursor.node();
        let e = &mut self.emitter;

        let Some(max) = node.token_dword("maxemissioncount") else {
            return Ok(false);
        };
        e.max_emission_count = max;
        e.cycle_length = node.token_float("cyclelength").unwrap_or(0.05);
        e.cycle_loop = node.token_int("cycleloopenable").unwrap_or(0);
        e.loop_count = node.token_int("loopcount").unwrap_or(0);

        let Some(shape) = node.token_byte("emittershape") else {
            return Ok(false);
        };
        e.emitter_shape = shape;
        match node.token_byte("emitteradvancedtype") {
            Some(t) => e.emitter_advanced_type = t,
            // C++ bug: resets the shape (to EMITTER_ADVANCED_TYPE_FREE, 0)
            // instead of the advanced type. Mirrored.
            None => e.emitter_shape = 0,
        }
        e.emitting_size = node.token_vector3("emittingsize").unwrap_or([0.0; 3]);
        e.emitting_radius = node.token_float("emittingradius").unwrap_or(0.0);
        e.emit_from_edge = node.token_int("emitteremitfromedgeflag").unwrap_or(0);
        e.emitting_direction = node.token_vector3("emittingdirection").unwrap_or([0.0; 3]);

        let optional = |key: &[u8]| -> Result<TimeEventTableFloat, Error> {
            Ok(token_time_event_float(node, key)?.unwrap_or_else(zero_table))
        };
        e.emitting_size_events = optional(b"timeeventemittingsize")?;
        e.emitting_angular_velocity = optional(b"timeeventemittingangularvelocity")?;
        e.emitting_direction_x = optional(b"timeeventemittingdirectionx")?;
        e.emitting_direction_y = optional(b"timeeventemittingdirectiony")?;
        e.emitting_direction_z = optional(b"timeeventemittingdirectionz")?;
        e.emitting_velocity = optional(b"timeeventemittingvelocity")?;

        for (key, table) in [
            (&b"timeeventemissioncountpersecond"[..], &mut e.emission_count_per_second),
            (b"timeeventlifetime", &mut e.life_time),
            (b"timeeventsizex", &mut e.size_x),
            (b"timeeventsizey", &mut e.size_y),
        ] {
            match token_time_event_float(node, key)? {
                Some(t) => *table = t,
                None => return Ok(false),
            }
        }
        Ok(true)
    }

    /// The `particleproperty` block; `Ok(false)` where C++ returns `FALSE`.
    fn load_particle(&mut self, cursor: &mut Cursor, script_name: &[u8]) -> Result<bool, Error> {
        let node = cursor.node();
        let p = &mut self.particle;

        p.src_blend_type = node.token_byte("srcblendtype").unwrap_or(D3DBLEND_SRCALPHA);
        p.dest_blend_type = node.token_byte("destblendtype").unwrap_or(D3DBLEND_ONE);
        p.color_operation_type = node.token_byte("coloroperationtype").unwrap_or(D3DTOP_MODULATE);

        macro_rules! required {
            ($field:expr, $value:expr) => {
                match $value {
                    Some(v) => $field = v,
                    None => return Ok(false),
                }
            };
        }
        required!(p.billboard_type, node.token_byte("billboardtype"));
        required!(p.rotation_type, node.token_byte("rotationtype"));
        required!(p.rotation_speed, node.token_float("rotationspeed"));
        required!(p.rotation_random_starting_begin, node.token_word("rotationrandomstartingbegin"));
        required!(p.rotation_random_starting_end, node.token_word("rotationrandomstartingend"));
        p.attach = node.token_int("attachenable").unwrap_or(0);
        required!(p.stretch, node.token_int("stretchenable"));
        required!(p.tex_ani_type, node.token_byte("texanitype"));
        required!(p.tex_ani_delay, node.token_float("texanidelay"));
        required!(p.tex_ani_random_start_frame, node.token_int("texanirandomstartframeenable"));

        // A single value is a one-event table (pushed onto the cleared one).
        if let Some(g) = node.token_float("gravity") {
            p.gravity.push(TimeEvent { time: 0.0, value: g });
        } else {
            p.gravity = token_time_event_float(node, b"timeeventgravity")?.unwrap_or_default();
        }
        if let Some(a) = node.token_float("airresistance") {
            p.air_resistance.push(TimeEvent { time: 0.0, value: a });
        } else {
            p.air_resistance = token_time_event_float(node, b"timeeventairresistance")?.unwrap_or_default();
        }

        required!(p.scale_x, token_time_event_float(node, b"timeeventscalex")?);
        required!(p.scale_y, token_time_event_float(node, b"timeeventscaley")?);

        let mut channels: [TimeEventTableFloat; 4] = Default::default();
        for (key, table) in
            [&b"timeeventcolorred"[..], b"timeeventcolorgreen", b"timeeventcolorblue", b"timeeventalpha"]
                .into_iter()
                .zip(&mut channels)
        {
            required!(*table, token_time_event_float(node, key)?);
        }
        p.color = merge_color_channels(&channels);

        required!(p.rotation, token_time_event_float(node, b"timeeventrotation")?);

        let Some(textures) = node.tokens("texturefiles") else {
            return Ok(false);
        };
        p.textures.extend(textures.iter().map(|t| resolve(script_name, t)));
        Ok(true)
    }
}

/// The color table of a non-`WORLD_EDITOR` build: at every time any of
/// the r, g, b, a tables has (a `std::set<float>`), each channel blended
/// and packed with `D3DXCOLOR::operator DWORD`.
fn merge_color_channels(channels: &[TimeEventTableFloat; 4]) -> TimeEventTableColor {
    // std::set<float>: a time equal (neither less) to one already there,
    // such as -0 after 0, is not inserted.
    let mut times: Vec<f32> = Vec::new();
    for event in channels.iter().flatten() {
        let t = event.time;
        let pos = times.partition_point(|&x| x < t);
        if pos == times.len() || t < times[pos] {
            times.insert(pos, t);
        }
    }
    times
        .into_iter()
        .map(|time| {
            let rgba: [f32; 4] = std::array::from_fn(|i| time_event_blend_value(time, &channels[i]));
            TimeEvent { time, value: color_to_dword(rgba) }
        })
        .collect()
}
