#include <Neoeter/Terrain/StdAfx.h>
#include <Neoeter/Terrain/Terrain.h>

#include <Neoeter/Platform/Base/Timer.h>
#include <Neoeter/Platform/Base/Debug.h>
#include <Neoeter/Platform/Base/Utils.h>
#include <Neoeter/IO/EterPackManager.h>

#include <cmath>
#include <cstring>
#include <sstream>
#include <algorithm>

//////////////////////////////////////////////////////////////////////////
// Texture Set
//////////////////////////////////////////////////////////////////////////
CTextureSet * CTerrainImpl::ms_pTextureSet = NULL;

void CTerrainImpl::SetTextureSet(CTextureSet * pTextureSet)
{
	static CTextureSet s_EmptyTextureSet;

	if (!pTextureSet)
		ms_pTextureSet = &s_EmptyTextureSet;
	else
		ms_pTextureSet = pTextureSet;
}

CTextureSet * CTerrainImpl::GetTextureSet()
{
	if (!ms_pTextureSet)
		SetTextureSet(NULL);

	return ms_pTextureSet;
}

CTerrainImpl::CTerrainImpl()
{
	Initialize();
}

CTerrainImpl::~CTerrainImpl()
{
	Clear();
}

void CTerrainImpl::Initialize()
{
	memset(m_lWaterHeight, -1, sizeof(m_lWaterHeight));

	m_byNumWater = 0;
	memset(&m_HeightMapHeader, 0, sizeof(TGA_HEADER));
	memset(&m_awShadowMap, 0xFFFF, sizeof(m_awShadowMap));
	memset(&m_lpAlphaTexture, 0, sizeof(m_lpAlphaTexture));
	
	m_lViewRadius = 0;

	m_wTileMapVersion = 8976;

	m_fHeightScale = 0.0f;
	
	m_lpShadowTexture = NULL;

	m_lSplatTilesX = 0;
	m_lSplatTilesY = 0;	
}

void CTerrainImpl::Clear()
{
	for (DWORD i = 0; i < GetTextureSet()->GetTextureCount(); ++i)
	{
		if (m_lpAlphaTexture[i])
		{
			m_lpAlphaTexture[i]->Release();
			m_lpAlphaTexture[i] = NULL;
		}
	}
	
	Initialize();
}

bool CTerrainImpl::LoadHeightMap(const char * c_szFileName)
{
	Tracef("LoadRawHeightMapFile %s ", c_szFileName);
	
	std::stringstream data;
	if (!CEterPackManager::Instance().Get(c_szFileName, data))
	{
		Tracen("Error");
		TraceError("CTerrainImpl::LoadHeightMap - %s OPEN ERROR", c_szFileName);
		return false;
	}
	
	data.read((char*) m_awRawHeightMap, sizeof(WORD)*HEIGHTMAP_RAW_XSIZE*HEIGHTMAP_RAW_YSIZE);
	if (!data)
	{
		// Could not read data
		TraceError("CTerrainImpl::LoadHeightMap - %s READ ERROR", c_szFileName);
		return false;
	}

	// Data read successfully
	return true;
}

