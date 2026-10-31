#include <Neoeter/Render/StdAfx.h>
#include <Neoeter/Platform/Base/Stl.h>
#include <Neoeter/Platform/Base/FileLoader.h>
#include <Neoeter/Render/GrpSubImage.h>
#include <Neoeter/Render/ResourceManager.h>

char CGraphicSubImage::m_SearchPath[256] = "D:/Ymir Work/UI/";

CGraphicSubImage::TType CGraphicSubImage::Type()
{
	static TType s_type = StringToType("CGraphicSubImage");
	return s_type;
}

CGraphicSubImage::CGraphicSubImage(const char* c_szFileName) : CGraphicImage(c_szFileName)
{
}

CGraphicSubImage::~CGraphicSubImage()
{
	m_roImage = NULL;
}

bool CGraphicSubImage::CreateDeviceObjects()
{
	m_imageTexture.CreateFromTexturePointer(m_roImage->GetTexturePointer());
	return true;
}

void CGraphicSubImage::SetImagePointer(CGraphicImage* pImage)
{
	m_roImage = pImage;
	CreateDeviceObjects();
}

bool CGraphicSubImage::SetImageFileName(const char* c_szFileName)
{
	CResource* pResource = CResourceManager::Instance().GetResourcePointer(c_szFileName);

	if (!pResource)
	{
		TraceError("[SUBIMAGE] %s: Parent image '%s' could not be found or loaded", GetFileName(), c_szFileName);
		return false;
	}

	if (!pResource->IsType(CGraphicImage::Type()))
	{
		TraceError("[SUBIMAGE] %s: Parent resource '%s' is not CGraphicImage", GetFileName(), c_szFileName);
		return false;
	}

	SetImagePointer(static_cast<CGraphicImage*>(pResource));
	return true;
}

void CGraphicSubImage::SetRectPosition(int left, int top, int right, int bottom)
{
	m_rect.left = left;
	m_rect.top = top;
	m_rect.right = right;
	m_rect.bottom = bottom;
}

void CGraphicSubImage::SetRectReference(const RECT& c_rRect)
{
	m_rect = c_rRect;
}

void CGraphicSubImage::SetSearchPath(const char * c_szFileName)
{
	strncpy(m_SearchPath, c_szFileName, sizeof(m_SearchPath)-1);
}

bool CGraphicSubImage::OnLoad(int iSize, const void* c_pvBuf)
{
	if (!c_pvBuf)
		return false;

	CTokenVector stTokenVector;
	std::map<std::string, std::string> stTokenMap;

	CMemoryTextFileLoader textFileLoader;

	textFileLoader.Bind(iSize, c_pvBuf);

	for (DWORD i = 0; i < textFileLoader.GetLineCount(); ++i)
	{
		if (!textFileLoader.SplitLine(i, &stTokenVector))
			continue;

		if (stTokenVector.size() != 2)
			return false;

		stl_lowers(stTokenVector[0]);
		stl_lowers(stTokenVector[1]);

		stTokenMap[stTokenVector[0]] = stTokenVector[1];
	}

	const std::string& c_rstTitle = stTokenMap["title"];
	const std::string& c_rstVersion = stTokenMap["version"];
	const std::string& c_rstImage = stTokenMap["image"];
	const std::string& c_rstLeft = stTokenMap["left"];
	const std::string& c_rstTop = stTokenMap["top"];
	const std::string& c_rstRight = stTokenMap["right"];
	const std::string& c_rstBottom = stTokenMap["bottom"];

	if (c_rstTitle != "subimage")
		return false;
	
	char szFileName[256];
	if ("2.0"==c_rstVersion)
	{	
		const std::string& c_rstSubFileName=GetFileNameString();
		size_t nPos=c_rstSubFileName.find_last_of("/\\");
		if (nPos!=std::string::npos)
		{
			nPos++;
			memcpy(szFileName, c_rstSubFileName.c_str(), nPos);
			szFileName[nPos]='\0';
			memcpy(szFileName+nPos, c_rstImage.c_str(), c_rstImage.length());
			szFileName[nPos+c_rstImage.length()]='\0';
		}
		else
		{
			memcpy(szFileName, c_rstImage.c_str(), c_rstImage.length());
			szFileName[c_rstImage.length()]='\0';
		}
	}
	else
	{
		_snprintf(szFileName, sizeof(szFileName), "%s%s", m_SearchPath, c_rstImage.c_str());
		if (!CResourceManager::Instance().IsFileExist(szFileName))
		{
			const std::string& c_rstSubFileName=GetFileNameString();
			size_t nPos=c_rstSubFileName.find_last_of("/\\");
			if (nPos!=std::string::npos)
			{
				nPos++;
				memcpy(szFileName, c_rstSubFileName.c_str(), nPos);
				szFileName[nPos]='\0';
				memcpy(szFileName+nPos, c_rstImage.c_str(), c_rstImage.length());
				szFileName[nPos+c_rstImage.length()]='\0';
			}
		}
	}

	if (!SetImageFileName(szFileName))
	{
		TraceError("[SUBIMAGE] %s: Failed to load parent image '%s'", GetFileName(), szFileName);
		return false;
	}

	SetRectPosition(atoi(c_rstLeft.c_str()),
					atoi(c_rstTop.c_str()),
					atoi(c_rstRight.c_str()),
					atoi(c_rstBottom.c_str()));

	return true;
}

void CGraphicSubImage::OnClear()
{
	m_roImage = NULL;
	memset(&m_rect, 0, sizeof(m_rect));
}

bool CGraphicSubImage::OnIsEmpty() const
{
	if (!m_roImage.IsNull())		
		if (!m_roImage->IsEmpty())
			return false;

	return true;
}

bool CGraphicSubImage::OnIsType(TType type)
{
	if (CGraphicSubImage::Type() == type)
		return true;

	return CGraphicImage::OnIsType(type);
}