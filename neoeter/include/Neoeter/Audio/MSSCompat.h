#pragma once

#include "Platform/Platform.h"
#include <miniaudio/miniaudio.h>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>

#ifndef AILCALLBACK
#define AILCALLBACK
#endif

typedef uint32_t U32;
typedef int32_t  S32;
typedef uint16_t U16;
typedef int16_t  S16;
typedef uint8_t  U8;
typedef int8_t   S8;

typedef uintptr_t HPROVIDER;
typedef int32_t   HPROENUM;

#define HPROENUM_FIRST 0
#define M3D_NOERR      0

#define FILE_READ_WITH_SIZE ((void*)(intptr_t)-1)

#define AILFILETYPE_UNKNOWN       0
#define AILFILETYPE_PCM_WAV       1
#define AILFILETYPE_ADPCM_WAV     2
#define AILFILETYPE_OTHER_WAV     3
#define AILFILETYPE_VOC           4
#define AILFILETYPE_MIDI          5
#define AILFILETYPE_XMIDI         6
#define AILFILETYPE_XMIDI_DLS     7
#define AILFILETYPE_XMIDI_MLS     8
#define AILFILETYPE_MPEG_L1_AUDIO 9
#define AILFILETYPE_MPEG_L2_AUDIO 10
#define AILFILETYPE_MPEG_L3_AUDIO 11

#define SMP_FREE          0x0001
#define SMP_DONE          0x0002
#define SMP_PLAYING       0x0004
#define SMP_STOPPED       0x0008
#define SMP_PLAYINGBUTRELEASED 0x0010

#define DIG_F_16BITS_MASK 1
#define DIG_F_STEREO_16   3

struct AILSOUNDINFO {
    S32 format;
    const void* data_ptr;
    U32 data_len;
    U32 rate;
    S32 bits;
    S32 channels;
    U32 samples;
    U32 block_size;
    const void* initial_ptr;
};

struct AIL_DigitalDriver {
    U32 frequency;
    S32 bits;
    S32 channels;
    U32 flags;
};
typedef AIL_DigitalDriver* HDIGDRIVER;

typedef U32 (AILCALLBACK *AIL_file_open_callback)(char const* filename, U32* file_handle);
typedef void (AILCALLBACK *AIL_file_close_callback)(U32 file_handle);
typedef S32 (AILCALLBACK *AIL_file_seek_callback)(U32 file_handle, S32 offset, U32 type);
typedef U32 (AILCALLBACK *AIL_file_read_callback)(U32 file_handle, void* buffer, U32 bytes);

namespace MilesCompatInternal {
    inline AIL_file_open_callback& OpenCb() {
        static AIL_file_open_callback s_cb = nullptr;
        return s_cb;
    }
    inline AIL_file_close_callback& CloseCb() {
        static AIL_file_close_callback s_cb = nullptr;
        return s_cb;
    }
    inline AIL_file_seek_callback& SeekCb() {
        static AIL_file_seek_callback s_cb = nullptr;
        return s_cb;
    }
    inline AIL_file_read_callback& ReadCb() {
        static AIL_file_read_callback s_cb = nullptr;
        return s_cb;
    }

    // Headless runs (CI, unit tests, dedicated servers) must not depend on a
    // sound card. Setting OPENMT2_AUDIO_BACKEND=null before the first engine
    // access pins miniaudio to ma_backend_null instead of probing the host
    // device list. The value is read once, when the singleton is created.
    inline bool UseNullAudioBackend() {
        const char* backend = std::getenv("OPENMT2_AUDIO_BACKEND");
        return backend != nullptr && std::strcmp(backend, "null") == 0;
    }

    struct EngineHolder {
        ma_context context;
        bool contextInitialized = false;
        ma_engine engine;
        bool initialized = false;

        EngineHolder() {
            ma_engine_config config = ma_engine_config_init();
            config.sampleRate = 44100;
            config.channels = 2;

            if (UseNullAudioBackend()) {
                ma_backend backends[1] = { ma_backend_null };
                ma_context_config contextConfig = ma_context_config_init();
                if (ma_context_init(backends, 1, &contextConfig, &context) == MA_SUCCESS) {
                    contextInitialized = true;
                    config.pContext = &context;
                }
            }

            if (ma_engine_init(&config, &engine) == MA_SUCCESS) {
                initialized = true;
            }
        }

