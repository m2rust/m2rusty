#include <Neoeter/Terrain/StdAfx.h>
#include <Neoeter/Terrain/TextureSet.h>

#include <Neoeter/Platform/Base/Debug.h>
#include <Neoeter/Render/ResourceManager.h>
#include <Neoeter/Render/Util.h>
#include <math.h>

#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <print>

CTextureSet::CTextureSet()
{
    Initialize();
}

CTextureSet::~CTextureSet()
{
    Clear();
}

void CTextureSet::Initialize()
{
}

void CTextureSet::Create()
{
    if (CResourceManager::InstancePtr())
    {
        CResource *pResource = CResourceManager::Instance().GetResourcePointer("d:/ymir work/special/error.tga");
        m_ErrorTexture.ImageInstance.SetImagePointer(static_cast<CGraphicImage *>(pResource));
    }
    AddEmptyTexture(); // 지우개 텍스춰를 처음에 추가 해야 함
}

bool CTextureSet::Load(const char *c_szTextureSetFileName, float fTerrainTexCoordBase)
{
    Clear();

    CTokenVectorMap stTokenVectorMap;

    if (!LoadMultipleTextData(c_szTextureSetFileName, stTokenVectorMap))
    {
        TraceError("TextureSet::Load : cannot load %s", c_szTextureSetFileName);
        return false;
    }

    if (!stTokenVectorMap.contains("textureset"))
    {
        TraceError("TextureSet::Load : syntax error, TextureSet (filename: %s)", c_szTextureSetFileName);
        return false;
    }

    if (!stTokenVectorMap.contains("texturecount"))
    {
        TraceError("TextureSet::Load : syntax error, TextureCount (filename: %s)", c_szTextureSetFileName);
        return false;
    }

    Create();

    const std::string &c_rstrCount = stTokenVectorMap["texturecount"][0];

    long const lCount = atol(c_rstrCount.c_str());
    char szTextureName[32 + 1];

    m_Textures.resize(lCount + 1);

    for (long i = 0; i < lCount; ++i)
    {
        _snprintf(szTextureName, sizeof(szTextureName), "texture%03d", i + 1);

        if (!stTokenVectorMap.contains(szTextureName))
            continue;

        const CTokenVector &rVector = stTokenVectorMap[szTextureName];

        const std::string &c_rstrFileName = rVector[0];
        const std::string &c_rstrUScale = rVector[1];
        const std::string &c_rstrVScale = rVector[2];
        const std::string &c_rstrUOffset = rVector[3];
        const std::string &c_rstrVOffset = rVector[4];
        const std::string &c_rstrbSplat = rVector[5];
        const std::string &c_rstrBegin = rVector[6];
        const std::string &c_rstrEnd = rVector[7];

        float fuScale = NAN;
        float fvScale = NAN;
        float fuOffset = NAN;
        float fvOffset = NAN;
        bool bSplat = false;
        unsigned short usBegin = 0;
        unsigned short usEnd = 0;

        fuScale = atof(c_rstrUScale.c_str());
        fvScale = atof(c_rstrVScale.c_str());
        fuOffset = atof(c_rstrUOffset.c_str());
        fvOffset = atof(c_rstrVOffset.c_str());
        bSplat = 0 != atoi(c_rstrbSplat.c_str());
        usBegin = static_cast<unsigned short>(atoi(c_rstrBegin.c_str()));
        usEnd = static_cast<unsigned short>(atoi(c_rstrEnd.c_str()));

        if (!SetTexture(i + 1, c_rstrFileName.c_str(), fuScale, fvScale, fuOffset, fvOffset, bSplat, usBegin, usEnd,
                        fTerrainTexCoordBase))
            TraceError("CTextureSet::Load : SetTexture failed : Filename: %s", c_rstrFileName.c_str());
    }

    m_stFileName.assign(c_szTextureSetFileName);

    return true;
}

void CTextureSet::Clear()
{
    m_ErrorTexture.ImageInstance.Destroy();
    m_Textures.clear();
    Initialize();
}

void CTextureSet::AddEmptyTexture()
{
    TTerrainTexture const eraser;
    m_Textures.push_back(eraser);
}

unsigned long CTextureSet::GetTextureCount()
{
    return m_Textures.size();
}

TTerrainTexture &CTextureSet::GetTexture(unsigned long ulIndex)
{
    if (GetTextureCount() <= ulIndex)
        return m_ErrorTexture;

    return m_Textures[ulIndex];
}

