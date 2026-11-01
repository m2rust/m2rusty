#pragma once

#include <Neoeter/Render/TextFileLoader.h>

#include "EffectElementBase.h"
#include "EmitterProperty.h"
#include "ParticleProperty.h"
// #include "ParticleInstance.h"

class CParticleInstance;

class CParticleSystemData : public CEffectElementBase
{
  public:
    ~CParticleSystemData() override;
    CParticleSystemData();

    CEmitterProperty *GetEmitterPropertyPointer();
    CParticleProperty *GetParticlePropertyPointer();

    void ChangeTexture(const char *c_szFileName);

    void BuildDecorator(CParticleInstance *pInstance);

  protected:
    BOOL OnLoadScript(CTextFileLoader &rTextFileLoader) override;

    void OnClear() override;
    bool OnIsData() override;

    CEmitterProperty m_EmitterProperty;
    CParticleProperty m_ParticleProperty;

  public:
    static void DestroySystem();

    static CParticleSystemData *New();
    static void Delete(CParticleSystemData *pkData);

    static CDynamicPool<CParticleSystemData> ms_kPool;
};
