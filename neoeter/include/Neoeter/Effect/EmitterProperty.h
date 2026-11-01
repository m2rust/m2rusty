#pragma once

#include "Type.h"

class CEmitterProperty
{
    friend class CParticleSystemData;
    friend class CParticleSystemInstance;

  public:
    enum
    {
        EMITTER_SHAPE_POINT,
        EMITTER_SHAPE_ELLIPSE,
        EMITTER_SHAPE_SQUARE,
        EMITTER_SHAPE_SPHERE,
    };

    enum
    {
        EMITTER_ADVANCED_TYPE_FREE,
        EMITTER_ADVANCED_TYPE_OUTER,
        EMITTER_ADVANCED_TYPE_INNER,
    };

    CEmitterProperty();
    virtual ~CEmitterProperty();

    void Clear();

    DWORD GetMaxEmissionCount() const
    {
        return m_dwMaxEmissionCount;
    }

    float GetCycleLength() const
    {
        return m_fCycleLength;
    }

    BOOL isCycleLoop() const
    {
        return m_bCycleLoopFlag;
    }

    int GetLoopCount() const
    {
        return m_iLoopCount;
    }

    BYTE GetEmitterShape() const;
    BYTE GetEmitterAdvancedType() const;
    BOOL isEmitFromEdge() const;

    void GetEmittingSize(float fTime, float *pfValue);
    void GetEmittingAngularVelocity(float fTime, float *pfValue);

    void GetEmittingDirectionX(float fTime, float *pfValue);
    void GetEmittingDirectionY(float fTime, float *pfValue);
    void GetEmittingDirectionZ(float fTime, float *pfValue);
    void GetEmittingVelocity(float fTime, float *pfValue);
    void GetEmissionCountPerSecond(float fTime, float *pfValue);
    void GetParticleLifeTime(float fTime, float *pfValue);
    void GetParticleSizeX(float fTime, float *pfValue);
    void GetParticleSizeY(float fTime, float *pfValue);

    /////

    DWORD m_dwMaxEmissionCount = 0U;

    float m_fCycleLength = 0.0F;
    BOOL m_bCycleLoopFlag = 0;
    int m_iLoopCount = 0;

    BYTE m_byEmitterShape = 0U;
    BYTE m_byEmitterAdvancedType = 0U;
    BOOL m_bEmitFromEdgeFlag = 0;
    D3DXVECTOR3 m_v3EmittingSize;
    float m_fEmittingRadius = 0.0F;

    D3DXVECTOR3 m_v3EmittingDirection;

    // TTimeEventTableFloat m_TimeEventEmittingRadius;
    TTimeEventTableFloat m_TimeEventEmittingSize;
    TTimeEventTableFloat m_TimeEventEmittingAngularVelocity;
    TTimeEventTableFloat m_TimeEventEmittingDirectionX;
    TTimeEventTableFloat m_TimeEventEmittingDirectionY;
    TTimeEventTableFloat m_TimeEventEmittingDirectionZ;
    TTimeEventTableFloat m_TimeEventEmittingVelocity;
    TTimeEventTableFloat m_TimeEventEmissionCountPerSecond;
    TTimeEventTableFloat m_TimeEventLifeTime;
    TTimeEventTableFloat m_TimeEventSizeX;
    TTimeEventTableFloat m_TimeEventSizeY;
};
