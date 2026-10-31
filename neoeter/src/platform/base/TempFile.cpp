#include <Neoeter/Platform/Base/StdAfx.h>
#include <Neoeter/Platform/Base/TempFile.h>
#include <Neoeter/Platform/Base/Utils.h>
#include <Neoeter/Platform/Base/Debug.h>

CTempFile::~CTempFile()
{
	Destroy();
	DeleteFile(m_szFileName);
}

CTempFile::CTempFile(const char * c_pszPrefix)
{
	strncpy(m_szFileName, CreateTempFileName(c_pszPrefix), MAX_PATH);

	if (!Create(m_szFileName, CFileBase::FILEMODE_WRITE))
	{
		TraceError("CTempFile::CTempFile cannot create temporary file. (filename: %s)", m_szFileName);
		return;
	}
}
