#include <Neoeter/Render/StdAfx.h>
#include <Neoeter/Render/DibBar.h>
#include <Neoeter/Render/BlockTexture.h>

void CDibBar::Invalidate()
{
    RECT const rect = {0, 0, static_cast<LONG>(m_dwWidth), static_cast<LONG>(m_dwHeight)};

    auto itor = m_kVec_pkBlockTexture.begin();
    for (; itor != m_kVec_pkBlockTexture.end(); ++itor)
    {
        CBlockTexture *pTexture = *itor;
        pTexture->InvalidateRect(rect);
    }
}

void CDibBar::SetClipRect(const RECT &c_rRect)
{
    auto itor = m_kVec_pkBlockTexture.begin();
    for (; itor != m_kVec_pkBlockTexture.end(); ++itor)
    {
        CBlockTexture *pTexture = *itor;
        assert(pTexture);
        if (!pTexture)
            continue;
        pTexture->SetClipRect(c_rRect);
    }
}

void CDibBar::ClearBar()
{
    auto *pdwBuf = static_cast<DWORD *>(m_dib.GetPointer());
    memset(pdwBuf, 0, m_dib.GetWidth() * m_dib.GetHeight() * 4);
    Invalidate();
}

void CDibBar::Render(int ix, int iy)
{
    auto itor = m_kVec_pkBlockTexture.begin();
    for (; itor != m_kVec_pkBlockTexture.end(); ++itor)
    {
        CBlockTexture *pTexture = *itor;
        pTexture->Render(ix, iy);
    }
}

DWORD CDibBar::__NearTextureSize(DWORD dwSize)
{
    if ((dwSize & (dwSize - 1)) == 0)
        return dwSize;

    DWORD dwRet = 2;
    while (dwRet < dwSize)
    {
        dwRet <<= 1;
    }

    return dwRet;
}

void CDibBar::__DivideTextureSize(DWORD dwSize, DWORD dwMax, DWORD *pdwxStep, DWORD *pdwxCount, DWORD *pdwxRest)
{
    if (dwSize < dwMax)
    {
        *pdwxStep = dwMax;
        *pdwxCount = 0;
        *pdwxRest = dwSize % dwMax;
        return;
    }

    *pdwxStep = dwMax;
    *pdwxCount = dwSize / dwMax;
    *pdwxRest = dwSize % dwMax;
}

CBlockTexture *CDibBar::__BuildTextureBlock(DWORD dwxPos, DWORD dwyPos, DWORD dwImageWidth, DWORD dwImageHeight,
                                            DWORD dwTextureWidth, DWORD dwTextureHeight)
{
    if (dwTextureWidth == 0 || dwTextureHeight == 0)
        return nullptr;

    RECT const posRect = {static_cast<LONG>(dwxPos), static_cast<LONG>(dwyPos),
                          static_cast<LONG>(dwxPos + dwImageWidth), static_cast<LONG>(dwyPos + dwImageHeight)};

    auto *pBlockTexture = new CBlockTexture;
    if (!pBlockTexture->Create(&m_dib, posRect, dwTextureWidth, dwTextureHeight))
    {
        delete pBlockTexture;
        return nullptr;
    }

    return pBlockTexture;
}

void CDibBar::__BuildTextureBlockList(DWORD dwWidth, DWORD dwHeight, DWORD dwMax)
{
    DWORD dwxStep = 0;
    DWORD dwyStep = 0;
    DWORD dwxCount = 0;
    DWORD dwyCount = 0;
    DWORD dwxRest = 0;
    DWORD dwyRest = 0;
    __DivideTextureSize(dwWidth, dwMax, &dwxStep, &dwxCount, &dwxRest);
    __DivideTextureSize(dwHeight, dwMax, &dwyStep, &dwyCount, &dwyRest);
    DWORD const dwxTexRest = __NearTextureSize(dwxRest);
    DWORD const dwyTexRest = __NearTextureSize(dwyRest);

    for (DWORD y = 0; y < dwyCount; ++y)
    {
        for (DWORD x = 0; x < dwxCount; ++x)
        {
            CBlockTexture *pTexture = __BuildTextureBlock(x * dwxStep, y * dwyStep, dwxStep, dwyStep, dwMax, dwMax);
            if (pTexture)
                m_kVec_pkBlockTexture.push_back(pTexture);
        }

        CBlockTexture *pTexture =
            __BuildTextureBlock(dwxCount * dwxStep, y * dwyStep, dwxRest, dwyStep, dwxTexRest, dwMax);

        if (pTexture)
            m_kVec_pkBlockTexture.push_back(pTexture);
    }

    for (DWORD x = 0; x < dwxCount; ++x)
    {
        CBlockTexture *pTexture =
            __BuildTextureBlock(x * dwxStep, dwyCount * dwyStep, dwxStep, dwyRest, dwMax, dwyTexRest);
        if (pTexture)
            m_kVec_pkBlockTexture.push_back(pTexture);
    }

    if (dwxRest > 0 && dwyRest > 0)
    {
        CBlockTexture *pTexture =
            __BuildTextureBlock(dwxCount * dwxStep, dwyCount * dwyStep, dwxRest, dwyRest, dwxTexRest, dwyTexRest);
        if (pTexture)
            m_kVec_pkBlockTexture.push_back(pTexture);
    }
}

bool CDibBar::Create(HDC hdc, DWORD dwWidth, DWORD dwHeight)
{
    if (!m_dib.Create(hdc, dwWidth, dwHeight))
    {
        Tracef(" Failed to create CDibBar\n");
        return false;
    }

    m_dwWidth = dwWidth;
    m_dwHeight = dwHeight;

    __BuildTextureBlockList(dwWidth, dwHeight);
    OnCreate();

    return true;
}

CDibBar::CDibBar() = default;
CDibBar::~CDibBar() = default;
