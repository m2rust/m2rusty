//! Port of `neoeter/src/effect/tests/test_effect_determinism.cpp`, plus
//! manager and sound behaviour.

use std::sync::Arc;

use m2_effect::{EffectInstance, EffectManager, GlibcRandom, Mat4, NoMeshes};
use m2_formats::effect::{EffectData, ParticleSystemData, SoundInstance, TimeEvent};

fn constant(value: f32) -> Vec<TimeEvent<f32>> {
    vec![TimeEvent { time: 0.0, value }]
}

fn spanning(value: f32) -> Vec<TimeEvent<f32>> {
    vec![TimeEvent { time: 0.0, value }, TimeEvent { time: 100.0, value }]
}

fn base_data() -> ParticleSystemData {
    let mut d = ParticleSystemData::default();
    let e = &mut d.emitter;
    e.max_emission_count = 100;
    e.cycle_length = 1000.0;
    e.cycle_loop = 1;
    e.emission_count_per_second = constant(2.0);
    e.life_time = constant(10.0);
    e.size_x = constant(1.0);
    e.size_y = constant(1.0);
    e.emitting_velocity = constant(2.0);
    e.emitting_direction_x = constant(3.0);
    e.emitting_direction_y = constant(0.0);
    e.emitting_direction_z = constant(0.0);
    let p = &mut d.particle;
    p.billboard_type = 1;
    p.gravity = constant(9.8);
    p.air_resistance = constant(0.1);
    p.scale_x = spanning(1.0);
    p.scale_y = spanning(1.0);
    p.color = vec![TimeEvent { time: 0.0, value: 0xffffffff }, TimeEvent { time: 100.0, value: 0xffffffff }];
    p.textures = vec![b"placeholder".to_vec()];
    d
}

fn effect(d: ParticleSystemData) -> Arc<EffectData> {
    Arc::new(EffectData { particles: vec![d], ..EffectData::default() })
}

#[test]
fn deterministic_particle_update() {
    let mut inst = EffectInstance::new(effect(base_data()), &NoMeshes, 0.0);
    let mut rng = GlibcRandom::new(1);
    for _ in 0..4 {
        inst.update(0.5, &mut rng);
    }
    let ps = &inst.particle_systems()[0];
    assert_eq!(ps.emission_count(), 4);
    assert_eq!(ps.particles(0).len(), 4);
    let first = &ps.particles(0)[0];
    // Gravity first, then air resistance, then the position step.
    let (mut pos, mut vel) = ([0.0f32; 3], [6.0f32, 0.0, 0.0]);
    for _ in 0..3 {
        vel[2] -= 9.8 * 0.5;
        for v in &mut vel {
            *v *= 1.0 - 0.1;
        }
        for k in 0..3 {
            pos[k] += vel[k] * 0.5;
        }
    }
    assert!((pos[0] - 7.317).abs() < 1e-3 && (pos[2] + 12.3699).abs() < 1e-3);
    for k in 0..3 {
        assert!((first.position[k] - pos[k]).abs() < 1e-4);
        assert!((first.velocity[k] - vel[k]).abs() < 1e-4);
    }
    assert!((first.last_life_time - (10.0 - 3.0 * 0.5)).abs() < 1e-4);
}

#[test]
fn seeded_emission_is_reproducible() {
    let mut d = base_data();
    d.emitter.max_emission_count = 10;
    d.emitter.emitter_advanced_type = 1; // outer: random initial velocity
    d.emitter.emitting_velocity = constant(1.0);
    d.emitter.emitting_direction_x = constant(0.0);
    let run = |seed| {
        let mut inst = EffectInstance::new(effect(d.clone()), &NoMeshes, 0.0);
        inst.update(1.0, &mut GlibcRandom::new(seed));
        inst.particle_systems()[0].particles(0)[0].velocity
    };
    assert_eq!(run(1234), run(1234));
    assert_ne!(run(1234), [0.0; 3]);
    assert_ne!(run(1234), run(1235));
}

#[test]
fn stop_lets_particles_fade_and_the_effect_die() {
    let mut inst = EffectInstance::new(effect(base_data()), &NoMeshes, 0.0);
    let mut rng = GlibcRandom::new(1);
    for _ in 0..4 {
        inst.update(0.5, &mut rng);
    }
    inst.set_deactive();
    let count = inst.particle_systems()[0].emission_count();
    inst.update(0.5, &mut rng);
    // No new particles, the old ones keep flying.
    assert_eq!(inst.particle_systems()[0].emission_count(), count);
    assert!(inst.is_alive());
    for _ in 0..30 {
        inst.update(0.5, &mut rng);
    }
    assert_eq!(inst.particle_systems()[0].emission_count(), 0);
    // The cycle (1000 s) has not ended, so the system itself lives on.
    assert!(inst.is_alive());
}

#[test]
fn restart_starts_over_and_resets_the_matrix() {
    let mut inst = EffectInstance::new(effect(base_data()), &NoMeshes, 0.0);
    let mut rng = GlibcRandom::new(1);
    let mut m = Mat4::IDENTITY;
    m.set_translation([1.0, 2.0, 3.0]);
    inst.set_global_matrix(m);
    for _ in 0..3 {
        inst.update(0.5, &mut rng);
    }
    inst.restart(&NoMeshes, 1.5);
    assert_eq!(inst.particle_systems()[0].emission_count(), 0);
    assert_eq!(*inst.global_matrix(), Mat4::IDENTITY);
}

#[test]
fn sounds_play_on_their_frame() {
    let data = EffectData {
        sounds: vec![SoundInstance { frame: 1, file_name: b"a.wav".to_vec() }],
        ..EffectData::default()
    };
    let mut inst = EffectInstance::new(Arc::new(data), &NoMeshes, 0.0);
    assert!(inst.update_sound().is_empty());
    assert_eq!(inst.update_sound().len(), 1);
    assert!(inst.update_sound().is_empty());
}

#[test]
fn manager_creates_updates_and_drops_dead_instances() {
    let mut d = base_data();
    d.emitter.cycle_length = 1.0;
    d.emitter.cycle_loop = 0;
    d.emitter.life_time = constant(0.5);
    let data = EffectData { particles: vec![d], ..EffectData::default() };
    let mut mgr = EffectManager::new();
    let id = mgr.register_effect(b"D:\\Effect\\Test.mse", data, false);
    assert_eq!(id, EffectManager::effect_id(b"d:/effect/test.mse"));
    let index = mgr.create_effect(id, [1.0, 2.0, 3.0], [0.0; 3], &NoMeshes, 0.0);
    assert!(mgr.is_alive_effect(index));
    let missing = mgr.create_effect(12345, [0.0; 3], [0.0; 3], &NoMeshes, 0.0);
    assert_ne!(index, missing);
    assert!(!mgr.is_alive_effect(missing));
    let mut rng = GlibcRandom::new(1);
    let mut now = 0.0;
    for _ in 0..20 {
        now += 0.25;
        mgr.update(now, &mut rng);
    }
    assert!(!mgr.is_alive_effect(index));
}