bool CTerrainImpl::LoadAttrMap(const char * c_szFileName)
{
	DWORD dwStart = ELTimer_GetMSec();
	Tracef("LoadAttrMapFile %s ", c_szFileName);

	std::stringstream data;
	if (!CEterPackManager::Instance().Get(c_szFileName, data))
	{
		TraceError("CTerrainImpl::LoadAttrMap - %s OPEN ERROR", c_szFileName);
		return false;
	}

	// LoadAttrMap
	{
#pragma pack(push)
#pragma pack(1)
		struct SAttrMapHeader
		{
			WORD m_wMagic;
			WORD m_wWidth;
			WORD m_wHeight;
		};
#pragma pack(pop)

		SAttrMapHeader kAttrMapHeader;
		data.read((char*) &kAttrMapHeader, sizeof(kAttrMapHeader));
		if (!data)
		{
			TraceError(" CTerrainImpl::LoadAttrMap - %s FILE SIZE ERROR", c_szFileName);
			return false;
		}

		const WORD c_wAttrMapMagic = 2634;
		if (c_wAttrMapMagic != kAttrMapHeader.m_wMagic)
		{
			TraceError("CTerrainImpl::LoadAttrMap - %s MAGIC NUMBER(%d!=MAGIC[%d]) ERROR", c_szFileName, kAttrMapHeader.m_wMagic, c_wAttrMapMagic);
			return false;
		}

		if (ATTRMAP_XSIZE != kAttrMapHeader.m_wWidth)
		{
			TraceError("CTerrainImpl::LoadAttrMap - kAttrMapHeader(%s).m_width(%d)!=ATTRMAP_XSIZE(%d)", c_szFileName, kAttrMapHeader.m_wWidth, ATTRMAP_XSIZE);
			return false;
		}

		if (ATTRMAP_YSIZE != kAttrMapHeader.m_wHeight)
		{
			TraceError("CTerrainImpl::LoadAttrMap - kAttrMapHeader(%s).m_height(%d)!=ATTRMAP_YSIZE(%d)", c_szFileName, kAttrMapHeader.m_wHeight, ATTRMAP_YSIZE);
			return false;
		}

		data.read((char*)m_abyAttrMap, sizeof(m_abyAttrMap));
		if (!data)
		{
			TraceError("CTerrainImpl::LoadAttrMap - %s FILE DATA SIZE ERROR", c_szFileName);
			return false;
		}
	}

	Tracef("%d\n", ELTimer_GetMSec() - dwStart);
	return true;
}

bool CTerrainImpl::RAW_LoadTileMap(const char * c_szFileName)
{
	Tracef("LoadSplatFile %s ", c_szFileName);

	std::stringstream data;
	if (!CEterPackManager::Instance().Get(c_szFileName, data))
	{
		Tracen("Error");
		TraceError("CTerrainImpl::RAW_LoadTileMap - %s OPEN ERROR", c_szFileName);
		return false;
	}

	data.read((char*)m_abyTileMap, sizeof(BYTE) * (TILEMAP_RAW_XSIZE) * (TILEMAP_RAW_YSIZE));
	if (!data)
	{
		// Could not read data
		TraceError("CTerrainImpl::RAW_LoadTileMap - %s READ ERROR", c_szFileName);
		return false;
	}

	// Data read successfully
	return true;
}

bool CTerrainImpl::LoadWaterMap(const char * c_szFileName)
{	
	DWORD dwStart = ELTimer_GetMSec();

	if (!LoadWaterMapFile(c_szFileName))
	{
		memset(m_abyWaterMap, 0xFF, sizeof(m_abyWaterMap));

		m_byNumWater = 0;
		memset(m_lWaterHeight, -1, sizeof(m_lWaterHeight));
		
		TraceError("CMapOutdoor::LoadWaterMap LoadWaterMapFile(%s) Failed", c_szFileName);

		return false;
	}

	Tracef("LoadWaterMapFile %s %d\n", c_szFileName, ELTimer_GetMSec() - dwStart);
	return true;
}

