#include <Neoeter/Platform/Base/StdAfx.h>
#include <Neoeter/Platform/Base/FileBase.h>

#if !defined(_WIN32)
#include <fcntl.h>
#include <unistd.h>
#include <sys/stat.h>
#include <sys/types.h>
#endif

CFileBase::CFileBase() : m_hFile(NULL), m_dwSize(0)
{
}

CFileBase::~CFileBase()
{
	Destroy();
}

char * CFileBase::GetFileName()
{
	return m_filename;
}

void CFileBase::Destroy()
{
	Close();
	m_dwSize = 0;
}

void CFileBase::Close()
{
	if (m_hFile)
	{
#if defined(_WIN32)
		CloseHandle(m_hFile);	
#else
		close((int)(intptr_t)m_hFile);
#endif
		m_hFile = NULL;
	}
}

BOOL CFileBase::Create(const char* filename, EFileMode mode)
{
	Destroy();

	strncpy(m_filename, filename, MAX_PATH);
	m_filename[MAX_PATH] = '\0';

#if defined(_WIN32)
	DWORD dwMode, dwShareMode = FILE_SHARE_READ;

	if (mode == FILEMODE_WRITE)
	{
		dwMode = GENERIC_READ | GENERIC_WRITE;
		dwShareMode = FILE_SHARE_READ | FILE_SHARE_WRITE;
	}
	else
		dwMode = GENERIC_READ;

	m_hFile = CreateFile(filename,					// name of the file
						 dwMode,					// desired access
						 dwShareMode,				// share mode
						 NULL,						// security attributes
						 mode == FILEMODE_READ ? OPEN_EXISTING : OPEN_ALWAYS, // creation disposition
						 FILE_ATTRIBUTE_NORMAL,		// flags and attr
						 NULL);						// template file

	if (m_hFile != INVALID_HANDLE_VALUE)
	{
		m_dwSize = GetFileSize(m_hFile, NULL);
		m_mode = mode;
		return true;
	}

	m_hFile = NULL;
	return false;
#else
	int flags = (mode == FILEMODE_WRITE) ? (O_RDWR | O_CREAT) : O_RDONLY;
	int fd = open(filename, flags, 0644);
	if (fd >= 0)
	{
		struct stat st;
		if (fstat(fd, &st) == 0)
			m_dwSize = (DWORD)st.st_size;
		else
			m_dwSize = 0;

		m_mode = mode;
		m_hFile = (HANDLE)(intptr_t)fd;
		return TRUE;
	}
	m_hFile = NULL;
	return FALSE;
#endif
}

DWORD CFileBase::Size()
{
	return (m_dwSize);
}

void CFileBase::SeekCur(DWORD size)
{
#if defined(_WIN32)
	SetFilePointer(m_hFile, size, NULL, FILE_CURRENT);
#else
	lseek((int)(intptr_t)m_hFile, (off_t)size, SEEK_CUR);
#endif
}

void CFileBase::Seek(DWORD offset)
{
	if (offset > m_dwSize)
		offset = m_dwSize;

#if defined(_WIN32)
	SetFilePointer(m_hFile, offset, NULL, FILE_BEGIN);
#else
	lseek((int)(intptr_t)m_hFile, (off_t)offset, SEEK_SET);
#endif
}

DWORD CFileBase::GetPosition()
{
#if defined(_WIN32)
	return SetFilePointer(m_hFile, 0, NULL, FILE_CURRENT);
#else
	return (DWORD)lseek((int)(intptr_t)m_hFile, 0, SEEK_CUR);
#endif
}

BOOL CFileBase::Write(const void* src, int bytes)
{
#if defined(_WIN32)
	DWORD dwUseless;
	BOOL ret = WriteFile(m_hFile, src, bytes, &dwUseless, NULL);
	
	if (!ret)
		return false;

	m_dwSize = GetFileSize(m_hFile, NULL);
	return true;
#else
	ssize_t written = write((int)(intptr_t)m_hFile, src, bytes);
	if (written < 0)
		return FALSE;

	struct stat st;
	if (fstat((int)(intptr_t)m_hFile, &st) == 0)
		m_dwSize = (DWORD)st.st_size;
	return TRUE;
#endif
}

BOOL CFileBase::Read(void* dest, int bytes)
{
#if defined(_WIN32)
	DWORD dwUseless;
	return ReadFile(m_hFile, dest, bytes, &dwUseless, NULL);
#else
	ssize_t bytesRead = read((int)(intptr_t)m_hFile, dest, bytes);
	return (bytesRead >= 0) ? TRUE : FALSE;
#endif
}

BOOL CFileBase::IsNull()
{
	return !m_hFile ? true : false;
}