        ~EngineHolder() {
            if (initialized) {
                ma_engine_uninit(&engine);
                initialized = false;
            }
            if (contextInitialized) {
                ma_context_uninit(&context);
                contextInitialized = false;
            }
        }
    };

    inline ma_engine* GetEngine() {
        static EngineHolder s_holder;
        return s_holder.initialized ? &s_holder.engine : nullptr;
    }

    // Positional audio is fed in "audio units". The client divides world
    // distances by CSoundManager::m_fSoundScale (200 by default), so one unit
    // is roughly one character height. Attenuation is therefore computed with
    // a 1-unit minimum distance so nearby sounds stay at full volume while
    // distant sounds fade out with the inverse-distance model.
    static constexpr float kMin3DDistance = 1.0f;
    static constexpr float kMax3DDistance = 100.0f;
    static constexpr float k3DRolloff = 1.0f;
}

struct AIL_Sample2D {
    HDIGDRIVER driver = nullptr;
    const void* data = nullptr;
    S32 size = 0;
    S32 loop_count = 1;
    U32 status = SMP_DONE;
    float volume = 1.0f;
    float pan = 0.5f;

    ma_decoder decoder;
    bool decoder_initialized = false;
    ma_sound sound;
    bool sound_initialized = false;

    void cleanup() {
        if (sound_initialized) {
            ma_sound_uninit(&sound);
            sound_initialized = false;
        }
        if (decoder_initialized) {
            ma_decoder_uninit(&decoder);
            decoder_initialized = false;
        }
    }

    ~AIL_Sample2D() {
        cleanup();
    }
};
typedef AIL_Sample2D* HSAMPLE;

struct AIL_Object3D {
    HPROVIDER provider = 0;
    bool is_listener = false;
    const void* data = nullptr;
    S32 size = 0;
    U32 loop_count = 1;
    U32 status = SMP_DONE;
    float volume = 1.0f;
    float x = 0.0f, y = 0.0f, z = 0.0f;
    float vx = 0.0f, vy = 0.0f, vz = 0.0f, vmag = 0.0f;
    float fx = 0.0f, fy = 0.0f, fz = 1.0f;
    float ux = 0.0f, uy = 1.0f, uz = 0.0f;
    S32 auto_update = 0;

    ma_decoder decoder;
    bool decoder_initialized = false;
    ma_sound sound;
    bool sound_initialized = false;

    void cleanup() {
        if (sound_initialized) {
            ma_sound_uninit(&sound);
            sound_initialized = false;
        }
        if (decoder_initialized) {
            ma_decoder_uninit(&decoder);
            decoder_initialized = false;
        }
    }

    ~AIL_Object3D() {
        cleanup();
    }
};
typedef AIL_Object3D* H3DPOBJECT;
typedef AIL_Object3D* H3DSAMPLE;

struct AIL_Stream {
    HDIGDRIVER driver = nullptr;
    char filename[256] = {};
    S32 loop_count = 1;
    S32 status = -1; // -1 = done/stopped, 0 = playing, 1 = paused
    float left_vol = 1.0f;
    float right_vol = 1.0f;

    ma_sound sound;
    bool sound_initialized = false;

    void cleanup() {
        if (sound_initialized) {
            ma_sound_uninit(&sound);
            sound_initialized = false;
        }
    }

    ~AIL_Stream() {
        cleanup();
    }
};
typedef AIL_Stream* HSTREAM;

inline char* AIL_set_redist_directory(const char* /*dir*/) {
    static char s_dir[] = "miles";
    return s_dir;
}

inline S32 AIL_startup() {
    MilesCompatInternal::GetEngine();
    return 1;
}

inline void AIL_shutdown() {
}

inline const char* AIL_last_error() {
    return "No error (miniaudio)";
}

inline void AIL_set_file_callbacks(
    AIL_file_open_callback opencb,
    AIL_file_close_callback closecb,
    AIL_file_seek_callback seekcb,
    AIL_file_read_callback readcb)
{
    MilesCompatInternal::OpenCb()  = opencb;
    MilesCompatInternal::CloseCb() = closecb;
    MilesCompatInternal::SeekCb()  = seekcb;
    MilesCompatInternal::ReadCb()  = readcb;
}

