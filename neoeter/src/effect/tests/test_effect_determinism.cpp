// Deterministic effect updates (TA.5).
//
// Particle physics in EffectLib is driven by decorator chains over elapsed
// time. This test pins the result of a fixed-step update against hand-computed
// reference values and proves that seeded RNG use is reproducible, so a future
// refactor cannot silently change gameplay effects.
//
// Tests rely on assert() even in Release builds.
#ifdef NDEBUG
#undef NDEBUG
#endif

#include <Neoeter/Effect/EmitterProperty.h>
#include <Neoeter/Effect/ParticleInstance.h>
#include <Neoeter/Effect/ParticleProperty.h>
#include <Neoeter/Effect/ParticleSystemData.h>
#include <Neoeter/Effect/ParticleSystemInstance.h>
#include <Neoeter/Effect/StdAfx.h>
#include <Neoeter/Effect/Type.h>

#include <Neoeter/Platform/Base/Random.h>
#include <Neoeter/Platform/Base/Timer.h>

#include <cassert>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>

namespace {

// EffectLib only advances particles for texture-animation frames that have an
// image. A null image is enough to create frame 0 without touching the GPU.
void add_placeholder_frame(CParticleProperty* property)
{
    property->m_ImageVector.push_back(nullptr);
}

void add_constant_float(TTimeEventTableFloat* table, float value)
{
    InsertItemTimeEvent(table, 0.0f, value);
}

void add_constant_color(TTimeEventTableColor* table, DWORD value)
{
    DWORDCOLOR color;
    color.m_dwColor = value;
    InsertItemTimeEvent(table, 0.0f, color);
}

// Two-key tables keep the decorator alive for the whole particle lifetime, so
// the test exercises interpolation rather than the one-shot "remove me" path.
void add_spanning_float(TTimeEventTableFloat* table, float value)
{
    InsertItemTimeEvent(table, 0.0f, value);
    InsertItemTimeEvent(table, 100.0f, value);
}

void add_spanning_color(TTimeEventTableColor* table, DWORD value)
{
    DWORDCOLOR color;
    color.m_dwColor = value;
    InsertItemTimeEvent(table, 0.0f, color);
    InsertItemTimeEvent(table, 100.0f, color);
}

void configure_deterministic_data(CParticleSystemData* data)
{
    data->Clear();

    CEmitterProperty* emitter = data->GetEmitterPropertyPointer();
    emitter->m_dwMaxEmissionCount = 100;
    emitter->m_fCycleLength = 1000.0f;
    emitter->m_bCycleLoopFlag = TRUE;
    emitter->m_iLoopCount = 0;
    emitter->m_byEmitterShape = CEmitterProperty::EMITTER_SHAPE_POINT;
    emitter->m_byEmitterAdvancedType = CEmitterProperty::EMITTER_ADVANCED_TYPE_FREE;
    emitter->m_v3EmittingDirection = D3DXVECTOR3(0.0f, 0.0f, 0.0f);

    add_constant_float(&emitter->m_TimeEventEmissionCountPerSecond, 2.0f);
    add_constant_float(&emitter->m_TimeEventLifeTime, 10.0f);
    add_constant_float(&emitter->m_TimeEventSizeX, 1.0f);
    add_constant_float(&emitter->m_TimeEventSizeY, 1.0f);
    add_constant_float(&emitter->m_TimeEventEmittingVelocity, 2.0f);
    add_constant_float(&emitter->m_TimeEventEmittingDirectionX, 3.0f);
    add_constant_float(&emitter->m_TimeEventEmittingDirectionY, 0.0f);
    add_constant_float(&emitter->m_TimeEventEmittingDirectionZ, 0.0f);

    CParticleProperty* particle = data->GetParticlePropertyPointer();
    particle->m_byBillboardType = BILLBOARD_TYPE_ALL;
    particle->m_byRotationType = CParticleProperty::ROTATION_TYPE_NONE;
    particle->m_fRotationSpeed = 0.0f;

    add_constant_float(&particle->m_TimeEventGravity, 9.8f);
    add_constant_float(&particle->m_TimeEventAirResistance, 0.1f);
    add_spanning_float(&particle->m_TimeEventScaleX, 1.0f);
    add_spanning_float(&particle->m_TimeEventScaleY, 1.0f);
    add_spanning_color(&particle->m_TimeEventColor, 0xffffffff);

    add_placeholder_frame(particle);
}

class TestParticleSystem : public CParticleSystemInstance
{
public:
    std::size_t ParticleCount() const
    {
        std::size_t count = 0;
        for (const auto& frame : m_ParticleInstanceListVector)
            count += frame.size();
        return count;
    }

