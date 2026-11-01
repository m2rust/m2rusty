#pragma once

#include "GrpBase.h"

class CVertexShader : public CGraphicBase
{
  public:
    CVertexShader();
    ~CVertexShader() override;

    void Destroy();
    bool CreateFromDiskFile(const char *c_szFileName, const DWORD *c_pdwVertexDecl);

    void Set();

  protected:
    void Initialize();

    LPDIRECT3DVERTEXSHADER9 m_handle = nullptr;
};
