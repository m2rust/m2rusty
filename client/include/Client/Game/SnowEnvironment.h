#pragma once

#include <Neoeter/Render/GrpScreen.h>

class CSnowParticle;

class CSnowEnvironment : public CScreen
{
  public:
    CSnowEnvironment();
    ~CSnowEnvironment() override;

    bool Create();
    void Destroy();

    void Enable();
    void Disable();

    void Update(const D3DXVECTOR3 &c_rv3Pos);
    void Deform();
    void Render();

  protected:
    void __Initialize();
    bool __CreateBlurTexture();
    bool __CreateGeometry();
    void __BeginBlur();
    void __ApplyBlur();

    LPDIRECT3DSURFACE9 m_lpOldSurface = nullptr;
    LPDIRECT3DSURFACE9 m_lpOldDepthStencilSurface = nullptr;

    LPDIRECT3DTEXTURE9 m_lpSnowTexture = nullptr;
    LPDIRECT3DSURFACE9 m_lpSnowRenderTargetSurface = nullptr;
    LPDIRECT3DSURFACE9 m_lpSnowDepthSurface = nullptr;

    LPDIRECT3DTEXTURE9 m_lpAccumTexture = nullptr;
    LPDIRECT3DSURFACE9 m_lpAccumRenderTargetSurface = nullptr;
    LPDIRECT3DSURFACE9 m_lpAccumDepthSurface = nullptr;

    LPDIRECT3DVERTEXBUFFER9 m_pVB = nullptr;
    LPDIRECT3DINDEXBUFFER9 m_pIB = nullptr;

    D3DXVECTOR3 m_v3Center;

    WORD m_wBlurTextureSize;
    CGraphicImageInstance *m_pImageInstance = nullptr;
    std::vector<CSnowParticle *> m_kVct_pkParticleSnow;

    DWORD m_dwParticleMaxNum;
    BOOL m_bBlurEnable;

    BOOL m_bSnowEnable = 0;
};
