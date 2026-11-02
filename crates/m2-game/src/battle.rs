//! Client-side hit and movement collision: the geometry of
//! `ActorInstanceCollisionDetection.cpp`, `GameUtil.cpp`,
//! `CollisionData.cpp` and `lineintersect_utils.cpp`.
//!
//! The C++ client decides on its own whether a swing connects: the motion's
//! `HitPosition` sweep (`.msa`) is rotated to the attacker's heading and
//! tested against the victim's defending spheres (`.msm`); only a hit sends
//! `CG_ATTACK` (`CPythonPlayerEventHandler::OnHit`). Movement is blocked by the
//! static collision shapes of buildings and trees (`.mdatr`, SpeedTree) and by
//! other actors' body spheres.
//!
//! Everything here works in Metin2 space (`x` east, `y` = minus the map's `y`,
//! `z` up), on plain values, so it can be tested without Bevy entities.

use std::collections::HashMap;

use bevy::math::{Mat3, Vec3};
use m2_assets::race::{MotionAttackData, MotionSplash};

/// `NRaceData::HIT_TYPE_*`.
pub const HIT_TYPE_NONE: i32 = 0;
pub const HIT_TYPE_GREAT: i32 = 1;
pub const HIT_TYPE_GOOD: i32 = 2;
/// `NRaceData::MOTION_TYPE_*`.
pub const MOTION_TYPE_NORMAL: i32 = 0;
pub const MOTION_TYPE_COMBO: i32 = 1;
/// `NRaceData::ATTACK_TYPE_SNIPE`.
pub const ATTACK_TYPE_SNIPE: i32 = 1;

/// `c_fAttackRadius` of `__NormalAttackProcess`.
const ATTACK_RADIUS: f32 = 20.0;
/// `gc_fReduceMove`.
const REDUCE_MOVE: f32 = 0.5;
/// `MY_EPSILON` of `lineintersect_utils.cpp`.
const EPSILON: f32 = 0.1;

/// `CDynamicSphereInstance`: a sphere that moved from `last` to `position`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DynamicSphere {
    pub position: Vec3,
    pub last: Vec3,
    pub radius: f32,
}

impl DynamicSphere {
    pub fn still(position: Vec3, radius: f32) -> Self {
        DynamicSphere { position, last: position, radius }
    }
}

/// `GetVector3Distance`: the *squared* distance in the ground plane (no `z`).
pub fn ground_distance_squared(a: Vec3, b: Vec3) -> f32 {
    (a.x - b.x) * (a.x - b.x) + (a.y - b.y) * (a.y - b.y)
}

/// `square_distance_between_linesegment_and_point` (`GrpMath.h`).
pub fn segment_point_distance_squared(p1: Vec3, p2: Vec3, x: Vec3) -> f32 {
    let l = (p2 - p1).length_squared();
    let d = (x - p1).dot(p2 - p1);
    if d <= 0.0 {
        (x - p1).length_squared()
    } else if d >= l {
        (x - p2).length_squared()
    } else {
        (x - p1).cross(p2 - p1).length_squared() / l
    }
}

fn out_of_range(a: f32) -> bool {
    !(0.0..=1.0).contains(&a)
}

/// The `D3DXVECTOR3` `FindNearestPointOnLineSegment`: always clamped; a
/// degenerate segment leaves `parameter` as it was.
fn nearest_on_segment(a1: Vec3, l: Vec3, b: Vec3, parameter: &mut f32) -> Vec3 {
    let d = l.length_squared();
    if d < EPSILON * EPSILON {
        return a1;
    }
    *parameter = ((b - a1).dot(l) / d).clamp(0.0, 1.0);
    a1 + *parameter * l
}

/// `FindNearestPointOfParallelLineSegments` (finite segments).
fn nearest_of_parallel(a1: Vec3, a2: Vec3, la: Vec3, b1: Vec3, b2: Vec3, lb: Vec3) -> (Vec3, Vec3) {
    let mut s = [0.0f32; 2];
    nearest_on_segment(a1, la, b1, &mut s[0]);
    nearest_on_segment(a1, la, b2, &mut s[1]);
    let (out_a, out_b);
    if s[0] < 0.0 && s[1] < 0.0 {
        out_a = a1;
        out_b = if s[0] < s[1] { b2 } else { b1 };
    } else if s[0] > 1.0 && s[1] > 1.0 {
        out_a = a2;
        out_b = if s[0] < s[1] { b1 } else { b2 };
    } else {
        let temp = 0.5 * (s[0].clamp(0.0, 1.0) + s[1].clamp(0.0, 1.0));
        out_a = a1 + temp * la;
        let mut t = temp;
        out_b = nearest_on_segment(b1, lb, out_a, &mut t);
    }
    (out_a, out_b)
}

