#pragma once

#include <Platform/Platform.h>
#include <Platform/DirectXMathCompat.h>

#include <Neoeter/Render/TextFileLoader.h>

#include "Type.h"
#include "EffectElementBase.h"

class CLightData : public CEffectElementBase
{
    friend class CLightInstance;

  public:
    CLightData();
    ~CLightData() override;

    void GetRange(float fTime, float &rRange);
    float GetDuration() const;
    BOOL isLoop() const
    {
        return m_bLoopFlag;
    }
    int GetLoopCount() const
    {
        return m_iLoopCount;
    }
    void InitializeLight(D3DLIGHT9 &light);

  protected:
    void OnClear() override;
    bool OnIsData() override;

    BOOL OnLoadScript(CTextFileLoader &rTextFileLoader) override;

    float m_fMaxRange = 0.0F;
    float m_fDuration = 0.0F;
    TTimeEventTableFloat m_TimeEventTableRange;

    D3DXCOLOR m_cAmbient;
    D3DXCOLOR m_cDiffuse;

    BOOL m_bLoopFlag = 0;
    int m_iLoopCount = 0;

    float m_fAttenuation0 = 0.0F;
    float m_fAttenuation1 = 0.0F;
    float m_fAttenuation2 = 0.0F;

  public:
    static void DestroySystem();

    static CLightData *New();
    static void Delete(CLightData *pkData);

    static CDynamicPool<CLightData> ms_kPool;
};
