#pragma once

#include "EffectElementBaseInstance.h"
#include "ParticleInstance.h"
#include "ParticleProperty.h"

#include <Neoeter/Render/GrpScreen.h>
#include <Neoeter/Render/StateManager.h>
#include <Neoeter/Render/GrpImageInstance.h>
#include "EmitterProperty.h"

class CParticleSystemInstance : public CEffectElementBaseInstance
{
  public:
    static void DestroySystem();

    static CParticleSystemInstance *New();
    static void Delete(CParticleSystemInstance *pkPSInst);

    static CDynamicPool<CParticleSystemInstance> ms_kPool;

    template <typename T> void ForEachParticleRendering(T FunObj)
    {
        DWORD dwFrameIndex = 0;
        for (dwFrameIndex = 0; dwFrameIndex < m_kVct_pkImgInst.size(); dwFrameIndex++)
        {
            STATEMANAGER.SetTexture(0, m_kVct_pkImgInst[dwFrameIndex]->GetTextureReference().GetD3DTexture());
            auto itor = m_ParticleInstanceListVector[dwFrameIndex].begin();
            for (; itor != m_ParticleInstanceListVector[dwFrameIndex].end(); ++itor)
            {
                if (!InFrustum(*itor))
                    return;
                FunObj(*itor);
            }
        }
    }

    CParticleSystemInstance();
    ~CParticleSystemInstance() override;

    void OnSetDataPointer(CEffectElementBase *pElement) override;

    void CreateParticles(float fElapsedTime);

    bool InFrustum(CParticleInstance *pInstance)
    {
        if (m_pParticleProperty->m_bAttachFlag)
        {
            return CScreen::GetFrustum().ViewVolumeTest(Vector3d(pInstance->m_v3Position.x + mc_pmatLocal->_41,
                                                                 pInstance->m_v3Position.y + mc_pmatLocal->_42,
                                                                 pInstance->m_v3Position.z + mc_pmatLocal->_43),
                                                        pInstance->GetRadiusApproximation()) != VS_OUTSIDE;
        }
        return CScreen::GetFrustum().ViewVolumeTest(
                   Vector3d(pInstance->m_v3Position.x, pInstance->m_v3Position.y, pInstance->m_v3Position.z),
                   pInstance->GetRadiusApproximation()) != VS_OUTSIDE;
    }

    DWORD GetEmissionCount() const;

  protected:
    void OnInitialize() override;
    void OnDestroy() override;

    bool OnUpdate(float fElapsedTime) override;
    void OnRender() override;

    float m_fEmissionResidue = 0.0F;

    DWORD m_dwCurrentEmissionCount = 0U;
    int m_iLoopCount = 0;

    using TParticleInstanceList = std::list<CParticleInstance *>;
    using TParticleInstanceListVector = std::vector<TParticleInstanceList>;
    TParticleInstanceListVector m_ParticleInstanceListVector;

    using TImageInstanceVector = std::vector<CGraphicImageInstance *>;
    TImageInstanceVector m_kVct_pkImgInst;

    CParticleSystemData *m_pData = nullptr;

    CParticleProperty *m_pParticleProperty = nullptr;
    CEmitterProperty *m_pEmitterProperty = nullptr;
};