/// `AdjustNearestPoints`.
fn adjust_nearest(a1: Vec3, la: Vec3, b1: Vec3, lb: Vec3, mut s: f32, mut t: f32) -> (Vec3, Vec3) {
    let (mut out_a, mut out_b);
    if out_of_range(s) && out_of_range(t) {
        s = s.clamp(0.0, 1.0);
        out_a = a1 + s * la;
        out_b = nearest_on_segment(b1, lb, out_a, &mut t);
        if out_of_range(t) {
            t = t.clamp(0.0, 1.0);
            out_b = b1 + t * lb;
            out_a = nearest_on_segment(a1, la, out_b, &mut s);
            out_b = nearest_on_segment(b1, lb, out_a, &mut t);
        }
    } else if out_of_range(s) {
        s = s.clamp(0.0, 1.0);
        out_a = a1 + s * la;
        out_b = nearest_on_segment(b1, lb, out_a, &mut t);
    } else {
        t = t.clamp(0.0, 1.0);
        out_b = b1 + t * lb;
        out_a = nearest_on_segment(a1, la, out_b, &mut s);
    }
    (out_a, out_b)
}

/// `IntersectLineSegments(A1, A2, B1, B2, OutA, OutB)`: the closest points of
/// two segments.
pub fn intersect_line_segments(a1: Vec3, a2: Vec3, b1: Vec3, b2: Vec3) -> (Vec3, Vec3) {
    let mut temp = 0.0;
    let la = a2 - a1;
    let lb = b2 - b1;
    let l11 = la.length_squared();
    let l22 = lb.length_squared();
    if l11 < EPSILON * EPSILON {
        return (a1, nearest_on_segment(b1, lb, a1, &mut temp));
    }
    if l22 < EPSILON * EPSILON {
        return (nearest_on_segment(a1, la, b1, &mut temp), b1);
    }
    let ab = b1 - a1;
    let l12 = -la.dot(lb);
    let det = l11 * l22 - l12 * l12;
    if det.abs() < EPSILON {
        return nearest_of_parallel(a1, a2, la, b1, b2, lb);
    }
    let ra = la.dot(ab);
    // The `D3DXVECTOR3` overload drops the minus of the original
    // `rb = -Lb·AB` (its comment still shows it), which mirrors `t` and moves
    // the closest points. Hit tests depend on it, so it is kept
    // (`c++_rust_bugs.md`, SEG-RB).
    let rb = lb.dot(ab);
    let t = (l11 * rb - ra * l12) / det;
    let s = (ra - l12 * t) / l11;
    if out_of_range(s) || out_of_range(t) {
        adjust_nearest(a1, la, b1, lb, s, t)
    } else {
        (a1 + s * la, b1 + t * lb)
    }
}

/// The AABB pre-check shared by the two dynamic tests.
fn sweeps_overlap(a: &DynamicSphere, b: &DynamicSphere) -> bool {
    let (amin, amax) = (a.last.min(a.position) - a.radius, a.last.max(a.position) + a.radius);
    let (bmin, bmax) = (b.last.min(b.position) - b.radius, b.last.max(b.position) + b.radius);
    !(bmax.x < amin.x || amax.x < bmin.x || bmax.y < amin.y || amax.y < bmin.y || bmax.z < amin.z || amax.z < bmin.z)
}

/// `DetectCollisionDynamicSphereVSDynamicSphere`.
pub fn sphere_vs_sphere(a: &DynamicSphere, b: &DynamicSphere) -> bool {
    if !sweeps_overlap(a, b) {
        return false;
    }
    let r = a.radius + b.radius;
    let (pa, pb) = intersect_line_segments(a.last, a.position, b.last, b.position);
    (pa - pb).length_squared() <= r * r
}

/// `DetectCollisionDynamicZCylinderVSDynamicZCylinder`: the same test with
/// every height flattened to 0.
pub fn zcylinder_vs_zcylinder(a: &DynamicSphere, b: &DynamicSphere) -> bool {
    let flat = |s: &DynamicSphere| DynamicSphere {
        position: s.position.with_z(0.0),
        last: s.last.with_z(0.0),
        radius: s.radius,
    };
    sphere_vs_sphere(&flat(a), &flat(b))
}