inline void* AIL_file_read(const char* filename, void* dest) {
    if (!filename || !*filename)
        return nullptr;

    const bool withSize = (dest == FILE_READ_WITH_SIZE);

    if (MilesCompatInternal::OpenCb() && MilesCompatInternal::CloseCb() &&
        MilesCompatInternal::SeekCb() && MilesCompatInternal::ReadCb())
    {
        U32 handle = 0;
        if (MilesCompatInternal::OpenCb()(filename, &handle)) {
            S32 endPos = MilesCompatInternal::SeekCb()(handle, 0, 2 /* SEEK_END */);
            MilesCompatInternal::SeekCb()(handle, 0, 0 /* SEEK_SET */);
            if (endPos >= 0) {
                U32 fileSize = static_cast<U32>(endPos);
                size_t allocSize = withSize ? (sizeof(U32) + fileSize) : fileSize;
                uint8_t* buf = static_cast<uint8_t*>(std::malloc(allocSize ? allocSize : 4));
                if (!buf) {
                    MilesCompatInternal::CloseCb()(handle);
                    return nullptr;
                }
                uint8_t* writePtr = buf;
                if (withSize) {
                    std::memcpy(buf, &fileSize, sizeof(U32));
                    writePtr += sizeof(U32);
                }
                if (fileSize > 0) {
                    MilesCompatInternal::ReadCb()(handle, writePtr, fileSize);
                }
                MilesCompatInternal::CloseCb()(handle);
                return buf;
            }
            MilesCompatInternal::CloseCb()(handle);
        }
    }

    FILE* fp = std::fopen(filename, "rb");
    if (!fp)
        return nullptr;

    std::fseek(fp, 0, SEEK_END);
    long len = std::ftell(fp);
    std::fseek(fp, 0, SEEK_SET);
    if (len < 0) {
        std::fclose(fp);
        return nullptr;
    }

    U32 fileSize = static_cast<U32>(len);
    size_t allocSize = withSize ? (sizeof(U32) + fileSize) : fileSize;
    uint8_t* buf = static_cast<uint8_t*>(std::malloc(allocSize ? allocSize : 4));
    if (!buf) {
        std::fclose(fp);
        return nullptr;
    }
    uint8_t* writePtr = buf;
    if (withSize) {
        std::memcpy(buf, &fileSize, sizeof(U32));
        writePtr += sizeof(U32);
    }
    if (fileSize > 0) {
        size_t nread = std::fread(writePtr, 1, fileSize, fp);
        (void)nread;
    }
    std::fclose(fp);
    return buf;
}

inline void AIL_mem_free_lock(void* ptr) {
    std::free(ptr);
}

inline S32 AIL_file_type(const void* data, U32 size) {
    if (!data || size < 4)
        return AILFILETYPE_PCM_WAV;
    const uint8_t* bytes = static_cast<const uint8_t*>(data);
    if (bytes[0] == 'I' && bytes[1] == 'D' && bytes[2] == '3')
        return AILFILETYPE_MPEG_L3_AUDIO;
    if (bytes[0] == 0xFF && (bytes[1] & 0xE0) == 0xE0)
        return AILFILETYPE_MPEG_L3_AUDIO;
    return AILFILETYPE_PCM_WAV;
}

inline S32 AIL_WAV_info(const void* data, AILSOUNDINFO* info) {
    if (!info)
        return 0;
    std::memset(info, 0, sizeof(AILSOUNDINFO));
    info->format = 1;
    info->data_ptr = data;
    info->rate = 44100;
    info->bits = 16;
    info->channels = 2;
    return 1;
}

inline S32 AIL_decompress_ADPCM(const AILSOUNDINFO* info, void** outData, U32* outSize) {
    if (!outData || !outSize)
        return 0;
    U32 sz = (info && info->data_len > 0) ? info->data_len : 64;
    void* buf = std::calloc(1, sz);
    *outData = buf;
    *outSize = sz;
    return buf ? 1 : 0;
}

inline S32 AIL_decompress_ASI(const void* inData, U32 inSize, const char* /*filename*/, void** outData, U32* outSize, U32 /* zero */) {
    if (!outData || !outSize)
        return 0;
    U32 sz = inSize > 0 ? inSize : 64;
    void* buf = std::malloc(sz);
    if (buf && inData && inSize > 0)
        std::memcpy(buf, inData, inSize);
    *outData = buf;
    *outSize = sz;
    return buf ? 1 : 0;
}

// 2D Digital Driver & Sample Functions
inline HDIGDRIVER AIL_open_digital_driver(U32 freq, S32 bits, S32 chans, U32 flags) {
    (void)flags;
    (void)bits;
    HDIGDRIVER drv = new AIL_DigitalDriver{freq, bits, chans, flags};
    MilesCompatInternal::GetEngine();
    return drv;
}

