#pragma once

#include <Neoeter/Render/GrpBase.h>
#include <Neoeter/Render/Pool.h>
#include "EffectUpdateDecorator.h"
class CParticleProperty;
class CEmitterProperty;

class CParticleInstance
{
    friend class CParticleSystemData;
    friend class CParticleSystemInstance;

    friend class NEffectUpdateDecorator::CBaseDecorator;
    friend class NEffectUpdateDecorator::CAirResistanceDecorator;
    friend class NEffectUpdateDecorator::CGravityDecorator;
    friend class NEffectUpdateDecorator::CRotationDecorator;

  public:
    CParticleInstance();
    ~CParticleInstance();

    float GetRadiusApproximation() const;

    // Read-only state accessors (used by deterministic effect tests and
    // debug tooling; the simulation still owns the data).
    const D3DXVECTOR3 &GetPosition() const
    {
        return m_v3Position;
    }
    const D3DXVECTOR3 &GetVelocity() const
    {
        return m_v3Velocity;
    }
    float GetLifeTime() const
    {
        return m_fLastLifeTime;
    }

    BOOL Update(float fElapsedTime, float fAngle);
    // virtual void Transform(const D3DXMATRIX * c_matLocal, const float
    // c_fZRotation)=0; virtual void Transform(const D3DXMATRIX * c_matLocal =
    // NULL)=0;

    // virtual TPTVertex * GetParticleMeshPointer() = 0;

    //__forceinline float GetLifePercentage()
    //{
    //	return m_fLifePercentage;
    // return (m_fLifeTime - m_fLastLifeTime) / m_fLifeTime;
    //}

    // virtual void DeleteThis() = 0;

  protected:
    // float				m_fLifePercentage;
    D3DXVECTOR3 m_v3StartPosition;

    D3DXVECTOR3 m_v3Position;
    D3DXVECTOR3 m_v3LastPosition;
    D3DXVECTOR3 m_v3Velocity;

    D3DXVECTOR2 m_v2HalfSize;
    D3DXVECTOR2 m_v2Scale;

    float m_fRotation = 0.0F;
#ifdef WORLD_EDITOR
    D3DXCOLOR m_Color;
#else
    DWORDCOLOR m_dcColor{};
#endif

    BYTE m_byTextureAnimationType = 0U;
    float m_fLastFrameTime = 0.0F;
    BYTE m_byFrameIndex = 0U;

    float m_fLifeTime = 0.0F;
    float m_fLastLifeTime = 0.0F;

    CParticleProperty *m_pParticleProperty = nullptr;
    CEmitterProperty *m_pEmitterProperty = nullptr;

    float m_fAirResistance = 0.0F;
    float m_fRotationSpeed = 0.0F;
    float m_fGravity = 0.0F;

    NEffectUpdateDecorator::CBaseDecorator *m_pDecorator = nullptr;

  public:
    static CParticleInstance *New();
    static void DestroySystem();

    void Transform(const D3DXMATRIX *c_matLocal = nullptr);
    void Transform(const D3DXMATRIX *c_matLocal, float c_fZRotation);

    TPTVertex *GetParticleMeshPointer();

    void DeleteThis();

    void Destroy();

  protected:
    void __Initialize();
    TPTVertex m_ParticleMesh[4];

  public:
    static CDynamicPool<CParticleInstance> ms_kPool;
};