/// Rotates an actor-local point by the actor's heading (degrees), as
/// `__NormalAttackProcess` does (`x·c − y·s`, `x·s + y·c`).
pub fn rotate_local(v: Vec3, rotation: f32) -> Vec3 {
    let (s, c) = rotation.to_radians().sin_cos();
    Vec3::new(v.x * c - v.y * s, v.x * s + v.y * c, v.z)
}

/// `m_HitDataMap`: which victims each hit window already struck, and until
/// when they cannot be struck again. Cleared when an attack motion starts.
#[derive(Debug, Default, Clone)]
pub struct HitTracker {
    by_hit: HashMap<usize, HashMap<u32, f32>>,
}

impl HitTracker {
    pub fn clear(&mut self) {
        self.by_hit.clear();
    }
}

/// `__NormalAttackProcess`: whether the weapon sweep of `attack` between
/// motion times `elapsed - frame` and `elapsed` crosses one of `victim`'s
/// defending spheres. `attacker` is the actor's position and `rotation` its
/// heading. Records the hit in `tracker` (and refuses repeat hits) so one
/// swing hits a victim once. `now` is the attacker's local time.
#[allow(clippy::too_many_arguments)]
pub fn normal_attack_hit(
    attack: &MotionAttackData,
    tracker: &mut HitTracker,
    elapsed: f32,
    frame: f32,
    attacker: Vec3,
    rotation: f32,
    reach_scale: f32,
    victim: u32,
    victim_spheres: &[DynamicSphere],
    now: f32,
) -> bool {
    for (index, hit) in attack.hits.iter().enumerate() {
        if let Some(until) = tracker.by_hit.get(&index).and_then(|victims| victims.get(&victim))
            && (attack.motion_type == MOTION_TYPE_COMBO || *until > now)
        {
            continue;
        }
        // `lower_bound(elapsed - frame)` .. `upper_bound(elapsed)`.
        let from = elapsed - frame;
        for (_, last, position) in hit.positions.iter().filter(|(t, _, _)| *t >= from && *t <= elapsed) {
            let (last, position) = (Vec3::from_array(*last), Vec3::from_array(*position));
            let reached = last + (position - last) * reach_scale;
            let sweep = DynamicSphere {
                position: rotate_local(reached, rotation) + attacker,
                last: rotate_local(last, rotation) + attacker,
                radius: ATTACK_RADIUS,
            };
            if victim_spheres.iter().any(|sphere| zcylinder_vs_zcylinder(&sweep, sphere)) {
                // The first victim of a hit window always counts; only later
                // ones are held to the limit ("Next Hit" in C++). The combo
                // `.msa`s are `MotionType 2` without `HitLimitCount`, so the
                // limit is 0 for them.
                let first = !tracker.by_hit.contains_key(&index);
                let victims = tracker.by_hit.entry(index).or_default();
                victims.insert(victim, now + attack.attack.invisible_time);
                if first {
                    return true;
                }
                let limit = if matches!(attack.motion_type, MOTION_TYPE_NORMAL | MOTION_TYPE_COMBO) {
                    16
                } else {
                    attack.attack.hit_limit_count.max(0) as usize
                };
                return victims.len() <= limit;
            }
        }
    }
    false
}

/// `CActorInstance::isValidAttacking` is always true for an attacking motion
/// (its loop result is ignored); the hit test runs over the whole motion.
pub fn is_attacking(attack: Option<&MotionAttackData>) -> bool {
    attack.is_some()
}

/// `m_kSplashArea` of a `SpecialAttacking` event: the spheres placed around
/// the actor when the event fires, the time it ends and who it hit.
#[derive(Debug, Clone, Default)]
pub struct SplashArea {
    pub spheres: Vec<DynamicSphere>,
    pub ends: f32,
    pub splash: MotionSplash,
    pub hit: HashMap<u32, f32>,
    /// The skill the motion belongs to (`m_kCurMotNode.uSkill`).
    pub skill: u32,
}

impl SplashArea {
    /// `ProcessMotionEventSpecialAttacking`: the sphere offsets turned by
    /// `270 + 360 - rotation` around the actor at `position`.
    pub fn new(splash: &MotionSplash, position: Vec3, rotation: f32, now: f32, duration: f32, skill: u32) -> Self {
        let radian = (270.0 + 360.0 - rotation).to_radians();
        let (s, c) = radian.sin_cos();
        let spheres = splash
            .spheres
            .iter()
            .map(|(radius, offset)| {
                let p = Vec3::new(
                    position.x + offset[0] * s + offset[1] * c,
                    position.y + offset[0] * c - offset[1] * s,
                    position.z + offset[2],
                );
                DynamicSphere::still(p, *radius)
            })
            .collect();
        SplashArea { spheres, ends: now + duration, splash: splash.clone(), hit: HashMap::new(), skill }
    }

