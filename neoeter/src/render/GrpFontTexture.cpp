#include <Neoeter/Render/StdAfx.h>
#include <Neoeter/Render/GrpText.h>
#include <Neoeter/Platform/Base/Stl.h>

#include <Neoeter/Render/Util.h>

#if defined(OPENMT2_PLATFORM_LINUX)
#include <ft2build.h>
#include FT_FREETYPE_H
#include <Neoeter/Render/FontLocator.h>

static FT_Library s_ftLibrary = nullptr;

static FT_Face GetFTFace(const char* fontName, int fontSize)
{
	if (!s_ftLibrary)
	{
		if (FT_Init_FreeType(&s_ftLibrary) != 0)
			return nullptr;
	}

	static std::map<std::pair<std::string, int>, FT_Face> s_faceCache;
	std::pair<std::string, int> key(fontName ? fontName : "", fontSize);
	auto it = s_faceCache.find(key);
	if (it != s_faceCache.end())
		return it->second;

	std::string path = FontLocator_FindFontFile(fontName);
	if (path.empty())
		return nullptr;

	FT_Face face = nullptr;
	if (FT_New_Face(s_ftLibrary, path.c_str(), 0, &face) != 0)
		return nullptr;

	FT_Set_Pixel_Sizes(face, 0, fontSize > 0 ? fontSize : 12);
	s_faceCache[key] = face;
	return face;
}
#endif

CGraphicFontTexture::CGraphicFontTexture()
{
	Initialize();
}

CGraphicFontTexture::~CGraphicFontTexture()
{
	Destroy();
}

void CGraphicFontTexture::Initialize()
{
	CGraphicTexture::Initialize();
	m_hFontOld = NULL;
	m_hFont = NULL;
	m_isDirty = false;
	m_bItalic = false;
}

bool CGraphicFontTexture::IsEmpty() const
{
	return m_fontMap.size() == 0;
}

void CGraphicFontTexture::Destroy()
{
	HDC hDC = m_dib.GetDCHandle();
	if (hDC)
		SelectObject(hDC, m_hFontOld);

	m_dib.Destroy();

	m_lpd3dTexture = NULL;
	CGraphicTexture::Destroy();
	stl_wipe(m_pFontTextureVector);
	m_charInfoMap.clear();

	if (m_fontMap.size())
	{
		TFontMap::iterator i = m_fontMap.begin();

		while(i != m_fontMap.end())
		{
			DeleteObject((HGDIOBJ)i->second);
			++i;
		}

		m_fontMap.clear();
	}
	
	Initialize();
}

bool CGraphicFontTexture::CreateDeviceObjects()
{
	return true;
}

void CGraphicFontTexture::DestroyDeviceObjects()
{
}

bool CGraphicFontTexture::Create(const char* c_szFontName, int fontSize, bool bItalic)
{
	Destroy();
	
	strncpy(m_fontName, c_szFontName, sizeof(m_fontName)-1);
	m_fontSize	= fontSize;
	m_bItalic	= bItalic;

	m_x = 0;
	m_y = 0;
	m_step = 0;

	DWORD width = 256,height = 256;
	if (GetMaxTextureWidth() > 512)
		width = 512;
	if (GetMaxTextureHeight() > 512)
		height = 512;
	
	if (!m_dib.Create(ms_hDC, width, height))
		return false;

	HDC hDC = m_dib.GetDCHandle();

	m_hFont = GetFont(GetDefaultCodePage());

	m_hFontOld=(HFONT)SelectObject(hDC, m_hFont);
	SetTextColor(hDC, RGB(255, 255, 255));
	SetBkColor(hDC,	0);

	if (!AppendTexture())
		return false;
	
	return true;
}



HFONT CGraphicFontTexture::GetFont(WORD codePage)
{
	HFONT hFont = NULL;
	TFontMap::iterator i = m_fontMap.find(codePage);

	if(i != m_fontMap.end())
	{
		hFont = i->second;
	}
	else
	{
		LOGFONT logFont;

		memset(&logFont, 0, sizeof(LOGFONT));

		logFont.lfHeight			= m_fontSize;
		logFont.lfEscapement		= 0;
		logFont.lfOrientation		= 0;
		logFont.lfWeight			= FW_NORMAL;
		logFont.lfItalic			= (BYTE) m_bItalic;
		logFont.lfUnderline			= FALSE;
		logFont.lfStrikeOut			= FALSE;
		logFont.lfCharSet			= GetCharsetFromCodePage(codePage);
		logFont.lfOutPrecision		= OUT_DEFAULT_PRECIS;
		logFont.lfClipPrecision		= CLIP_DEFAULT_PRECIS;
		logFont.lfQuality			= ANTIALIASED_QUALITY;
		logFont.lfPitchAndFamily	= DEFAULT_PITCH;
		//Tracenf("font: %s", GetFontFaceFromCodePage(codePage));
		strcpy(logFont.lfFaceName, m_fontName); //GetFontFaceFromCodePage(codePage));
		//strcpy(logFont.lfFaceName, GetFontFaceFromCodePage(codePage));

		hFont = CreateFontIndirect(&logFont);

		m_fontMap.insert(TFontMap::value_type(codePage, hFont));
	}

	return hFont;
}