bool CTerrainImpl::LoadWaterMapFile(const char * c_szFileName)
{	
	std::stringstream data;
	size_t fileSize = 0;
	if (!CEterPackManager::Instance().Get(c_szFileName, data))
	{
		Tracen("Error");
		TraceError("CTerrainImpl::LoadWaterMap - %s OPEN ERROR", c_szFileName);
		return false;
	}

	fileSize = data.str().length();

	{
#pragma pack(push)
#pragma pack(1)
		struct SWaterMapHeader
		{
			WORD m_wMagic;
			WORD m_wWidth;
			WORD m_wHeight;
			BYTE m_byLayerCount;
		};
#pragma pack(pop)
		
		SWaterMapHeader kWaterMapHeader;
		data.read((char*)&kWaterMapHeader, sizeof(kWaterMapHeader));
		if (!data)
		{
			TraceError("CTerrainImpl::LoadWaterMap - %s FILE SIZE ERROR", c_szFileName);
			return false;
		}

		const WORD c_wWaterMapMagic = 5426;
		
		if (c_wWaterMapMagic != kWaterMapHeader.m_wMagic)
		{
			TraceError("CTerrainImpl::LoadWaterMap - %s MAGIC NUMBER(%d!=MAGIC[%d]) ERROR", c_szFileName, kWaterMapHeader.m_wMagic, c_wWaterMapMagic);
			return false;
		}	
		
		if (WATERMAP_XSIZE != kWaterMapHeader.m_wWidth)
		{
			TraceError("CTerrainImpl::LoadWaterMap - kWaterMapHeader(%s).m_width(%d)!=WATERMAP_XSIZE(%d)", c_szFileName, kWaterMapHeader.m_wWidth, WATERMAP_XSIZE);
			return false;
		}

		if (WATERMAP_YSIZE != kWaterMapHeader.m_wHeight)
		{
			TraceError("CTerrainImpl::LoadWaterMap - kWaterMapHeader(%s).m_height(%d)!=WATERMAP_YSIZE(%d)", c_szFileName, kWaterMapHeader.m_wHeight, WATERMAP_YSIZE);
			return false;
		}

		m_byNumWater = kWaterMapHeader.m_byLayerCount;

		DWORD dwFileRestSize = fileSize - sizeof(kWaterMapHeader);
		DWORD dwFileNeedSize = sizeof(m_abyWaterMap) + sizeof(LONG) * m_byNumWater;
		DWORD dwFileNeedSize2 = sizeof(m_abyWaterMap) + sizeof(WORD) * m_byNumWater;
		if (dwFileRestSize == dwFileNeedSize2)
		{
			WORD wWaterHeight[MAX_WATER_NUM + 1];
			
			data.read((char*)m_abyWaterMap, sizeof(m_abyWaterMap));
			if (!data)
				return false;

			m_byNumWater = MIN(static_cast<BYTE>(MAX_WATER_NUM), m_byNumWater);
			if (m_byNumWater)
			{
				data.read((char*)wWaterHeight, sizeof(WORD) * m_byNumWater);
				if (!data)
					return false;

				for (int i = 0; i < m_byNumWater; ++i)
					m_lWaterHeight[i] = wWaterHeight[i];
			}
		}
		else if (dwFileRestSize != dwFileNeedSize)
		{
			TraceError("CTerrainImpl::LoadWaterMap - %s FILE DATA SIZE(rest %d != need %d) ERROR", c_szFileName, dwFileRestSize, dwFileNeedSize);
			return false;
		}
	
		data.read((char*)m_abyWaterMap, sizeof(m_abyWaterMap));
		if (!data)
			return false;

		if (m_byNumWater) {
			data.read((char*)m_lWaterHeight, sizeof(LONG)* m_byNumWater);
			if (!data)
				return false;
		}
	}

	return true;
}

DWORD CTerrainImpl::GetShadowMapColor(float fx, float fy)
{
	float fMapSize = (float)(TERRAIN_XSIZE);
	float fooMapSize = 1.0f / fMapSize;
	if (fx < 0 || fy < 0 || fx >= fMapSize || fy >= fMapSize)
		return 0xFFFFFFFF;

	fx = fx * fooMapSize * (float)(SHADOWMAP_XSIZE - 1);
	fy = fy * fooMapSize * (float)(SHADOWMAP_YSIZE - 1);
	int ix, iy;
	PR_FLOAT_TO_INT(fx, ix);
	PR_FLOAT_TO_INT(fy, iy);

	WORD w = *(m_awShadowMap + (iy * SHADOWMAP_XSIZE) + ix);

	int b = w & 0x1f; w >>= 5; b <<= 3;
	int g = w & 0x1f; w >>= 5; g <<= 3;
	int r = w & 0x1f;		   r <<= 3;

	return (DWORD)((0xffU << 24) | (static_cast<DWORD>(g) << 16) | (static_cast<DWORD>(g) << 8) | static_cast<DWORD>(r));
}