    /// `__SplashAttackProcess` against one victim. A snipe splash only hits
    /// `fly_target` when one is set.
    pub fn hit(&mut self, victim: u32, victim_spheres: &[DynamicSphere], now: f32, fly_target: Option<u32>) -> bool {
        if now >= self.ends || self.hit.contains_key(&victim) {
            return false;
        }
        if self.splash.attack.attack_type == ATTACK_TYPE_SNIPE && fly_target.is_some_and(|target| target != victim) {
            return false;
        }
        let touched =
            victim_spheres.iter().any(|defending| self.spheres.iter().any(|sphere| sphere_vs_sphere(sphere, defending)));
        if !touched {
            return false;
        }
        self.hit.insert(victim, now + self.splash.attack.invisible_time);
        let limit = if self.splash.attack.hit_limit_count == 0 { 16 } else { self.splash.attack.hit_limit_count as usize };
        self.hit.len() <= limit
    }
}

/// `CActorInstance::TestActorCollision` on body spheres: `mover` would move
/// into `other` (they touch and the move does not take it away).
pub fn actor_blocks(mover: &[DynamicSphere], other: &[DynamicSphere]) -> bool {
    for a in mover {
        for b in other {
            if sphere_vs_sphere(a, b) {
                return ground_distance_squared(a.position, b.position) <= ground_distance_squared(a.last, b.position);
            }
        }
    }
    false
}

/// One static collision shape of a building or tree, in Metin2 world space
/// (`CBaseCollisionInstance::BuildCollisionInstance`).
#[derive(Debug, Clone, PartialEq)]
pub enum StaticCollision {
    Sphere { position: Vec3, radius: f32 },
    Cylinder { position: Vec3, radius: f32, height: f32 },
    Plane { position: Vec3, normal: Vec3, quad: [Vec3; 4], inside: [Vec3; 4] },
    Aabb { min: Vec3, max: Vec3 },
    Obb { min: Vec3, max: Vec3, rotation_inverse: Mat3 },
}

/// `CStaticCollisionData::dwType`.
pub const COLLISION_PLANE: u32 = 0;
pub const COLLISION_BOX: u32 = 1;
pub const COLLISION_SPHERE: u32 = 2;
pub const COLLISION_CYLINDER: u32 = 3;
pub const COLLISION_AABB: u32 = 4;
pub const COLLISION_OBB: u32 = 5;

/// A row-vector (D3D) transform: `p' = p · rotation + translation`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct D3dTransform {
    /// The upper 3x3, applied as `rotation * p` (column form of the rows).
    pub rotation: Mat3,
    pub translation: Vec3,
}

impl D3dTransform {
    pub fn apply(&self, p: Vec3) -> Vec3 {
        self.rotation * p + self.translation
    }

    /// `D3DXMatrixRotationYawPitchRoll` + translation of a placed object, as
    /// [`m2_world::area::rotation_yaw_pitch_roll`] builds it (row-major
    /// 4x4).
    pub fn from_row_major(m: &[f32; 16]) -> Self {
        // Row vectors: p' = p M, so p'_j = Σ p_i M[i][j]; as a column-form
        // matrix that is Mᵀ, whose columns are M's rows.
        D3dTransform {
            rotation: Mat3::from_cols(
                Vec3::new(m[0], m[1], m[2]),
                Vec3::new(m[4], m[5], m[6]),
                Vec3::new(m[8], m[9], m[10]),
            ),
            translation: Vec3::new(m[12], m[13], m[14]),
        }
    }
}

