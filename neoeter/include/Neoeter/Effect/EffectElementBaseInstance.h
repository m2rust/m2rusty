#pragma once

#include "EffectElementBase.h"

class CEffectElementBaseInstance
{
  public:
    CEffectElementBaseInstance();
    virtual ~CEffectElementBaseInstance();

    void SetDataPointer(CEffectElementBase *pElement);

    void Initialize();
    void Destroy();

    void SetLocalMatrixPointer(const D3DXMATRIX *c_pMatrix);
    bool Update(float fElapsedTime);
    void Render();

    bool isActive() const;
    void SetActive();
    void SetDeactive();

  protected:
    virtual void OnSetDataPointer(CEffectElementBase *pElement) = 0;

    virtual void OnInitialize() = 0;
    virtual void OnDestroy() = 0;

    virtual bool OnUpdate(float fElapsedTime) = 0;
    virtual void OnRender() = 0;

    const D3DXMATRIX *mc_pmatLocal = nullptr;

    bool m_isActive = false;

    float m_fLocalTime = 0.0F;
    DWORD m_dwStartTime = 0U;
    float m_fElapsedTime = 0.0F;
    float m_fRemainingTime = 0.0F;
    bool m_bStart = false;

  private:
    CEffectElementBase *m_pBase = nullptr;
};