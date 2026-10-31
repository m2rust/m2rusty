#include <Neoeter/Platform/Base/StdAfx.h>
#include <Neoeter/Platform/Base/FileDir.h>
#include <string>

#if !defined(_WIN32)
#include <filesystem>
namespace fs = std::filesystem;
#endif

CDir::CDir()
{
	Initialize();
}

CDir::~CDir()
{
	Destroy();
}

void CDir::Destroy()
{
#if defined(_WIN32)
	if (m_hFind)
		FindClose(m_hFind);
#endif

	Initialize();
}

bool CDir::Create(const char * c_szFilter, const char* c_szPath, BOOL bCheckedExtension)
{
	Destroy();
	
	std::string stPath = c_szPath;

#if defined(_WIN32)
	if (stPath.length())
	{
		char end = stPath[stPath.length() - 1];

		if (end != '\\')
			stPath+='\\';
	}

	std::string stQuery;
	stQuery += stPath;
	stQuery += "*.*";
	
	m_wfd.dwFileAttributes |= FILE_ATTRIBUTE_DIRECTORY;
	m_hFind = FindFirstFile(stQuery.c_str(), &m_wfd);

	if (m_hFind == INVALID_HANDLE_VALUE)
		return true;

	do
	{
		if (*m_wfd.cFileName == '.')
			continue;

		if (IsFolder())	
		{
			if (!OnFolder(c_szFilter, stPath.c_str(), m_wfd.cFileName))
				return false;
		}
		else
		{
			const char * c_szExtension = strchr(m_wfd.cFileName, '.');
			if (!c_szExtension)
				continue;

			if (bCheckedExtension)
			{
				std::string strFilter = c_szFilter;
				int iPos = strFilter.find_first_of(';', 0);
				if (iPos > 0)
				{
					std::string strFirstFilter = std::string(c_szFilter).substr(0, iPos);
					std::string strSecondFilter = std::string(c_szFilter).substr(iPos+1, strlen(c_szFilter));
					if (0 != strFirstFilter.compare(c_szExtension+1) && 0 != strSecondFilter.compare(c_szExtension+1))
						continue;
				}
				else
				{
					if (0 != stricmp(c_szExtension+1, c_szFilter))
						continue;
				}
			}

			if (!OnFile(stPath.c_str(), m_wfd.cFileName))
				return false;
		}
	} 			
	while (FindNextFile(m_hFind, &m_wfd));

	return true;
#else
	if (!stPath.empty())
	{
		char end = stPath.back();
		if (end != '/' && end != '\\')
			stPath += '/';
	}

	std::string dirPath = stPath.empty() ? "." : stPath;
	std::error_code ec;
	fs::directory_iterator it(dirPath, ec);
	if (ec)
		return true;

	for (const auto& entry : fs::directory_iterator(dirPath, ec))
	{
		std::string filename = entry.path().filename().string();
		if (filename.empty() || filename[0] == '.')
			continue;

		strncpy(m_wfd.cFileName, filename.c_str(), MAX_PATH);
		m_wfd.cFileName[MAX_PATH - 1] = '\0';

		bool isDir = entry.is_directory(ec);
		if (isDir)
			m_wfd.dwFileAttributes = FILE_ATTRIBUTE_DIRECTORY;
		else
			m_wfd.dwFileAttributes = FILE_ATTRIBUTE_NORMAL;

		if (isDir)
		{
			if (!OnFolder(c_szFilter, stPath.c_str(), m_wfd.cFileName))
				return false;
		}
		else
		{
			const char * c_szExtension = strrchr(m_wfd.cFileName, '.');
			if (!c_szExtension)
				continue;

			if (bCheckedExtension)
			{
				std::string strFilter = c_szFilter;
				size_t iPos = strFilter.find_first_of(';', 0);
				if (iPos != std::string::npos)
				{
					std::string strFirst = strFilter.substr(0, iPos);
					std::string strSecond = strFilter.substr(iPos + 1);
					if (strcasecmp(strFirst.c_str(), c_szExtension + 1) != 0 &&
					    strcasecmp(strSecond.c_str(), c_szExtension + 1) != 0)
						continue;
				}
				else
				{
					if (strcasecmp(c_szExtension + 1, c_szFilter) != 0)
						continue;
				}
			}

			if (!OnFile(stPath.c_str(), m_wfd.cFileName))
				return false;
		}
	}
	return true;
#endif
}

bool CDir::IsFolder()
{
	if (m_wfd.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY)
		return true;

	return false;
}

void CDir::Initialize()
{
	m_hFind = NULL;
	memset(&m_wfd, 0, sizeof(m_wfd));
}