impl StaticCollision {
    /// `BuildCollisionInstance` for one record of a `.mdatr`, placed by the
    /// object's transform. `quat` is `(x, y, z, w)`. `COLLISION_TYPE_BOX` is
    /// unimplemented in C++ (an assert); it yields `None`, as do unknown types.
    pub fn build(kind: u32, position: Vec3, dimensions: [f32; 3], quat: [f32; 4], transform: &D3dTransform) -> Option<Self> {
        match kind {
            COLLISION_PLANE => {
                // `matRotation * matTranslationLocal * *pMat` (row vectors).
                let q = bevy::math::Quat::from_xyzw(quat[0], quat[1], quat[2], quat[3]);
                // `D3DXMatrixRotationQuaternion` applied to a row vector
                // rotates it by `q`.
                let local = |p: Vec3| q * p;
                let (hw, hl) = (dimensions[0] / 2.0, dimensions[1] / 2.0);
                let quad = [
                    Vec3::new(-hw, -hl, 0.0),
                    Vec3::new(hw, -hl, 0.0),
                    Vec3::new(-hw, hl, 0.0),
                    Vec3::new(hw, hl, 0.0),
                ]
                .map(|p| transform.apply(local(p) + position));
                let line0 = (quad[1] - quad[0]).normalize_or_zero();
                let line1 = (quad[2] - quad[0]).normalize_or_zero();
                let line2 = (quad[1] - quad[3]).normalize_or_zero();
                let line3 = (quad[2] - quad[3]).normalize_or_zero();
                let normal = line0.cross(line1).normalize_or_zero();
                let inside = [normal.cross(line0), line1.cross(normal), line2.cross(normal), normal.cross(line3)];
                Some(StaticCollision::Plane { position: transform.apply(position), normal, quad, inside })
            }
            COLLISION_AABB => {
                let min = transform.apply(position - Vec3::from_array(dimensions));
                let max = transform.apply(position + Vec3::from_array(dimensions));
                Some(StaticCollision::Aabb { min, max })
            }
            COLLISION_OBB => {
                let d = Vec3::from_array(dimensions);
                let centre = (transform.apply(position - d) + transform.apply(position + d)) * 0.5;
                let inverse = if transform.rotation.determinant().abs() > f32::EPSILON {
                    transform.rotation.inverse()
                } else {
                    Mat3::IDENTITY
                };
                Some(StaticCollision::Obb { min: centre - d, max: centre + d, rotation_inverse: inverse })
            }
            COLLISION_SPHERE => Some(StaticCollision::Sphere { position: transform.apply(position), radius: dimensions[0] }),
            COLLISION_CYLINDER => Some(StaticCollision::Cylinder {
                position: transform.apply(position),
                radius: dimensions[0],
                height: dimensions[1],
            }),
            _ => None,
        }
    }

    /// A rough bounding circle in the ground plane, for broad-phase culling.
    pub fn bounds(&self) -> (Vec3, f32) {
        match self {
            StaticCollision::Sphere { position, radius } => (*position, *radius),
            StaticCollision::Cylinder { position, radius, .. } => (*position, *radius),
            StaticCollision::Plane { position, quad, .. } => {
                (*position, quad.iter().map(|q| (q.truncate() - position.truncate()).length()).fold(0.0, f32::max))
            }
            StaticCollision::Aabb { min, max } | StaticCollision::Obb { min, max, .. } => {
                let centre = (*min + *max) * 0.5;
                // An OBB's box is rotated; its half diagonal bounds it anyway.
                (centre, ((*max - *min) * 0.5).truncate().length())
            }
        }
    }

    /// `OnMovementCollisionDynamicSphere`: the moving sphere `s` runs into the
    /// shape (and is not moving away from it).
    pub fn movement_collides(&self, s: &DynamicSphere) -> bool {
        match self {
            StaticCollision::Sphere { position, radius } => {
                let r = radius + s.radius;
                segment_point_distance_squared(s.last, s.position, *position) < r * r
                    && ground_distance_squared(s.position, *position) < ground_distance_squared(s.last, *position)
            }
            StaticCollision::Cylinder { position, radius, height } => {
                if cylinder_vs_sphere(*position, *radius, *height, s)
                    && ground_distance_squared(s.position, *position) < ground_distance_squared(s.last, *position)
                {
                    return true;
                }
                let distance = (s.position - s.last).length();
                if s.radius <= 0.0001 {
                    return false;
                }
                if distance >= s.radius * 2.0 {
                    // C++ steps a copy of the *cylinder* from the sphere's
                    // last position along the move.
                    let steps = (distance / s.radius * 2.0) as i32;
                    let step = (s.position - s.last) / steps as f32;
                    let mut centre = s.last;
                    for _ in 0..steps {
                        centre += step;
                        if cylinder_vs_sphere(centre, *radius, *height, s) {
                            return true;
                        }
                    }
                }
                false
            }
            StaticCollision::Plane { position, normal, quad, inside } => {
                let p1 = normal.dot(s.position - *position);
                let p2 = normal.dot(s.last - *position);
                let crosses = (p1 > 0.0 && p2 < 0.0) || (p1 < 0.0 && p2 > 0.0) || (p1 <= s.radius && p1 >= -s.radius);
                if !crosses {
                    return false;
                }
                let q1 = s.position - quad[0];
                let q2 = s.position - quad[3];
                q1.dot(inside[0]) > -s.radius
                    && q1.dot(inside[1]) > -s.radius
                    && q2.dot(inside[2]) > -s.radius
                    && q2.dot(inside[3]) > -s.radius
                    && normal.dot(s.position - *position).abs() < normal.dot(s.last - *position).abs()
            }
            StaticCollision::Aabb { min, max } => {
                aabb_touches(*min, *max, s.position, s.radius) || aabb_touches(*min, *max, s.last, s.radius)
            }
            StaticCollision::Obb { min, max, rotation_inverse } => {
                let centre = (*min + *max) * 0.5;
                [s.position, s.last].into_iter().any(|p| {
                    let local = *rotation_inverse * (p - centre) + centre;
                    let point = local.clamp(*min, *max);
                    ground_distance_squared(point, local) <= s.radius * s.radius
                })
            }
        }
    }

