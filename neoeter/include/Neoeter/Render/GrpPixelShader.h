#pragma once

#include "GrpBase.h"

class CPixelShader : public CGraphicBase
{
  public:
    CPixelShader();
    ~CPixelShader() override;

    void Destroy();
    bool CreateFromDiskFile(const char *c_szFileName);

    void Set();

  protected:
    void Initialize();

    LPDIRECT3DPIXELSHADER9 m_handle = nullptr;
};