bool CGraphicFontTexture::AppendTexture()
{
	CGraphicImageTexture * pNewTexture = new CGraphicImageTexture;

	if (!pNewTexture->Create(m_dib.GetWidth(), m_dib.GetHeight(), D3DFMT_A4R4G4B4))
	{
		delete pNewTexture;
		return false;
	}

	m_pFontTextureVector.push_back(pNewTexture);
	return true;
}

bool CGraphicFontTexture::UpdateTexture()
{
	if(!m_isDirty)
		return true;

	m_isDirty = false;

	CGraphicImageTexture * pFontTexture = m_pFontTextureVector.back();

	if (!pFontTexture)
		return false;

	WORD* pwDst;
	int pitch;

	if (!pFontTexture->Lock(&pitch, (void**)&pwDst))
		return false;

	pitch /= 2;

	int width = m_dib.GetWidth();
	int height = m_dib.GetHeight();

	DWORD * pdwSrc = (DWORD*)m_dib.GetPointer();

	for (int y = 0; y < height; ++y, pwDst += pitch, pdwSrc += width)
		for (int x = 0; x < width; ++x)
			pwDst[x]=pdwSrc[x];
	
	pFontTexture->Unlock();
	return true;
}

CGraphicFontTexture::TCharacterInfomation* CGraphicFontTexture::GetCharacterInfomation(WORD codePage, wchar_t keyValue)
{
	TCharacterKey code(codePage, keyValue);

	TCharacterInfomationMap::iterator f = m_charInfoMap.find(code);

	if (m_charInfoMap.end() == f)
	{
		return UpdateCharacterInfomation(code);
	}
	else
	{
		return &f->second;	
	}
}

