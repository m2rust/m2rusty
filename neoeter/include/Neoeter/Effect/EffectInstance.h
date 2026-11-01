#pragma once

#include <Neoeter/Render/GrpObjectInstance.h>
#include <Neoeter/Render/Pool.h>
#include <Neoeter/Audio/Type.h>

#include "EffectElementBaseInstance.h"
#include "EffectData.h"
#include "EffectMeshInstance.h"
#include "ParticleSystemInstance.h"
#include "SimpleLightInstance.h"

class CEffectInstance : public CGraphicObjectInstance
{
  public:
    using TEffectElementInstanceVector = std::vector<CEffectElementBaseInstance *>;

    enum
    {
        ID = EFFECT_OBJECT
    };
    int GetType() const override
    {
        return CEffectInstance::ID;
    }

    bool GetBoundingSphere(D3DXVECTOR3 &v3Center, float &fRadius) override;

    static void DestroySystem();

    static CEffectInstance *New();
    static void Delete(CEffectInstance *pkEftInst);

    static void ResetRenderingEffectCount();
    static int GetRenderingEffectCount();

    CEffectInstance();
    ~CEffectInstance() override;

    bool LessRenderOrder(CEffectInstance *pkEftInst);

    void SetEffectDataPointer(CEffectData *pEffectData);

    void Clear();
    BOOL isAlive() const;
    void SetActive();
    void SetDeactive();
    void SetGlobalMatrix(const D3DXMATRIX &c_rmatGlobal);
    void UpdateSound();
    void OnUpdate() override;
    void OnRender() override;
    void OnBlendRender() override
    {
    } // Not used
    void OnRenderToShadowMap() override
    {
    } // Not used
    void OnRenderShadow() override
    {
    } // Not used
    void OnRenderPCBlocker() override
    {
    } // Not used

  protected:
    void __Initialize();

    void __SetParticleData(CParticleSystemData *pData);
    void __SetMeshData(CEffectMeshScript *pMesh);
    void __SetLightData(CLightData *pData);

    void OnUpdateCollisionData(const CStaticCollisionDataVector *pscdVector) override
    {
    } // Not used
    void OnUpdateHeighInstance(CAttributeInstance *pAttributeInstance) override
    {
    }
    bool OnGetObjectHeight(float fX, float fY, float *pfHeight) override
    {
        return false;
    }

    BOOL m_isAlive = 0;
    DWORD m_dwFrame = 0U;
    D3DXMATRIX m_matGlobal;

    CEffectData *m_pkEftData = nullptr;

    std::vector<CParticleSystemInstance *> m_ParticleInstanceVector;
    std::vector<CEffectMeshInstance *> m_MeshInstanceVector;
    std::vector<CLightInstance *> m_LightInstanceVector;

    NSound::TSoundInstanceVector *m_pSoundInstanceVector = nullptr;

    float m_fBoundingSphereRadius = 0.0F;
    D3DXVECTOR3 m_v3BoundingSpherePosition;

    float m_fLastTime = 0.0F;

  public:
    static CDynamicPool<CEffectInstance> ms_kPool;
    static int ms_iRenderingEffectCount;
};