inline void AIL_close_digital_driver(HDIGDRIVER drv) {
    delete drv;
}

inline HSAMPLE AIL_allocate_sample_handle(HDIGDRIVER drv) {
    HSAMPLE s = new AIL_Sample2D();
    s->driver = drv;
    s->status = SMP_DONE;
    return s;
}

inline void AIL_release_sample_handle(HSAMPLE s) {
    if (!s) return;
    delete s;
}

inline void AIL_init_sample(HSAMPLE s) {
    if (!s) return;
    s->cleanup();
    s->data = nullptr;
    s->size = 0;
    s->loop_count = 1;
    s->status = SMP_DONE;
    s->volume = 1.0f;
    s->pan = 0.5f;
}

inline S32 AIL_set_sample_file(HSAMPLE s, const void* file_image, S32 block) {
    if (!s || !file_image)
        return 0;

    s->cleanup();
    s->data = file_image;
    s->size = block;
    s->status = SMP_DONE;

    if (s->size <= 0) {
        const char* p = static_cast<const char*>(file_image);
        if (p[0] == 'R' && p[1] == 'I' && p[2] == 'F' && p[3] == 'F') {
            uint32_t riffSize = *reinterpret_cast<const uint32_t*>(p + 4);
            s->size = static_cast<S32>(riffSize + 8);
        }
    }

    ma_engine* engine = MilesCompatInternal::GetEngine();
    if (engine && s->size > 0) {
        ma_decoder_config dcfg = ma_decoder_config_init(ma_format_f32, 2, 44100);
        if (ma_decoder_init_memory(s->data, s->size, &dcfg, &s->decoder) == MA_SUCCESS) {
            s->decoder_initialized = true;
            if (ma_sound_init_from_data_source(engine, &s->decoder, 0, NULL, &s->sound) == MA_SUCCESS) {
                s->sound_initialized = true;
                ma_sound_set_volume(&s->sound, s->volume);
                ma_sound_set_pan(&s->sound, s->pan * 2.0f - 1.0f);
                ma_sound_set_looping(&s->sound, s->loop_count == 0 ? MA_TRUE : MA_FALSE);
            }
        }
    }

    return 1;
}

inline U32 AIL_sample_status(HSAMPLE s) {
    if (!s) return SMP_DONE;
    if (s->sound_initialized) {
        if (ma_sound_at_end(&s->sound) && !ma_sound_is_looping(&s->sound)) {
            s->status = SMP_DONE;
        } else if (ma_sound_is_playing(&s->sound)) {
            s->status = SMP_PLAYING;
        }
    }
    return s->status;
}

inline void AIL_set_sample_loop_count(HSAMPLE s, S32 loop_count) {
    if (!s) return;
    s->loop_count = loop_count;
    if (s->sound_initialized) {
        ma_sound_set_looping(&s->sound, loop_count == 0 ? MA_TRUE : MA_FALSE);
    }
}

inline void AIL_start_sample(HSAMPLE s) {
    if (!s) return;
    if (s->sound_initialized) {
        ma_sound_seek_to_pcm_frame(&s->sound, 0);
        ma_sound_set_looping(&s->sound, s->loop_count == 0 ? MA_TRUE : MA_FALSE);
        ma_sound_start(&s->sound);
        s->status = SMP_PLAYING;
    } else {
        s->status = SMP_DONE;
    }
}

inline void AIL_stop_sample(HSAMPLE s) {
    if (!s) return;
    if (s->sound_initialized) {
        ma_sound_stop(&s->sound);
    }
    s->status = SMP_STOPPED;
}

inline void AIL_resume_sample(HSAMPLE s) {
    if (!s) return;
    if (s->sound_initialized) {
        ma_sound_start(&s->sound);
        s->status = SMP_PLAYING;
    }
}

inline void AIL_end_sample(HSAMPLE s) {
    if (!s) return;
    if (s->sound_initialized) {
        ma_sound_stop(&s->sound);
        ma_sound_seek_to_pcm_frame(&s->sound, 0);
    }
    s->status = SMP_DONE;
}

inline void AIL_sample_volume_pan(HSAMPLE s, float* volume, float* pan) {
    if (volume) *volume = s ? s->volume : 0.0f;
    if (pan) *pan = s ? s->pan : 0.5f;
}

