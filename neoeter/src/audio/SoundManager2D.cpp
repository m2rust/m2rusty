#include <Neoeter/Audio/StdAfx.h>
#include <Neoeter/Audio/SoundManager2D.h>

CSoundManager2D::CSoundManager2D() = default;

CSoundManager2D::~CSoundManager2D() = default;

bool CSoundManager2D::Initialize()
{
    CSoundBase::Initialize();

    if (ms_DIGDriver)
        return true;

    ms_DIGDriver = AIL_open_digital_driver(44100, 16, 2, 0);

    for (auto &ms_Instance : ms_Instances)
        ms_Instance.Initialize();
    /*	ms_DIGDriver = AIL_open_digital_driver(44100,
                                               (DIG_F_STEREO_16 &
       DIG_F_16BITS_MASK) ? 16 : 8, (DIG_F_STEREO_16 & DIG_F_16BITS_MASK) ? 2 :
       1, 0);
    */
    return true;
}

void CSoundManager2D::Destroy()
{
    for (auto &ms_Instance : ms_Instances)
        ms_Instance.Destroy();

    if (ms_DIGDriver != nullptr)
    {
        AIL_close_digital_driver(ms_DIGDriver);
        ms_DIGDriver = nullptr;
    }

    CSoundBase::Destroy();
}

ISoundInstance *CSoundManager2D::GetInstance(const char *c_pszFileName)
{
    DWORD const dwFileCRC = GetFileCRC(c_pszFileName);
    auto itor = ms_dataMap.find(dwFileCRC);

    CSoundData *pkSoundData = nullptr;

    if (itor == ms_dataMap.end())
    {
        pkSoundData = AddFile(dwFileCRC, c_pszFileName); // CSoundBase::AddFile
    }
    else
    {
        pkSoundData = itor->second;
    }

    assert(pkSoundData != nullptr);

    static DWORD k = 0;

    DWORD start = k++;
    DWORD const end = start + INSTANCE_MAX_COUNT;

    while (start < end)
    {
        CSoundInstance2D *pkInst = &ms_Instances[start % INSTANCE_MAX_COUNT];

        if (pkInst->IsDone())
        {
            if (!pkInst->SetSound(pkSoundData))
                TraceError("CSoundManager2D::GetInstance (filename: %s)", c_pszFileName);
            return (pkInst);
        }

        ++start;
    }

    return nullptr;
}