CGraphicFontTexture::TCharacterInfomation* CGraphicFontTexture::UpdateCharacterInfomation(TCharacterKey code)
{
	wchar_t keyValue = code.second;

	if (keyValue == 0x08)
		keyValue = L' '; // 탭은 공백으로 바꾼다 (아랍 출력시 탭 사용: NAME:\tTEXT -> TEXT\t:NAME 로 전환됨 )

#if defined(OPENMT2_PLATFORM_LINUX)
	FT_Face face = GetFTFace(m_fontName, m_fontSize);
	if (face)
	{
		FT_Error err = FT_Load_Char(face, keyValue, FT_LOAD_RENDER);
		(void)err;
		FT_GlyphSlot slot = face->glyph;
		FT_Bitmap& bmp = slot->bitmap;

		int advance = (slot->advance.x >> 6);
		if (advance <= 0) advance = slot->bitmap.width + 1;
		if (advance <= 0) advance = m_fontSize / 2;

		int cellWidth = advance;
		if (slot->bitmap_left + (int)bmp.width > cellWidth)
			cellWidth = slot->bitmap_left + bmp.width + 1;
		if (cellWidth <= 0) cellWidth = m_fontSize / 2;

		int lineHeight = (face->size->metrics.height >> 6);
		if (lineHeight <= 0) lineHeight = m_fontSize + 4;

		int ascender = (face->size->metrics.ascender >> 6);
		if (ascender <= 0) ascender = m_fontSize;

		int width = m_dib.GetWidth();
		int height = m_dib.GetHeight();

		if (m_x + cellWidth >= (width - 1))
		{
			m_y += (m_step + 1);
			m_step = 0;
			m_x = 0;

			if (m_y + lineHeight >= (height - 1))
			{
				if (!UpdateTexture())
					return NULL;

				if (!AppendTexture())
					return NULL;

				m_y = 0;
				std::memset(m_dib.GetPointer(), 0, (size_t)width * (size_t)height * 4);
			}
		}

		DWORD* pdwDIBData = (DWORD*)m_dib.GetPointer();
		int nDIBWidth = m_dib.GetWidth();

		int top = ascender - slot->bitmap_top;
		int left = slot->bitmap_left;
		if (left < 0) left = 0;

		for (int r = 0; r < (int)bmp.rows; ++r)
		{
			int dstY = m_y + top + r;
			if (dstY < 0 || dstY >= height) continue;

			for (int c = 0; c < (int)bmp.width; ++c)
			{
				int dstX = m_x + left + c;
				if (dstX < 0 || dstX >= width) continue;

				uint8_t alpha = bmp.buffer[r * bmp.pitch + c];
				if (alpha > 32)
				{
					uint8_t a4 = (alpha + 8) / 17;
					if (a4 > 0)
						pdwDIBData[dstY * nDIBWidth + dstX] = (a4 << 12) | 0x0FFF;
				}
			}
		}

		float rhwidth = 1.0f / float(width);
		float rhheight = 1.0f / float(height);

		TCharacterInfomation& rNewCharInfo = m_charInfoMap[code];

		rNewCharInfo.index = m_pFontTextureVector.size() - 1;
		rNewCharInfo.width = cellWidth;
		rNewCharInfo.height = lineHeight;
		rNewCharInfo.left = float(m_x) * rhwidth;
		rNewCharInfo.top = float(m_y) * rhheight;
		rNewCharInfo.right = float(m_x + cellWidth) * rhwidth;
		rNewCharInfo.bottom = float(m_y + lineHeight) * rhheight;
		rNewCharInfo.advance = (float)advance;

		m_x += cellWidth;

		if (m_step < lineHeight)
			m_step = lineHeight;

		m_isDirty = true;

		return &rNewCharInfo;
	}
#endif

	HDC hDC = m_dib.GetDCHandle();
	SelectObject(hDC, GetFont(code.first));

	ABCFLOAT	stABC;
	SIZE		size;

	if (!GetTextExtentPoint32W(hDC, &keyValue, 1, &size) || !GetCharABCWidthsFloatW(hDC, keyValue, keyValue, &stABC))
		return NULL;

	size.cx = stABC.abcfB;
	if( stABC.abcfA > 0.0f )
		size.cx += ceilf(stABC.abcfA);
	if( stABC.abcfC > 0.0f )
		size.cx += ceilf(stABC.abcfC);
	size.cx++;

	LONG lAdvance = ceilf( stABC.abcfA + stABC.abcfB + stABC.abcfC );

	int width = m_dib.GetWidth();
	int height = m_dib.GetHeight();

	if (m_x + size.cx >= (width - 1))
	{
		m_y += (m_step + 1);
		m_step = 0;
		m_x = 0;

		if (m_y + size.cy >= (height - 1))
		{
			if (!UpdateTexture())
			{
				return NULL;
			}

			if (!AppendTexture())
				return NULL;

			m_y = 0;
		}
	}

	TextOutW(hDC, m_x, m_y, &keyValue, 1);
		
	int nChrX;
	int nChrY;
	int nChrWidth = size.cx;
	int nChrHeight = size.cy;
	int nDIBWidth = m_dib.GetWidth();

	
	DWORD*pdwDIBData=(DWORD*)m_dib.GetPointer();		
	DWORD*pdwDIBBase=pdwDIBData+nDIBWidth*m_y+m_x;
	DWORD*pdwDIBRow;
	
	pdwDIBRow=pdwDIBBase;
	for (nChrY=0; nChrY<nChrHeight; ++nChrY, pdwDIBRow+=nDIBWidth)
	{			
		for (nChrX=0; nChrX<nChrWidth; ++nChrX)
		{
			pdwDIBRow[nChrX]=(pdwDIBRow[nChrX]&0xff) ? 0xffff : 0;
		}
	}

	float rhwidth = 1.0f / float(width);
	float rhheight = 1.0f / float(height);

	TCharacterInfomation& rNewCharInfo = m_charInfoMap[code];

	rNewCharInfo.index = m_pFontTextureVector.size() - 1;
	rNewCharInfo.width = size.cx;
	rNewCharInfo.height = size.cy;
	rNewCharInfo.left = float(m_x) * rhwidth;
	rNewCharInfo.top = float(m_y) * rhheight;
	rNewCharInfo.right = float(m_x+size.cx) * rhwidth;
	rNewCharInfo.bottom = float(m_y+size.cy) * rhheight;
	rNewCharInfo.advance = (float) lAdvance;

	m_x += size.cx;

	if (m_step < size.cy)
		m_step = size.cy;	

	m_isDirty = true;

	return &rNewCharInfo;
}

bool CGraphicFontTexture::CheckTextureIndex(DWORD dwTexture)
{
	if (dwTexture >= m_pFontTextureVector.size())
		return false;

	return true;
}

void CGraphicFontTexture::SelectTexture(DWORD dwTexture)
{
	assert(CheckTextureIndex(dwTexture));
	m_lpd3dTexture = m_pFontTextureVector[dwTexture]->GetD3DTexture();
}
