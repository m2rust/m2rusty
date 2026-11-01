#pragma once

#include <Neoeter/Render/GrpScreen.h>

#include "EffectElementBaseInstance.h"
#include "SimpleLightData.h"

class CLightInstance : public CEffectElementBaseInstance
{
  public:
    friend class CLightData;

    CLightInstance();
    ~CLightInstance() override;

  protected:
    void OnSetDataPointer(CEffectElementBase *pElement) override;

    void OnInitialize() override;
    void OnDestroy() override;

    bool OnUpdate(float fElapsedTime) override;
    void OnRender() override;

    DWORD m_LightID = 0U;
    CLightData *m_pData = nullptr;
    DWORD m_dwRangeIndex = 0U;

    DWORD m_iLoopCount = 0U;

  public:
    static void DestroySystem();

    static CLightInstance *New();
    static void Delete(CLightInstance *pkData);

    static CDynamicPool<CLightInstance> ms_kPool;
};