inline void AIL_set_sample_volume_pan(HSAMPLE s, float volume, float pan) {
    if (!s) return;
    s->volume = volume;
    s->pan = pan;
    if (s->sound_initialized) {
        ma_sound_set_volume(&s->sound, volume);
        ma_sound_set_pan(&s->sound, pan * 2.0f - 1.0f);
    }
}

// 3D Provider, Listener & 3D Sample Functions
inline S32 AIL_enumerate_3D_providers(HPROENUM* next, HPROVIDER* dest, char** name) {
    static char s_providerName[] = "Miles Fast 2D Positional Audio";
    if (!next || !dest || !name)
        return 0;
    if (*next == HPROENUM_FIRST) {
        *next = 1;
        *dest = 1;
        *name = s_providerName;
        return 1;
    }
    return 0;
}

inline S32 AIL_open_3D_provider(HPROVIDER /*lib*/) {
    return M3D_NOERR;
}

inline void AIL_close_3D_provider(HPROVIDER /*lib*/) {
}

inline H3DPOBJECT AIL_open_3D_listener(HPROVIDER lib) {
    H3DPOBJECT listener = new AIL_Object3D();
    listener->provider = lib;
    listener->is_listener = true;
    return listener;
}

inline void AIL_close_3D_listener(H3DPOBJECT listener) {
    delete listener;
}

inline H3DSAMPLE AIL_allocate_3D_sample_handle(HPROVIDER lib) {
    H3DSAMPLE s = new AIL_Object3D();
    s->provider = lib;
    s->status = SMP_DONE;
    return s;
}

inline void AIL_release_3D_sample_handle(H3DSAMPLE s) {
    if (!s) return;
    delete s;
}

inline S32 AIL_set_3D_sample_file(H3DSAMPLE s, const void* file_image) {
    if (!s || !file_image)
        return 0;

    s->cleanup();
    s->data = file_image;
    s->status = SMP_DONE;

    s->size = 0;
    const char* p = static_cast<const char*>(file_image);
    if (p[0] == 'R' && p[1] == 'I' && p[2] == 'F' && p[3] == 'F') {
        uint32_t riffSize = *reinterpret_cast<const uint32_t*>(p + 4);
        s->size = static_cast<S32>(riffSize + 8);
    } else {
        s->size = static_cast<S32>(*(reinterpret_cast<const uint32_t*>(p) - 1));
    }

    ma_engine* engine = MilesCompatInternal::GetEngine();
    if (engine && s->size > 0) {
        // 3D sounds must be mono so the spatializer can pan them. Keep the
        // native sample rate and let miniaudio resample to the engine rate.
        ma_decoder_config dcfg = ma_decoder_config_init(ma_format_f32, 1, 0);
        if (ma_decoder_init_memory(s->data, s->size, &dcfg, &s->decoder) == MA_SUCCESS) {
            s->decoder_initialized = true;
            if (ma_sound_init_from_data_source(engine, &s->decoder, 0, NULL, &s->sound) == MA_SUCCESS) {
                s->sound_initialized = true;
                ma_sound_set_spatialization_enabled(&s->sound, MA_TRUE);
                ma_sound_set_position(&s->sound, s->x, s->y, s->z);
                ma_sound_set_attenuation_model(&s->sound, ma_attenuation_model_inverse);
                ma_sound_set_rolloff(&s->sound, MilesCompatInternal::k3DRolloff);
                ma_sound_set_min_distance(&s->sound, MilesCompatInternal::kMin3DDistance);
                ma_sound_set_max_distance(&s->sound, MilesCompatInternal::kMax3DDistance);
                ma_sound_set_volume(&s->sound, s->volume);
                ma_sound_set_looping(&s->sound, s->loop_count == 0 ? MA_TRUE : MA_FALSE);
            }
        }
    }

    return 1;
}

inline U32 AIL_3D_sample_status(H3DSAMPLE s) {
    if (!s) return SMP_DONE;
    if (s->sound_initialized) {
        if (ma_sound_at_end(&s->sound) && !ma_sound_is_looping(&s->sound)) {
            s->status = SMP_DONE;
        } else if (ma_sound_is_playing(&s->sound)) {
            s->status = SMP_PLAYING;
        }
    }
    return s->status;
}

inline void AIL_set_3D_sample_loop_count(H3DSAMPLE s, U32 loops) {
    if (!s) return;
    s->loop_count = loops;
    if (s->sound_initialized) {
        ma_sound_set_looping(&s->sound, loops == 0 ? MA_TRUE : MA_FALSE);
    }
}

