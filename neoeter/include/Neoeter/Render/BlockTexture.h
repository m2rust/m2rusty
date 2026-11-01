#pragma once

#include "GrpBase.h"

class CGraphicDib;

class CBlockTexture : public CGraphicBase
{
  public:
    CBlockTexture();
    ~CBlockTexture() override;

    bool Create(CGraphicDib *pDIB, const RECT &c_rRect, DWORD dwWidth, DWORD dwHeight);
    void SetClipRect(const RECT &c_rRect);
    void Render(int ix, int iy);
    void InvalidateRect(const RECT &c_rsrcRect);

  protected:
    CGraphicDib *m_pDIB;
    RECT m_rect{};
    RECT m_clipRect{};
    BOOL m_bClipEnable = 0;
    DWORD m_dwWidth = 0U;
    DWORD m_dwHeight = 0U;
    LPDIRECT3DTEXTURE9 m_lpd3dTexture;
};