bool CTextureSet::SetTexture(unsigned long ulIndex, const char *c_szFileName, float fuScale, float fvScale,
                             float fuOffset, float fvOffset, bool bSplat, unsigned short usBegin, unsigned short usEnd,
                             float fTerrainTexCoordBase)
{
    if (ulIndex >= m_Textures.size())
    {
        TraceError("CTextureSet::SetTexture : Index Error : Index(%lu) is "
                   "Larger than TextureSet Size(%zu)",
                   ulIndex, m_Textures.size());
        return false;
    }

    TTerrainTexture &tex = m_Textures[ulIndex];
    tex.stFilename = c_szFileName;
    tex.UScale = fuScale;
    tex.VScale = fvScale;
    tex.UOffset = fuOffset;
    tex.VOffset = fvOffset;
    tex.bSplat = bSplat;
    tex.Begin = usBegin;
    tex.End = usEnd;

    if (CResourceManager::InstancePtr())
    {
        CResource *pResource = CResourceManager::Instance().GetResourcePointer(c_szFileName);
        if (pResource)
        {
            if (!pResource->IsType(CGraphicImage::Type()))
            {
                TraceError("CTerrainImpl::GenerateTexture : %s is NOT Image File", pResource->GetFileName());
                return false;
            }
            tex.ImageInstance.SetImagePointer(static_cast<CGraphicImage *>(pResource));
            if (tex.ImageInstance.GetTexturePointer())
                tex.pd3dTexture = tex.ImageInstance.GetTexturePointer()->GetD3DTexture();
        }
    }

    D3DXMatrixScaling(&tex.m_matTransform, fTerrainTexCoordBase * tex.UScale, -fTerrainTexCoordBase * tex.VScale, 0.0f);
    tex.m_matTransform._41 = tex.UOffset;
    tex.m_matTransform._42 = -tex.VOffset;
    return true;
}

void CTextureSet::Reload(float fTerrainTexCoordBase)
{
    for (DWORD dwIndex = 1; dwIndex < GetTextureCount(); ++dwIndex)
    {
        TTerrainTexture &tex = m_Textures[dwIndex];

        if (CResourceManager::InstancePtr())
        {
            tex.ImageInstance.ReloadImagePointer(
                (CGraphicImage *)CResourceManager::Instance().GetResourcePointer(tex.stFilename.c_str()));
            if (tex.ImageInstance.GetTexturePointer())
                tex.pd3dTexture = tex.ImageInstance.GetTexturePointer()->GetD3DTexture();
        }

        D3DXMatrixScaling(&tex.m_matTransform, fTerrainTexCoordBase * tex.UScale, -fTerrainTexCoordBase * tex.VScale,
                          0.0f);
        tex.m_matTransform._41 = tex.UOffset;
        tex.m_matTransform._42 = -tex.VOffset;
    }
}

bool CTextureSet::AddTexture(const char *c_szFileName, float fuScale, float fvScale, float fuOffset, float fvOffset,
                             bool bSplat, unsigned short usBegin, unsigned short usEnd, float fTerrainTexCoordBase)
{
    if (GetTextureCount() >= 256)
    {
        LogBox("You cannot add any more textures.", "Maximum number of textures 255");
        return false;
    }

    for (unsigned long i = 1; i < GetTextureCount(); ++i)
    {
        if (c_szFileName == m_Textures[i].stFilename)
        {
            LogBox("There is already a texture with the same name.", "Duplicate");
            return false;
        }
    }

    if (CResourceManager::InstancePtr())
    {
        CResource *pResource = CResourceManager::Instance().GetResourcePointer(c_szFileName);
        if (pResource && !pResource->IsType(CGraphicImage::Type()))
        {
            LogBox("CTerrainImpl::GenerateTexture : Not an image file. %s", pResource->GetFileName());
            return false;
        }
    }

    m_Textures.resize(m_Textures.size() + 1);

    SetTexture(m_Textures.size() - 1, c_szFileName, fuScale, fvScale, fuOffset, fvOffset, bSplat, usBegin, usEnd,
               fTerrainTexCoordBase);

    return true;
}

bool CTextureSet::RemoveTexture(unsigned long ulIndex)
{
    if (GetTextureCount() <= ulIndex)
        return false;

    auto itor = m_Textures.begin() + ulIndex;
    m_Textures.erase(itor);
    return true;
}

bool CTextureSet::Save(const char *c_pszFileName)
{
    FILE *pFile = fopen(c_pszFileName, "w");

    if (!pFile)
        return false;

    std::println(pFile, "TextureSet");
    std::println(pFile, "");

    std::println(pFile, "TextureCount {}",
                 static_cast<long>(GetTextureCount() - 1)); // -1 을 하는 이유는 지우개 때문임
    std::println(pFile, "");

    for (DWORD i = 1; i < GetTextureCount(); ++i)
    {
        TTerrainTexture &rTex = m_Textures[i];

        std::println(pFile, "Start Texture{:03}", i);
        std::println(pFile, "    \"{}\"", rTex.stFilename);
        std::println(pFile, "    {:f}", rTex.UScale);
        std::println(pFile, "    {:f}", rTex.VScale);
        std::println(pFile, "    {:f}", rTex.UOffset);
        std::println(pFile, "    {:f}", rTex.VOffset);
        std::println(pFile, "    {}", rTex.bSplat ? 1 : 0);
        std::println(pFile, "    {}", rTex.Begin);
        std::println(pFile, "    {}", rTex.End);
        std::println(pFile, "End Texture{:03}", i);
    }

    fclose(pFile);
    return true;
}