inline void AIL_start_3D_sample(H3DSAMPLE s) {
    if (!s) return;
    if (s->sound_initialized) {
        ma_sound_seek_to_pcm_frame(&s->sound, 0);
        ma_sound_set_looping(&s->sound, s->loop_count == 0 ? MA_TRUE : MA_FALSE);
        ma_sound_start(&s->sound);
        s->status = SMP_PLAYING;
    } else {
        s->status = SMP_DONE;
    }
}

inline void AIL_stop_3D_sample(H3DSAMPLE s) {
    if (!s) return;
    if (s->sound_initialized) {
        ma_sound_stop(&s->sound);
    }
    s->status = SMP_STOPPED;
}

inline void AIL_resume_3D_sample(H3DSAMPLE s) {
    if (!s) return;
    if (s->sound_initialized) {
        ma_sound_start(&s->sound);
        s->status = SMP_PLAYING;
    }
}

inline void AIL_end_3D_sample(H3DSAMPLE s) {
    if (!s) return;
    if (s->sound_initialized) {
        ma_sound_stop(&s->sound);
        ma_sound_seek_to_pcm_frame(&s->sound, 0);
    }
    s->status = SMP_DONE;
}

inline float AIL_3D_sample_volume(H3DSAMPLE s) {
    return s ? s->volume : 0.0f;
}

inline void AIL_set_3D_sample_volume(H3DSAMPLE s, float volume) {
    if (!s) return;
    s->volume = volume;
    if (s->sound_initialized) {
        ma_sound_set_volume(&s->sound, volume);
    }
}

inline void AIL_set_3D_position(H3DPOBJECT obj, float x, float y, float z) {
    if (!obj) return;
    obj->x = x;
    obj->y = y;
    obj->z = z;
    if (obj->is_listener) {
        ma_engine* engine = MilesCompatInternal::GetEngine();
        if (engine) {
            ma_engine_listener_set_position(engine, 0, x, y, z);
        }
        return;
    }
    if (obj->sound_initialized) {
        ma_sound_set_position(&obj->sound, x, y, z);
    }
}

inline void AIL_set_3D_orientation(H3DPOBJECT obj, float fx, float fy, float fz, float ux, float uy, float uz) {
    if (!obj) return;
    obj->fx = fx;
    obj->fy = fy;
    obj->fz = fz;
    obj->ux = ux;
    obj->uy = uy;
    obj->uz = uz;
    if (obj->is_listener) {
        ma_engine* engine = MilesCompatInternal::GetEngine();
        if (engine) {
            ma_engine_listener_set_direction(engine, 0, fx, fy, fz);
            ma_engine_listener_set_world_up(engine, 0, ux, uy, uz);
        }
        return;
    }
    if (obj->sound_initialized) {
        ma_sound_set_direction(&obj->sound, fx, fy, fz);
    }
}

inline void AIL_set_3D_velocity(H3DPOBJECT obj, float dx, float dy, float dz, float mag) {
    if (!obj) return;
    obj->vx = dx;
    obj->vy = dy;
    obj->vz = dz;
    obj->vmag = mag;
    if (obj->is_listener) {
        ma_engine* engine = MilesCompatInternal::GetEngine();
        if (engine) {
            ma_engine_listener_set_velocity(engine, 0, dx, dy, dz);
        }
        return;
    }
    if (obj->sound_initialized) {
        ma_sound_set_velocity(&obj->sound, dx, dy, dz);
    }
}

inline void AIL_set_listener_3D_position(HPROVIDER /*lib*/, float x, float y, float z) {
    ma_engine* engine = MilesCompatInternal::GetEngine();
    if (engine) {
        ma_engine_listener_set_position(engine, 0, x, y, z);
    }
}

inline void AIL_set_listener_3D_orientation(HPROVIDER /*lib*/, float fx, float fy, float fz, float ux, float uy, float uz) {
    ma_engine* engine = MilesCompatInternal::GetEngine();
    if (engine) {
        ma_engine_listener_set_direction(engine, 0, fx, fy, fz);
        ma_engine_listener_set_world_up(engine, 0, ux, uy, uz);
    }
}

inline void AIL_auto_update_3D_position(H3DSAMPLE s, S32 enable) {
    if (s) s->auto_update = enable;
}

