#pragma once

#include "GrpBase.h"

class CGraphicTexture : public CGraphicBase
{
  public:
    virtual bool IsEmpty() const;

    int GetWidth() const;
    int GetHeight() const;

    void SetTextureStage(int stage) const;
    LPDIRECT3DTEXTURE9 GetD3DTexture() const;

    void DestroyDeviceObjects();

  protected:
    CGraphicTexture();
    ~CGraphicTexture() override;

    void Destroy();
    void Initialize();

    bool m_bEmpty = false;

    int m_width = 0;
    int m_height = 0;

    LPDIRECT3DTEXTURE9 m_lpd3dTexture = nullptr;
};