    /// `OnGetCollisionMovementAdjust`: the slide to add to the move so the
    /// sphere goes round the shape. Boxes do not slide in C++ (they block).
    pub fn movement_adjust(&self, s: &DynamicSphere) -> Vec3 {
        match self {
            StaticCollision::Sphere { position, radius } => round_adjust(*position, *radius, s, false),
            StaticCollision::Cylinder { position, radius, .. } => {
                round_adjust(position.with_z(s.position.z), *radius, s, true)
            }
            StaticCollision::Plane { position, normal, .. } => {
                let advance = s.position - s.last;
                let d = normal.dot(advance);
                if (-0.0001..=0.0001).contains(&d) {
                    return Vec3::ZERO;
                }
                let t = -normal.dot(s.position - *position) / d;
                if d >= 0.0 { t * advance - s.radius * *normal } else { t * advance + s.radius * *normal }
            }
            StaticCollision::Aabb { .. } | StaticCollision::Obb { .. } => Vec3::ZERO,
        }
    }
}

/// `CCylinderCollisionInstance::CollideCylinderVSDynamicSphere`.
fn cylinder_vs_sphere(position: Vec3, radius: f32, height: f32, s: &DynamicSphere) -> bool {
    if s.position.z + s.radius < position.z || s.position.z - s.radius > position.z + height {
        return false;
    }
    let (a, b) = intersect_line_segments(position, position + Vec3::Z * height, s.last, s.position);
    let r = radius + s.radius;
    (a - b).length_squared() <= r * r
}

/// The AABB test of `CAABBCollisionInstance::OnMovementCollisionDynamicSphere`.
/// The C++ clamps `y` into `x` (`v.x = m_attribute.v3Min.y`) and compares a
/// squared ground distance with `radius²`; both are kept (see
/// `c++_rust_bugs.md`, AABB-Y).
fn aabb_touches(min: Vec3, max: Vec3, p: Vec3, radius: f32) -> bool {
    let mut v = p;
    v.x = v.x.max(min.x).min(max.x);
    if v.y < min.y {
        v.x = min.y;
    }
    if v.y > max.y {
        v.x = max.y;
    }
    v.z = v.z.max(min.z).min(max.z);
    ground_distance_squared(v, p) <= radius * radius
}

/// The circle slide of the sphere and cylinder adjusts: move along the
/// perpendicular of the advance until the circles stop overlapping, halved.
fn round_adjust(centre: Vec3, radius: f32, s: &DynamicSphere, flat: bool) -> Vec3 {
    let r = s.radius + radius;
    if (s.position - centre).length_squared() >= r * r {
        return Vec3::ZERO;
    }
    let mut advance = s.position - s.last;
    if flat {
        advance.z = 0.0;
    }
    let c = advance.cross(Vec3::Z);
    let sum = -c.dot(s.position - centre);
    let mul = r * r - (s.position - centre).length_squared();
    if sum * sum - 4.0 * mul <= 0.0 {
        return Vec3::ZERO;
    }
    let sq = (sum * sum - 4.0 * mul).sqrt();
    let (t1, t2) = ((-sum - sq) * 0.5, (-sum + sq) * 0.5);
    if t1.abs() <= t2.abs() { REDUCE_MOVE * t1 * c } else { REDUCE_MOVE * t2 * c }
}