inline void AIL_update_3D_position(H3DSAMPLE /*s*/, float /*dt*/) {
}

// Stream Functions
inline HSTREAM AIL_open_stream(HDIGDRIVER drv, const char* filename, S32 /*stream_mem*/) {
    if (!filename || !*filename)
        return nullptr;
    HSTREAM st = new AIL_Stream();
    st->driver = drv;
    std::strncpy(st->filename, filename, sizeof(st->filename) - 1);
    st->status = -1;

    std::string normPath = filename;
    for (char& c : normPath) {
        if (c == '\\') c = '/';
    }

    ma_engine* engine = MilesCompatInternal::GetEngine();
    if (engine) {
        ma_result res = ma_sound_init_from_file(engine, normPath.c_str(), MA_SOUND_FLAG_STREAM, NULL, NULL, &st->sound);
        if (res != MA_SUCCESS) {
            if (normPath.length() >= 4 && (normPath[0] == 'b' || normPath[0] == 'B') &&
                (normPath[1] == 'g' || normPath[1] == 'G') &&
                (normPath[2] == 'm' || normPath[2] == 'M') && normPath[3] == '/') {
                std::string alt = "BGM/" + normPath.substr(4);
                res = ma_sound_init_from_file(engine, alt.c_str(), MA_SOUND_FLAG_STREAM, NULL, NULL, &st->sound);
                if (res != MA_SUCCESS) {
                    alt = "bgm/" + normPath.substr(4);
                    res = ma_sound_init_from_file(engine, alt.c_str(), MA_SOUND_FLAG_STREAM, NULL, NULL, &st->sound);
                }
            } else if (normPath.find('/') == std::string::npos) {
                std::string bgmPath = "BGM/" + normPath;
                res = ma_sound_init_from_file(engine, bgmPath.c_str(), MA_SOUND_FLAG_STREAM, NULL, NULL, &st->sound);
                if (res != MA_SUCCESS) {
                    bgmPath = "bgm/" + normPath;
                    res = ma_sound_init_from_file(engine, bgmPath.c_str(), MA_SOUND_FLAG_STREAM, NULL, NULL, &st->sound);
                }
            }
        }
        if (res == MA_SUCCESS) {
            st->sound_initialized = true;
            ma_sound_set_volume(&st->sound, (st->left_vol + st->right_vol) * 0.5f);
            ma_sound_set_looping(&st->sound, st->loop_count == 0 ? MA_TRUE : MA_FALSE);
        }
    }

    return st;
}

inline void AIL_close_stream(HSTREAM st) {
    if (!st) return;
    delete st;
}

inline S32 AIL_stream_status(HSTREAM st) {
    if (!st) return -1;
    if (st->sound_initialized) {
        if (ma_sound_at_end(&st->sound) && !ma_sound_is_looping(&st->sound)) {
            st->status = -1;
        } else if (ma_sound_is_playing(&st->sound)) {
            st->status = 0;
        }
    }
    return st->status;
}

inline void AIL_set_stream_loop_count(HSTREAM st, S32 count) {
    if (!st) return;
    st->loop_count = count;
    if (st->sound_initialized) {
        ma_sound_set_looping(&st->sound, count == 0 ? MA_TRUE : MA_FALSE);
    }
}

inline void AIL_start_stream(HSTREAM st) {
    if (!st) return;
    if (st->sound_initialized) {
        ma_sound_set_looping(&st->sound, st->loop_count == 0 ? MA_TRUE : MA_FALSE);
        ma_sound_start(&st->sound);
        st->status = 0;
    } else {
        st->status = 0;
    }
}

inline void AIL_pause_stream(HSTREAM st, S32 onoff) {
    if (!st) return;
    if (st->sound_initialized) {
        if (onoff) {
            ma_sound_stop(&st->sound);
            st->status = 1;
        } else {
            ma_sound_start(&st->sound);
            st->status = 0;
        }
    } else {
        st->status = onoff ? 1 : 0;
    }
}

inline void AIL_stream_volume_levels(HSTREAM st, float* left, float* right) {
    if (left) *left = st ? st->left_vol : 0.0f;
    if (right) *right = st ? st->right_vol : 0.0f;
}

inline void AIL_set_stream_volume_levels(HSTREAM st, float left, float right) {
    if (!st) return;
    st->left_vol = left;
    st->right_vol = right;
    if (st->sound_initialized) {
        ma_sound_set_volume(&st->sound, (left + right) * 0.5f);
    }
}
