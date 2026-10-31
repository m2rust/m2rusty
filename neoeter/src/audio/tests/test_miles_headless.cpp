// Headless audio characterization (TA.5).
//
// The client must be able to boot and drive its audio subsystem on machines
// without a sound card (CI containers, servers). OPENMT2_AUDIO_BACKEND=null
// pins miniaudio to ma_backend_null; this test proves the whole MilesLib shim
// works on that backend instead of relying on the host device probe.
//
// Tests rely on assert() even in Release builds.
#ifdef NDEBUG
#undef NDEBUG
#endif

#include <Neoeter/Audio/StdAfx.h>
#include <Neoeter/Audio/MSSCompat.h>
#include <Neoeter/Audio/SoundManager.h>

#include <cassert>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <vector>

namespace {

std::vector<unsigned char> make_mono_wav(unsigned sampleRate,
                                         unsigned frameCount)
{
    const unsigned dataSize = frameCount * 2;
    std::vector<unsigned char> wav(44 + dataSize, 0);

    auto put32 = [&wav](size_t offset, unsigned value) {
        std::memcpy(wav.data() + offset, &value, sizeof(value));
    };
    auto put16 = [&wav](size_t offset, unsigned short value) {
        std::memcpy(wav.data() + offset, &value, sizeof(value));
    };

    std::memcpy(wav.data() + 0, "RIFF", 4);
    put32(4, 36 + dataSize);
    std::memcpy(wav.data() + 8, "WAVE", 4);
    std::memcpy(wav.data() + 12, "fmt ", 4);
    put32(16, 16);
    put16(20, 1); // PCM
    put16(22, 1); // mono
    put32(24, sampleRate);
    put32(28, sampleRate * 2);
    put16(32, 2);
    put16(34, 16);
    std::memcpy(wav.data() + 36, "data", 4);
    put32(40, dataSize);
    return wav;
}

// The engine is a process-wide singleton, so this must run before anything
// else touches MilesLib.
void test_engine_uses_null_backend()
{
    // The environment contract: tests must opt into the null backend.
    assert(MilesCompatInternal::UseNullAudioBackend());

    ma_engine* engine = MilesCompatInternal::GetEngine();
    assert(engine != nullptr);

    ma_device* device = ma_engine_get_device(engine);
    assert(device != nullptr);

    ma_context* context = ma_device_get_context(device);
    assert(context != nullptr);

    // OPENMT2_AUDIO_BACKEND=null from the test environment, or the explicit
    // setenv() in main(), must select the null backend. Any host backend here
    // would mean the test is silently depending on a sound device.
    assert(context->backend == ma_backend_null);
    assert(ma_engine_get_sample_rate(engine) == 44100);
    assert(ma_engine_get_channels(engine) == 2);

    std::printf("[PASS] test_engine_uses_null_backend\n");
}

void test_3d_sample_lifecycle_without_device()
{
    ma_engine* engine = MilesCompatInternal::GetEngine();
    assert(engine != nullptr);

    H3DPOBJECT listener = AIL_open_3D_listener(0);
    assert(listener != nullptr);
    AIL_set_3D_position(listener, 1.0f, 2.0f, 3.0f);
    AIL_close_3D_listener(listener);

    std::vector<unsigned char> wav = make_mono_wav(44100, 64);

    H3DSAMPLE sample = AIL_allocate_3D_sample_handle(0);
    assert(sample != nullptr);
    assert(AIL_set_3D_sample_file(sample, wav.data()) != 0);
    assert(sample->sound_initialized);

    AIL_set_3D_position(sample, 4.0f, 2.0f, -8.0f);
    ma_vec3f samplePos = ma_sound_get_position(&sample->sound);
    assert(std::fabs(samplePos.x - 4.0f) < 1e-5f);
    assert(std::fabs(samplePos.y - 2.0f) < 1e-5f);
    assert(std::fabs(samplePos.z + 8.0f) < 1e-5f);

    AIL_release_3D_sample_handle(sample);
}

void test_sound_manager_lifecycle_without_device()
{
    CSoundManager soundMgr;
    assert(soundMgr.Create() == TRUE);

    soundMgr.SetSoundVolume(0.75f);
    soundMgr.SetMusicVolume(0.5f);
    assert(std::fabs(soundMgr.GetSoundVolume() - 0.75f) < 1e-4f);
    assert(std::fabs(soundMgr.GetMusicVolume() - 0.5f) < 1e-4f);

    soundMgr.SetPosition(10.0f, 20.0f, 0.0f);
    soundMgr.SetDirection(0.0f, 1.0f, 0.0f, 0.0f, 0.0f, 1.0f);

    // Fades advance through the null device without blocking or asserting.
    soundMgr.FadeOutMusic("bgm/login_window.mp3", 0.2f);
    for (int i = 0; i < 5; ++i)
        soundMgr.Update();

    soundMgr.Destroy();
    std::printf("[PASS] test_sound_manager_lifecycle_without_device\n");
}

} // namespace

int main()
{
    // Direct runs (outside CTest) stay headless too; the CMake test property
    // sets the same variable and wins because overwrite is disabled.
    setenv("OPENMT2_AUDIO_BACKEND", "null", 0);

    test_engine_uses_null_backend();
    test_3d_sample_lifecycle_without_device();
    test_sound_manager_lifecycle_without_device();

    std::printf("All MilesLib headless tests passed!\n");
    return 0;
}
