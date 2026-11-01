#include <Neoeter/Render/StdAfx.h>
#include <Neoeter/Render/BlockTexture.h>
#include <Neoeter/Render/GrpBase.h>
#include <Neoeter/Render/GrpDIB.h>
#include <Neoeter/Platform/Base/Stl.h>
#include <Neoeter/Render/StateManager.h>

void CBlockTexture::SetClipRect(const RECT &c_rRect)
{
    m_bClipEnable = TRUE;
    m_clipRect = c_rRect;
}

void CBlockTexture::Render(int ix, int iy)
{
    int isx = ix + m_rect.left;
    int isy = iy + m_rect.top;
    int iex = ix + m_rect.left + m_dwWidth;
    int iey = iy + m_rect.top + m_dwHeight;

    float su = 0.0f;
    float sv = 0.0f;
    float eu = 1.0f;
    float ev = 1.0f;

    if (m_bClipEnable)
    {
        if (isx > m_clipRect.right)
            return;
        if (iex < m_clipRect.left)
            return;

        if (isy > m_clipRect.bottom)
            return;
        if (iey < m_clipRect.top)
            return;

        if (m_clipRect.left > isx)
        {
            int const idx = m_clipRect.left - isx;
            isx += idx;
            su += static_cast<float>(idx) / static_cast<float>(m_dwWidth);
        }
        if (iex > m_clipRect.right)
        {
            int const idx = iex - m_clipRect.right;
            iex -= idx;
            eu -= static_cast<float>(idx) / static_cast<float>(m_dwWidth);
        }

        if (m_clipRect.top > isy)
        {
            int const idy = m_clipRect.top - isy;
            isy += idy;
            sv += static_cast<float>(idy) / static_cast<float>(m_dwHeight);
        }
        if (iey > m_clipRect.bottom)
        {
            int const idy = iey - m_clipRect.bottom;
            iey -= idy;
            ev -= static_cast<float>(idy) / static_cast<float>(m_dwHeight);
        }
    }

    TPDTVertex vertices[4];
    vertices[0].position.x = isx - 0.5f;
    vertices[0].position.y = isy - 0.5f;
    vertices[0].position.z = 0.0f;
    vertices[0].texCoord = TTextureCoordinate(su, sv);
    vertices[0].diffuse = 0xffffffff;

    vertices[1].position.x = iex - 0.5f;
    vertices[1].position.y = isy - 0.5f;
    vertices[1].position.z = 0.0f;
    vertices[1].texCoord = TTextureCoordinate(eu, sv);
    vertices[1].diffuse = 0xffffffff;

    vertices[2].position.x = isx - 0.5f;
    vertices[2].position.y = iey - 0.5f;
    vertices[2].position.z = 0.0f;
    vertices[2].texCoord = TTextureCoordinate(su, ev);
    vertices[2].diffuse = 0xffffffff;

    vertices[3].position.x = iex - 0.5f;
    vertices[3].position.y = iey - 0.5f;
    vertices[3].position.z = 0.0f;
    vertices[3].texCoord = TTextureCoordinate(eu, ev);
    vertices[3].diffuse = 0xffffffff;

    if (CGraphicBase::SetPDTStream(vertices, 4))
    {
        CGraphicBase::SetDefaultIndexBuffer(CGraphicBase::DEFAULT_IB_FILL_RECT);

        STATEMANAGER.SetTexture(0, m_lpd3dTexture);
        STATEMANAGER.SetTexture(1, nullptr);
        STATEMANAGER.SetFVF(D3DFVF_XYZ | D3DFVF_TEX1 | D3DFVF_DIFFUSE);
        STATEMANAGER.DrawIndexedPrimitive(D3DPT_TRIANGLELIST, 0, 4, 0, 2);
    }
}

void CBlockTexture::InvalidateRect(const RECT &c_rsrcRect)
{
    RECT const dstRect = m_rect;
    if (c_rsrcRect.right < dstRect.left || c_rsrcRect.left > dstRect.right || c_rsrcRect.bottom < dstRect.top ||
        c_rsrcRect.top > dstRect.bottom)
    {
        Tracef("InvalidateRect() - Strange rect");
        return;
    }

    // DIBBAR_LONGSIZE_BUGFIX
    const RECT clipRect = {
        max(c_rsrcRect.left - dstRect.left, 0),
        max(c_rsrcRect.top - dstRect.top, 0),
        min(c_rsrcRect.right - dstRect.left, dstRect.right - dstRect.left),
        min(c_rsrcRect.bottom - dstRect.top, dstRect.bottom - dstRect.top),
    };
    // END_OF_DIBBAR_LONGSIZE_BUGFIX

    DWORD *pdwSrc = nullptr;
    pdwSrc = static_cast<DWORD *>(m_pDIB->GetPointer());
    pdwSrc += dstRect.left + (dstRect.top * m_pDIB->GetWidth());

    D3DLOCKED_RECT lockedRect;
    if (FAILED(m_lpd3dTexture->LockRect(0, &lockedRect, &clipRect, 0)))
    {
        Tracef("InvalidateRect() - Failed to LockRect");
        return;
    }

    int const iclipWidth = clipRect.right - clipRect.left;
    int const iclipHeight = clipRect.bottom - clipRect.top;
    auto *pdwDst = static_cast<DWORD *>(lockedRect.pBits);
    DWORD const dwDstWidth = lockedRect.Pitch >> 2;
    DWORD const dwSrcWidth = m_pDIB->GetWidth();
    for (int i = 0; i < iclipHeight; ++i)
    {
        for (int i = 0; i < iclipWidth; ++i)
        {
            if (pdwSrc[i])
            {
                pdwDst[i] = pdwSrc[i] | 0xff000000;
            }
            else
            {
                pdwDst[i] = 0;
            }
        }
        pdwDst += dwDstWidth;
        pdwSrc += dwSrcWidth;
    }

    m_lpd3dTexture->UnlockRect(0);
}

bool CBlockTexture::Create(CGraphicDib *pDIB, const RECT &c_rRect, DWORD dwWidth, DWORD dwHeight)
{
    if (FAILED(ms_lpd3dDevice->CreateTexture(dwWidth, dwHeight, 0, 0, D3DFMT_A8R8G8B8, D3DPOOL_MANAGED, &m_lpd3dTexture,
                                             nullptr)))
    {
        Tracef("Failed to create block texture %u, %u\n", dwWidth, dwHeight);
        return false;
    }

    m_pDIB = pDIB;
    m_rect = c_rRect;
    m_dwWidth = dwWidth;
    m_dwHeight = dwHeight;
    m_bClipEnable = FALSE;

    return true;
}

CBlockTexture::CBlockTexture() : m_pDIB(nullptr), m_lpd3dTexture(nullptr)
{
}

CBlockTexture::~CBlockTexture()
{
    safe_release(m_lpd3dTexture);
    m_lpd3dTexture = nullptr;
}
