// Tests rely on assert() even in Release builds.
#ifdef NDEBUG
#undef NDEBUG
#endif

#include <Neoeter/Audio/StdAfx.h>
#include <Neoeter/Audio/SoundManager.h>
#include <Neoeter/Audio/SoundData.h>
#include <Neoeter/Audio/MSSCompat.h>
#include <Neoeter/Audio/Type.h>
#include <Neoeter/IO/EterPackManager.h>

#include <cassert>
#include <cmath>
#include <cstdio>
#include <cstring>
#include <vector>
#include <unistd.h>

namespace {

void test_nsound_type_and_serialization()
{
    NSound::SetResultString("MilesTestOK");
    assert(std::strcmp(NSound::GetResultString(), "MilesTestOK") == 0);

    NSound::TSoundDataVector dataVec(2);
    dataVec[0].fTime = 0.0f;
    dataVec[0].strSoundFileName = "sound/pc/warrior/swing1.wav";
    dataVec[1].fTime = 0.5f;
    dataVec[1].strSoundFileName = "sound/pc/warrior/swing2.wav";

    NSound::TSoundInstanceVector instVec;
    NSound::DataToInstance(dataVec, &instVec);
    assert(instVec.size() == 2);
    assert(instVec[0].dwFrame == 0);
    assert(instVec[1].dwFrame == 29 || instVec[1].dwFrame == 30);
    assert(instVec[1].strSoundFileName == "sound/pc/warrior/swing2.wav");

    const char* tmpPath = "/tmp/test_mileslib_soundinfo.mss";
    bool saved = NSound::SaveSoundInformationPiece(tmpPath, dataVec);
    assert(saved);
    unlink(tmpPath);

    std::printf("[PASS] test_nsound_type_and_serialization\n");
}

void test_3d_spatialization_configuration()
{
    ma_engine* engine = MilesCompatInternal::GetEngine();
    assert(engine != nullptr);

    // Listener updates must reach the miniaudio engine, otherwise positional
    // sounds are evaluated against a listener stuck at the world origin.
    H3DPOBJECT listener = AIL_open_3D_listener(0);
    assert(listener != nullptr);

    AIL_set_3D_position(listener, 10.0f, 20.0f, -30.0f);
    ma_vec3f listenerPos = ma_engine_listener_get_position(engine, 0);
    assert(std::fabs(listenerPos.x - 10.0f) < 1e-5f);
    assert(std::fabs(listenerPos.y - 20.0f) < 1e-5f);
    assert(std::fabs(listenerPos.z + 30.0f) < 1e-5f);

    AIL_set_3D_orientation(listener, 0.0f, 0.0f, -1.0f, 0.0f, 1.0f, 0.0f);
    ma_vec3f listenerDir = ma_engine_listener_get_direction(engine, 0);
    ma_vec3f listenerUp = ma_engine_listener_get_world_up(engine, 0);
    assert(std::fabs(listenerDir.z + 1.0f) < 1e-5f);
    assert(std::fabs(listenerUp.y - 1.0f) < 1e-5f);

    AIL_set_3D_velocity(listener, 1.0f, 2.0f, 3.0f, 4.0f);
    ma_vec3f listenerVel = ma_engine_listener_get_velocity(engine, 0);
    assert(std::fabs(listenerVel.x - 1.0f) < 1e-5f);
    assert(std::fabs(listenerVel.y - 2.0f) < 1e-5f);
    assert(std::fabs(listenerVel.z - 3.0f) < 1e-5f);

    AIL_close_3D_listener(listener);

    // Build a valid mono PCM WAV and verify the 3D sample spatializer setup.
    const ma_uint32 sampleRate = 44100;
    const ma_uint32 frameCount = 64;
    const ma_uint32 dataSize = frameCount * 2;
    std::vector<unsigned char> wav(44 + dataSize, 0);

    auto put32 = [&wav](size_t offset, uint32_t value) {
        std::memcpy(wav.data() + offset, &value, sizeof(value));
    };
    auto put16 = [&wav](size_t offset, uint16_t value) {
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

    H3DSAMPLE sample = AIL_allocate_3D_sample_handle(0);
    assert(sample != nullptr);
    assert(AIL_set_3D_sample_file(sample, wav.data()) != 0);
    assert(sample->sound_initialized);
    assert(std::fabs(ma_sound_get_min_distance(&sample->sound) - 1.0f) < 1e-5f);
    assert(std::fabs(ma_sound_get_max_distance(&sample->sound) - 100.0f) < 1e-5f);

    AIL_set_3D_position(sample, 4.0f, 2.0f, -8.0f);
    ma_vec3f samplePos = ma_sound_get_position(&sample->sound);
    assert(std::fabs(samplePos.x - 4.0f) < 1e-5f);
    assert(std::fabs(samplePos.y - 2.0f) < 1e-5f);
    assert(std::fabs(samplePos.z + 8.0f) < 1e-5f);

    AIL_release_3D_sample_handle(sample);

    std::printf("[PASS] test_3d_spatialization_configuration\n");
}

void test_sound_manager_lifecycle_and_playback()
{
    // Create a dummy WAV file on disk for 2D/3D sample loading
    const char* wavPath = "/tmp/test_mileslib_sample.wav";
    {
        FILE* fp = std::fopen(wavPath, "wb");
        assert(fp != nullptr);
        const char dummyWav[64] = {'R', 'I', 'F', 'F', 36, 0, 0, 0, 'W', 'A', 'V', 'E', 'f', 'm', 't', ' '};
        std::fwrite(dummyWav, 1, sizeof(dummyWav), fp);
        std::fclose(fp);
    }

    CEterPackManager packMgr;
    CSoundManager soundMgr;

    BOOL created = soundMgr.Create();
    assert(created == TRUE);

    // Volume & scale controls
    soundMgr.SetSoundScale(250.0f);
    assert(std::fabs(soundMgr.GetSoundScale() - 250.0f) < 1e-4f);

    soundMgr.SetSoundVolume(0.8f);
    assert(std::fabs(soundMgr.GetSoundVolume() - 0.8f) < 1e-4f);

    soundMgr.SetMusicVolume(0.6f);
    assert(std::fabs(soundMgr.GetMusicVolume() - 0.6f) < 1e-4f);

    // SaveVolume & RestoreVolume
    soundMgr.SaveVolume();
    assert(std::fabs(soundMgr.GetSoundVolume() - 0.0f) < 1e-4f);
    assert(std::fabs(soundMgr.GetMusicVolume() - 0.0f) < 1e-4f);

    soundMgr.RestoreVolume();
    assert(std::fabs(soundMgr.GetSoundVolume() - 0.8f) < 1e-4f);
    assert(std::fabs(soundMgr.GetMusicVolume() - 0.6f) < 1e-4f);

    // 2D & 3D sound playback
    soundMgr.SetPosition(100.0f, 200.0f, 0.0f);
    soundMgr.SetDirection(0.0f, 1.0f, 0.0f, 0.0f, 0.0f, 1.0f);
    soundMgr.PlaySound2D(wavPath);
    soundMgr.PlaySound3D(120.0f, 220.0f, 0.0f, wavPath, 1);

    int ambIdx = soundMgr.PlayAmbienceSound3D(150.0f, 250.0f, 0.0f, wavPath, 1);
    assert(ambIdx >= 0);
    soundMgr.SetSoundVolume3D(ambIdx, 0.5f);
    soundMgr.StopSound3D(ambIdx);
    soundMgr.StopAllSound3D();

    // Music streaming & fade transitions
    soundMgr.FadeInMusic("bgm/login_window.mp3", 0.2f);
    for (int i = 0; i < 5; ++i)
        soundMgr.Update();

    soundMgr.FadeOutMusic("bgm/login_window.mp3", 0.3f);
    for (int i = 0; i < 5; ++i)
        soundMgr.Update();

    soundMgr.Destroy();
    unlink(wavPath);

    std::printf("[PASS] test_sound_manager_lifecycle_and_playback\n");
}

} // namespace

int main()
{
    test_nsound_type_and_serialization();
    test_3d_spatialization_configuration();
    test_sound_manager_lifecycle_and_playback();

    std::printf("All MilesLib smoke tests passed!\n");
    return 0;
}
