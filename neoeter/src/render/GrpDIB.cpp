#include <Neoeter/Render/StdAfx.h>
#include <Neoeter/Render/GrpDIB.h>

CGraphicDib::CGraphicDib()
{
	Initialize();
}

CGraphicDib::~CGraphicDib()
{
	Destroy();
}

void CGraphicDib::Initialize()
{
	m_hDC=NULL;
	m_hBmp=NULL;
	m_pvBuf=NULL;
	m_width=0;
	m_height=0;
}

void CGraphicDib::Destroy()
{			
	if (m_hBmp) DeleteObject(m_hBmp);			
	if (m_hDC)	DeleteDC(m_hDC);
	
	Initialize();			
}

bool CGraphicDib::Create(HDC hDC, int width, int height)
{
	Destroy();

	m_width = width;
	m_height = height;

	ZeroMemory(&m_bmi.bmiHeader,  sizeof(BITMAPINFOHEADER));
	m_bmi.bmiHeader.biSize        = sizeof(BITMAPINFOHEADER);
	m_bmi.bmiHeader.biWidth       =  m_width;
	m_bmi.bmiHeader.biHeight      = -m_height;
	m_bmi.bmiHeader.biPlanes      = 1;			
	m_bmi.bmiHeader.biBitCount    = 32;
	m_bmi.bmiHeader.biCompression = BI_RGB;

	m_hDC=CreateCompatibleDC(hDC);
	if (!m_hDC)
	{
		assert(!"CGraphicDib::Create CreateCompatibleDC Error");
		return false;
	}

	m_hBmp=CreateDIBSection(m_hDC, &m_bmi, DIB_RGB_COLORS, &m_pvBuf, NULL, 0);
	if (!m_hBmp)
	{
		assert(!"CGraphicDib::Create CreateDIBSection Error");
		return false;
	}

	SelectObject(m_hDC, m_hBmp);

	::SetTextColor(m_hDC, RGB(255, 255, 255));

	return true;
}

HDC	CGraphicDib::GetDCHandle()
{
	return m_hDC;
}

void CGraphicDib::SetBkMode(int iBkMode)
{
	::SetBkMode(m_hDC, iBkMode);
}

#if defined(OPENMT2_PLATFORM_LINUX)
#include <ft2build.h>
#include FT_FREETYPE_H
#include <Neoeter/Render/FontLocator.h>

static FT_Library s_dibFtLib = nullptr;
static FT_Face s_dibFtFace = nullptr;

static void EnsureDIBFT()
{
	if (!s_dibFtLib) {
		FT_Init_FreeType(&s_dibFtLib);
	}
	if (!s_dibFtFace && s_dibFtLib) {
		std::string path = FontLocator_FindFontFile("sans-serif");
		if (!path.empty() && FT_New_Face(s_dibFtLib, path.c_str(), 0, &s_dibFtFace) == 0)
			FT_Set_Pixel_Sizes(s_dibFtFace, 0, 12);
	}
}
#endif

void CGraphicDib::TextOut(int ix, int iy, const char * c_szText)
{
#if defined(OPENMT2_PLATFORM_LINUX)
	EnsureDIBFT();
	if (s_dibFtFace && m_pvBuf && c_szText)
	{
		uint32_t* pdw = (uint32_t*)m_pvBuf;
		int curX = ix;
		int ascender = s_dibFtFace->size->metrics.ascender >> 6;
		for (size_t i = 0; c_szText[i]; ++i)
		{
			unsigned char ch = (unsigned char)c_szText[i];
			if (FT_Load_Char(s_dibFtFace, ch, FT_LOAD_RENDER) != 0) continue;
			FT_GlyphSlot slot = s_dibFtFace->glyph;
			FT_Bitmap& bmp = slot->bitmap;
			int top = ascender - slot->bitmap_top;
			int left = slot->bitmap_left;
			if (left < 0) left = 0;

			for (int r = 0; r < (int)bmp.rows; ++r) {
				int dstY = iy + top + r;
				if (dstY < 0 || dstY >= m_height) continue;
				for (int c = 0; c < (int)bmp.width; ++c) {
					int dstX = curX + left + c;
					if (dstX < 0 || dstX >= m_width) continue;
					uint8_t a = bmp.buffer[r * bmp.pitch + c];
					if (a > 32) {
						pdw[dstY * m_width + dstX] = 0xFFFFFFFF;
					}
				}
			}
			int adv = slot->advance.x >> 6;
			curX += (adv > 0) ? adv : (slot->bitmap.width + 1);
		}
		return;
	}
#endif
	::SetBkColor(m_hDC, 0);	
	::TextOut(m_hDC, ix, iy, c_szText, strlen(c_szText));
}

void CGraphicDib::Put(HDC hDC, int x, int y)
{
	SetDIBitsToDevice(
	  hDC,     
	  x,       
	  y,       
	  m_width, 
	  m_height,
	  0,
	  0,
	  0,
	  m_height,
	  m_pvBuf,
	  &m_bmi,
	  DIB_RGB_COLORS
	);
}

void* CGraphicDib::GetPointer()
{
	return m_pvBuf;
}

int CGraphicDib::GetWidth()
{
	return m_width;
}

int CGraphicDib::GetHeight()
{
	return m_height;
}