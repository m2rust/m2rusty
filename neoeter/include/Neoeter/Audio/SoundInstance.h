#ifndef __MILESLIB_CSOUNDINSTANCE_H__
#define __MILESLIB_CSOUNDINSTANCE_H__

#include "SoundBase.h"

class ISoundInstance : public CSoundBase
{
  public:
    ISoundInstance() = default;
    ~ISoundInstance() override = default;

    virtual bool Initialize() = 0;
    virtual void Destroy() = 0;
    virtual bool SetSound(CSoundData *pSound) = 0;
    virtual void Play(int iLoopCount = 1, DWORD dwPlayCycleTimeLimit = 0) const = 0;
    virtual void Pause() const = 0;
    virtual void Resume() const = 0;
    virtual void Stop() = 0;
    virtual void GetVolume(float &rfVolume) const = 0;
    virtual void SetVolume(float volume) const = 0;
    virtual bool IsDone() const = 0;
    virtual void SetPosition(float x, float y, float z) const = 0;
    virtual void SetOrientation(float x_face, float y_face, float z_face, float x_normal, float y_normal,
                                float z_normal) const = 0;
    virtual void SetVelocity(float x, float y, float z, float fMagnitude) const = 0;
};

class CSoundInstance2D : public ISoundInstance
{
  public:
    CSoundInstance2D();
    ~CSoundInstance2D() override;

    // from interface
    bool Initialize() override;
    void Destroy() override;

    bool SetSound(CSoundData *pSound) override;
    void Play(int iLoopCount = 1, DWORD dwPlayCycleTimeLimit = 0) const override;
    void Pause() const override;
    void Resume() const override;
    void Stop() override;
    void GetVolume(float &rfVolume) const override;
    void SetVolume(float volume) const override;
    bool IsDone() const override;
    void SetPosition(float x, float y, float z) const override;
    void SetOrientation(float x_face, float y_face, float z_face, float x_normal, float y_normal,
                        float z_normal) const override;
    void SetVelocity(float fx, float fy, float fz, float fMagnitude) const override;

  private:
    HSAMPLE m_sample;
    CSoundData *m_pSoundData;
};

class CSoundInstance3D : public ISoundInstance
{
  public:
    CSoundInstance3D();
    ~CSoundInstance3D() override;

    // from interface
    bool Initialize() override;
    void Destroy() override;

    bool SetSound(CSoundData *pSound) override;
    void Play(int iLoopCount = 1, DWORD dwPlayCycleTimeLimit = 0) const override;
    void Pause() const override;
    void Resume() const override;
    void Stop() override;
    void GetVolume(float &rfVolume) const override;
    void SetVolume(float volume) const override;
    bool IsDone() const override;

    void SetPosition(float x, float y, float z) const override;
    void SetOrientation(float x_face, float y_face, float z_face, float x_normal, float y_normal,
                        float z_normal) const override;
    void SetVelocity(float fDistanceX, float fDistanceY, float fDistanceZ, float fNagnitude) const override;

    void UpdatePosition(float fElapsedTime);

  private:
    H3DSAMPLE m_sample;
    CSoundData *m_pSoundData;
};

class CSoundInstanceStream : public ISoundInstance
{
  public:
    CSoundInstanceStream();
    ~CSoundInstanceStream() override;

    // from interface
    bool Initialize() override;
    void Destroy() override;

    void SetStream(HSTREAM stream);
    bool SetSound(CSoundData *pSound) override;

    void Play(int iLoopCount = 1, DWORD dwPlayCycleTimeLimit = 0) const override;
    void Pause() const override;
    void Resume() const override;
    void Stop() override;
    void GetVolume(float &rfVolume) const override;
    void SetVolume(float volume) const override;
    bool IsDone() const override;
    bool IsData() const;

    void SetPosition(float x, float y, float z) const override;
    void SetOrientation(float x_face, float y_face, float z_face, float x_normal, float y_normal,
                        float z_normal) const override;
    void SetVelocity(float fx, float fy, float fz, float fMagnitude) const override;

  private:
    HSTREAM m_stream;
};

#endif