/// `CActorInstance::__AdjustCollisionMovement` and `IsBlockObject` for a
/// single body sphere: slides `movement` round every shape it would enter,
/// capping the slide at `max_slide` (`gc_fActorSlideMoveSpeed`), then reports
/// whether the adjusted move still collides (`BlockMovement`).
pub fn adjust_movement(shapes: &[&StaticCollision], body: DynamicSphere, max_slide: f32) -> (Vec3, bool) {
    let mut movement = body.position - body.last;
    if movement == Vec3::ZERO {
        return (movement, false);
    }
    let mut collided = false;
    let mut sphere = body;
    for shape in shapes {
        if !shape.movement_collides(&sphere) {
            continue;
        }
        collided = true;
        if movement.length() > max_slide {
            movement *= max_slide / movement.length();
            sphere.position = sphere.last + movement;
        }
        let delta = shape.movement_adjust(&sphere);
        movement += delta;
        sphere.position += delta;
        if delta != Vec3::ZERO && movement.length() > max_slide {
            movement *= max_slide / movement.length();
            sphere.position = sphere.last + movement;
        }
    }
    if !collided {
        return (movement, false);
    }
    let blocked = shapes.iter().any(|shape| shape.movement_collides(&sphere));
    (movement, blocked)
}

#[cfg(test)]
mod tests {
    use super::*;
    use m2_assets::race::{MotionAttack, MotionHit};

    #[test]
    fn segments_find_their_closest_points() {
        // Crossing segments: the mirrored sign of `rb` puts `t` at −0.5, so B
        // is clamped to its start (the true closest point is (0, 0, 5)).
        let (a, b) = intersect_line_segments(
            Vec3::new(-10.0, 0.0, 0.0),
            Vec3::new(10.0, 0.0, 0.0),
            Vec3::new(0.0, -10.0, 5.0),
            Vec3::new(0.0, 10.0, 5.0),
        );
        assert!((a - Vec3::ZERO).length() < 1e-4);
        assert!((b - Vec3::new(0.0, -10.0, 5.0)).length() < 1e-4);
        // Past the ends: clamped.
        let (a, b) = intersect_line_segments(Vec3::ZERO, Vec3::X, Vec3::new(5.0, 1.0, 0.0), Vec3::new(5.0, 2.0, 0.0));
        assert_eq!(a, Vec3::X);
        assert_eq!(b, Vec3::new(5.0, 1.0, 0.0));
    }

    #[test]
    fn spheres_and_cylinders_collide_along_their_sweeps() {
        let moving = DynamicSphere { last: Vec3::new(-100.0, 0.0, 0.0), position: Vec3::new(100.0, 0.0, 0.0), radius: 10.0 };
        let still = DynamicSphere::still(Vec3::new(0.0, 15.0, 0.0), 10.0);
        assert!(sphere_vs_sphere(&moving, &still));
        let far = DynamicSphere::still(Vec3::new(0.0, 25.0, 0.0), 4.0);
        assert!(!sphere_vs_sphere(&moving, &far));
        // Height is ignored by the Z-cylinder test.
        let high = DynamicSphere::still(Vec3::new(0.0, 15.0, 500.0), 10.0);
        assert!(!sphere_vs_sphere(&moving, &high));
        assert!(zcylinder_vs_zcylinder(&moving, &high));
    }

    fn sweep_attack() -> MotionAttackData {
        MotionAttackData {
            attack: MotionAttack { hitting_type: HIT_TYPE_GOOD, invisible_time: 0.2, ..Default::default() },
            motion_type: MOTION_TYPE_COMBO,
            hits: vec![MotionHit {
                start: 0.1,
                end: 0.3,
                // A swing in front of the actor (north is −y in Metin2 space).
                positions: vec![(0.2, [-80.0, -100.0, 100.0], [80.0, -100.0, 100.0])],
            }],
        }
    }

    #[test]
    fn a_swing_hits_a_victim_in_front_once() {
        let attack = sweep_attack();
        let mut tracker = HitTracker::default();
        let victim = [DynamicSphere::still(Vec3::new(0.0, -110.0, 80.0), 40.0)];
        // Before the sample time nothing is swept.
        assert!(!normal_attack_hit(&attack, &mut tracker, 0.1, 0.016, Vec3::ZERO, 0.0, 1.0, 7, &victim, 0.0));
        assert!(normal_attack_hit(&attack, &mut tracker, 0.21, 0.016, Vec3::ZERO, 0.0, 1.0, 7, &victim, 0.0));
        // A combo swing never hits the same victim twice.
        assert!(!normal_attack_hit(&attack, &mut tracker, 0.21, 0.016, Vec3::ZERO, 0.0, 1.0, 7, &victim, 5.0));
        // Facing away (180°) the sweep is behind the actor.
        let mut tracker = HitTracker::default();
        assert!(!normal_attack_hit(&attack, &mut tracker, 0.21, 0.016, Vec3::ZERO, 180.0, 1.0, 7, &victim, 0.0));
    }

