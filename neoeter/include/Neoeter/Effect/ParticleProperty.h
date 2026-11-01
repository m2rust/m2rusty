#pragma once

#include <vector>
#include <Neoeter/Render/GrpImageInstance.h>

#include "Type.h"

class CParticleProperty
{
    friend class CParticleSystemData;
    friend class CParticleSystemInstance;

  public:
    enum
    {
        ROTATION_TYPE_NONE,
        ROTATION_TYPE_TIME_EVENT,
        ROTATION_TYPE_CW,
        ROTATION_TYPE_CCW,
        ROTATION_TYPE_RANDOM_DIRECTION,
    };

    enum
    {
        TEXTURE_ANIMATION_TYPE_NONE,
        TEXTURE_ANIMATION_TYPE_CW,
        TEXTURE_ANIMATION_TYPE_CCW,
        TEXTURE_ANIMATION_TYPE_RANDOM_FRAME,
        TEXTURE_ANIMATION_TYPE_RANDOM_DIRECTION,
    };

    CParticleProperty();
    virtual ~CParticleProperty();

    void Clear();

    void InsertTexture(const char *c_szFileName);
    bool SetTexture(const char *c_szFileName);

    __forceinline BYTE GetTextureAnimationType() const
    {
        return m_byTexAniType;
    }

    __forceinline DWORD GetTextureAnimationFrameCount() const
    {
        return m_ImageVector.size();
    }

    __forceinline float GetTextureAnimationFrameDelay() const
    {
        return m_fTexAniDelay;
    }

    BYTE m_byTexAniType = 0U;
    float m_fTexAniDelay = 0.0F;
    BOOL m_bTexAniRandomStartFrameFlag = 0;

    BYTE m_bySrcBlendType = 0U;
    BYTE m_byDestBlendType = 0U;
    BYTE m_byColorOperationType = 0U;
    BYTE m_byBillboardType = 0U;

    BYTE m_byRotationType = 0U;
    float m_fRotationSpeed = 0.0F;
    WORD m_wRotationRandomStartingBegin = 0U;
    WORD m_wRotationRandomStartingEnd = 0U;

    BOOL m_bAttachFlag = 0;
    BOOL m_bStretchFlag = 0;

    TTimeEventTableFloat m_TimeEventGravity;
    TTimeEventTableFloat m_TimeEventAirResistance;

    TTimeEventTableFloat m_TimeEventScaleX;
    TTimeEventTableFloat m_TimeEventScaleY;
#ifdef WORLD_EDITOR
    TTimeEventTableFloat m_TimeEventColorRed;
    TTimeEventTableFloat m_TimeEventColorGreen;
    TTimeEventTableFloat m_TimeEventColorBlue;
    TTimeEventTableFloat m_TimeEventAlpha;

    std::vector<std::string> m_TextureNameVector;
#else
    TTimeEventTableColor m_TimeEventColor;
#endif
    TTimeEventTableFloat m_TimeEventRotation;

    std::vector<CGraphicImage *> m_ImageVector;

    CParticleProperty &operator=(const CParticleProperty &c_ParticleProperty);

    // pre-transformed variables
    D3DXVECTOR3 m_v3ZAxis;
};