    CParticleInstance* FirstParticle()
    {
        if (m_ParticleInstanceListVector.empty() ||
            m_ParticleInstanceListVector[0].empty())
            return nullptr;
        return m_ParticleInstanceListVector[0].front();
    }
};

void assert_close(float actual, float expected, float tolerance = 1e-4f)
{
    assert(std::fabs(actual - expected) <= tolerance);
}

// Reference integration: gravity is applied first, then air resistance, then
// the position is advanced by the new velocity (CParticleInstance::Update).
void reference_step(D3DXVECTOR3* position, D3DXVECTOR3* velocity,
                    float dt, float gravity, float airResistance)
{
    velocity->z -= gravity * dt;
    velocity->x *= 1.0f - airResistance;
    velocity->y *= 1.0f - airResistance;
    velocity->z *= 1.0f - airResistance;
    position->x += velocity->x * dt;
    position->y += velocity->y * dt;
    position->z += velocity->z * dt;
}

void test_deterministic_particle_update()
{
    CParticleSystemData data;
    configure_deterministic_data(&data);

    TestParticleSystem system;
    system.Initialize();
    system.SetDataPointer(&data);

    const float dt = 0.5f;
    const int updates = 4;

    for (int i = 0; i < updates; ++i)
        assert(system.Update(dt));

    // 2 particles per second at 0.5 s per update -> exactly one per update.
    assert(system.GetEmissionCount() == static_cast<DWORD>(updates));
    assert(system.ParticleCount() == static_cast<std::size_t>(updates));

    CParticleInstance* first = system.FirstParticle();
    assert(first != nullptr);

    // The first particle is spawned during update 1 and stepped updates-1 times.
    D3DXVECTOR3 expectedPosition(0.0f, 0.0f, 0.0f);
    D3DXVECTOR3 expectedVelocity(6.0f, 0.0f, 0.0f); // 3 * emitting velocity 2
    for (int i = 0; i < updates - 1; ++i)
        reference_step(&expectedPosition, &expectedVelocity, dt, 9.8f, 0.1f);

    // Hand-computed values for this exact sequence.
    assert_close(expectedPosition.x, 7.317f, 1e-3f);
    assert_close(expectedPosition.y, 0.0f, 1e-3f);
    assert_close(expectedPosition.z, -12.3699f, 1e-3f);
    assert_close(expectedVelocity.x, 4.374f, 1e-3f);
    assert_close(expectedVelocity.z, -11.9511f, 1e-3f);

    assert_close(first->GetPosition().x, expectedPosition.x);
    assert_close(first->GetPosition().y, expectedPosition.y);
    assert_close(first->GetPosition().z, expectedPosition.z);
    assert_close(first->GetVelocity().x, expectedVelocity.x);
    assert_close(first->GetVelocity().y, expectedVelocity.y);
    assert_close(first->GetVelocity().z, expectedVelocity.z);
    assert_close(first->GetLifeTime(), 10.0f - (updates - 1) * dt);

    // A second run must reproduce the same bits, not just an approximate pose.
    CParticleSystemData dataCopy;
    configure_deterministic_data(&dataCopy);
    TestParticleSystem second;
    second.Initialize();
    second.SetDataPointer(&dataCopy);
    for (int i = 0; i < updates; ++i)
        assert(second.Update(dt));

    CParticleInstance* secondParticle = second.FirstParticle();
    assert(secondParticle != nullptr);
    assert(std::memcmp(&first->GetPosition(), &secondParticle->GetPosition(),
                       sizeof(D3DXVECTOR3)) == 0);
    assert(std::memcmp(&first->GetVelocity(), &secondParticle->GetVelocity(),
                       sizeof(D3DXVECTOR3)) == 0);

    system.Destroy();
    second.Destroy();

    std::printf("[PASS] test_deterministic_particle_update\n");
}

void configure_random_data(CParticleSystemData* data)
{
    data->Clear();

    CEmitterProperty* emitter = data->GetEmitterPropertyPointer();
    emitter->m_dwMaxEmissionCount = 10;
    emitter->m_fCycleLength = 1000.0f;
    emitter->m_byEmitterShape = CEmitterProperty::EMITTER_SHAPE_POINT;
    emitter->m_byEmitterAdvancedType =
        CEmitterProperty::EMITTER_ADVANCED_TYPE_OUTER;
    // A zero emitting direction keeps the random source in the advanced-type
    // branch only (three frandom() calls for the initial velocity).
    emitter->m_v3EmittingDirection = D3DXVECTOR3(0.0f, 0.0f, 0.0f);

    add_constant_float(&emitter->m_TimeEventEmissionCountPerSecond, 1.0f);
    add_constant_float(&emitter->m_TimeEventLifeTime, 10.0f);
    add_constant_float(&emitter->m_TimeEventSizeX, 1.0f);
    add_constant_float(&emitter->m_TimeEventSizeY, 1.0f);
    add_constant_float(&emitter->m_TimeEventEmittingVelocity, 1.0f);
    add_constant_float(&emitter->m_TimeEventEmittingDirectionX, 0.0f);
    add_constant_float(&emitter->m_TimeEventEmittingDirectionY, 0.0f);
    add_constant_float(&emitter->m_TimeEventEmittingDirectionZ, 0.0f);

    CParticleProperty* particle = data->GetParticlePropertyPointer();
    particle->m_byBillboardType = BILLBOARD_TYPE_ALL;
    particle->m_byRotationType = CParticleProperty::ROTATION_TYPE_NONE;
    add_spanning_float(&particle->m_TimeEventScaleX, 1.0f);
    add_spanning_float(&particle->m_TimeEventScaleY, 1.0f);
    add_spanning_color(&particle->m_TimeEventColor, 0xffffffff);
    add_placeholder_frame(particle);
}

void test_seeded_emission_is_reproducible()
{
    auto run = []() -> D3DXVECTOR3 {
        CParticleSystemData data;
        configure_random_data(&data);

        TestParticleSystem system;
        system.Initialize();
        system.SetDataPointer(&data);

        // The seed must be applied immediately before the first particle is
        // emitted; everything before this point is deterministic setup.
        srandom(1234);
        assert(system.Update(1.0f));

        CParticleInstance* particle = system.FirstParticle();
        assert(particle != nullptr);

        D3DXVECTOR3 velocity = particle->GetVelocity();
        system.Destroy();
        return velocity;
    };

    D3DXVECTOR3 first = run();
    D3DXVECTOR3 second = run();

    // The random branch must actually have run ...
    assert(first.x != 0.0f || first.y != 0.0f || first.z != 0.0f);
    // ... and a fixed seed must reproduce it bit for bit.
    assert(std::memcmp(&first, &second, sizeof(D3DXVECTOR3)) == 0);

    std::printf("[PASS] test_seeded_emission_is_reproducible\n");
}

} // namespace

int main()
{
    // CEffectElementBaseInstance queries the global timer when data is bound.
    CTimer timer;

    test_deterministic_particle_update();
    test_seeded_emission_is_reproducible();

    CParticleSystemInstance::DestroySystem();
    CParticleSystemData::DestroySystem();
    CParticleInstance::DestroySystem();
    // Particle textures allocate from this pool; returning them to the free
    // list is not enough, the pool itself must be destroyed before exit.
    CGraphicImageInstance::DestroySystem();

    std::printf("All EffectLib determinism tests passed!\n");
    return 0;
}