    #[test]
    fn a_skill_typed_combo_without_hit_limit_still_hits_its_first_victim() {
        // `combo_01.msa` of the assassin daggers: `MotionType 2`, no
        // `HitLimitCount`.
        let mut attack = sweep_attack();
        attack.motion_type = 2;
        attack.attack.hit_limit_count = 0;
        let mut tracker = HitTracker::default();
        let victim = [DynamicSphere::still(Vec3::new(0.0, -110.0, 80.0), 40.0)];
        assert!(normal_attack_hit(&attack, &mut tracker, 0.21, 0.016, Vec3::ZERO, 0.0, 1.0, 7, &victim, 0.0));
        // A second victim of the same window is over the limit of 0.
        assert!(!normal_attack_hit(&attack, &mut tracker, 0.21, 0.016, Vec3::ZERO, 0.0, 1.0, 8, &victim, 0.0));
    }

    #[test]
    fn splash_spheres_turn_with_the_actor() {
        let splash = MotionSplash {
            attack: MotionAttack { hitting_type: HIT_TYPE_GREAT, ..Default::default() },
            spheres: vec![(50.0, [0.0, 100.0, 0.0])],
            enable_hit_process: true,
        };
        let mut area = SplashArea::new(&splash, Vec3::ZERO, 0.0, 0.0, 1.0, 0);
        // rotation 0: radian = 630° -> sin −1, cos 0: (0·−1 + 100·0, 0·0 − 100·−1) = (0, 100).
        assert!((area.spheres[0].position - Vec3::new(0.0, 100.0, 0.0)).length() < 1e-3);
        assert!(area.hit(3, &[DynamicSphere::still(Vec3::new(0.0, 120.0, 0.0), 30.0)], 0.5, None));
        assert!(!area.hit(3, &[DynamicSphere::still(Vec3::new(0.0, 120.0, 0.0), 30.0)], 0.5, None));
        assert!(!area.hit(4, &[DynamicSphere::still(Vec3::new(0.0, 120.0, 0.0), 30.0)], 2.0, None));
    }

    #[test]
    fn walking_into_a_cylinder_slides_or_blocks() {
        let tree = StaticCollision::Cylinder { position: Vec3::new(0.0, 0.0, 0.0), radius: 50.0, height: 300.0 };
        // Head-on: the slide cannot get round, so it blocks.
        let body = DynamicSphere { last: Vec3::new(-80.0, 0.0, 50.0), position: Vec3::new(-60.0, 0.0, 50.0), radius: 30.0 };
        let (_, blocked) = adjust_movement(&[&tree], body, 30.0);
        assert!(blocked);
        // Moving away is free.
        let away = DynamicSphere { last: Vec3::new(-60.0, 0.0, 50.0), position: Vec3::new(-80.0, 0.0, 50.0), radius: 30.0 };
        assert_eq!(adjust_movement(&[&tree], away, 30.0), (Vec3::new(-20.0, 0.0, 0.0), false));
        // Far away nothing happens.
        let free = DynamicSphere { last: Vec3::new(-500.0, 0.0, 50.0), position: Vec3::new(-480.0, 0.0, 50.0), radius: 30.0 };
        assert!(!adjust_movement(&[&tree], free, 30.0).1);
    }

    #[test]
    fn actors_block_each_other_only_when_closing_in() {
        let other = [DynamicSphere::still(Vec3::new(0.0, 0.0, 50.0), 40.0)];
        let closer = [DynamicSphere { last: Vec3::new(-90.0, 0.0, 50.0), position: Vec3::new(-70.0, 0.0, 50.0), radius: 40.0 }];
        assert!(actor_blocks(&closer, &other));
        let leaving = [DynamicSphere { last: Vec3::new(-70.0, 0.0, 50.0), position: Vec3::new(-90.0, 0.0, 50.0), radius: 40.0 }];
        assert!(!actor_blocks(&leaving, &other));
    }

    #[test]
    fn row_major_transforms_rotate_like_d3d() {
        // A 90° yaw around z in row-vector form: x' = -y? D3DXMatrixRotationZ(90°)
        // rows: (c, s, 0), (−s, c, 0): (1,0,0)·M = (c, s, 0) = (0, 1, 0).
        let m = [0.0, 1.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 10.0, 20.0, 30.0, 1.0];
        let t = D3dTransform::from_row_major(&m);
        let p = t.apply(Vec3::X);
        assert!((p - Vec3::new(10.0, 21.0, 30.0)).length() < 1e-5);
    }
}
