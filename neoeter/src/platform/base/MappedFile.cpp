#include <Neoeter/Platform/Base/StdAfx.h>

#include <Neoeter/Platform/Base/Debug.h>
#include <Neoeter/Platform/Base/MappedFile.h>

#ifndef _WIN32
#include <algorithm>
#include <cerrno>
#include <sys/mman.h>
#include <unistd.h>
#endif

CMappedFile::CMappedFile()
    : m_pbBufLinkData(nullptr), m_pbAppendResultDataBlock(nullptr), m_hFM(nullptr), m_lpMapData(nullptr),
      m_lpData(nullptr), m_pLZObj(nullptr)
{
}

CMappedFile::~CMappedFile()
{
    Destroy();
}

BOOL CMappedFile::Create(const char *filename)
{
    Destroy();
    return CFileBase::Create(filename, FILEMODE_READ);
}

BOOL CMappedFile::Create(const char *filename, const void **dest, int offset, int size)
{
    if (!CMappedFile::Create(filename))
        return FALSE;

    int const ret = Map(dest, offset, size);
    return (ret) > 0;
}

LPCVOID CMappedFile::Get()
{
    return m_lpData;
}

void CMappedFile::Link(DWORD dwBufSize, const void *c_pvBufData)
{
    m_dwBufLinkSize = dwBufSize;
    m_pbBufLinkData = (BYTE *)c_pvBufData;
}

void CMappedFile::BindLZObject(CLZObject *pLZObj)
{
    assert(m_pLZObj == nullptr);
    m_pLZObj = pLZObj;

    Link(m_pLZObj->GetSize(), m_pLZObj->GetBuffer());
}

void CMappedFile::BindLZObjectWithBufferedSize(CLZObject *pLZObj)
{
    assert(m_pLZObj == nullptr);
    m_pLZObj = pLZObj;

    Link(m_pLZObj->GetBufferSize(), m_pLZObj->GetBuffer());
}

BYTE *CMappedFile::AppendDataBlock(const void *pBlock, DWORD dwBlockSize)
{

    delete[] m_pbAppendResultDataBlock;

    // realloc
    m_dwAppendResultDataSize = m_dwBufLinkSize + dwBlockSize;
    m_pbAppendResultDataBlock = new BYTE[m_dwAppendResultDataSize];

    memcpy(m_pbAppendResultDataBlock, m_pbBufLinkData, m_dwBufLinkSize);
    memcpy(m_pbAppendResultDataBlock + m_dwBufLinkSize, pBlock, dwBlockSize);

    // redirect
    Link(m_dwAppendResultDataSize, m_pbAppendResultDataBlock);

    return m_pbAppendResultDataBlock;
}

void CMappedFile::Destroy()
{
    if (m_pLZObj) // 압축된 데이터가 이 포인터로 연결 된다
    {
        delete m_pLZObj;
        m_pLZObj = nullptr;
    }

    if (nullptr != m_lpMapData)
    {
        Unmap(m_lpMapData);
        m_lpMapData = nullptr;
    }

#ifdef _WIN32
    if (NULL != m_hFM)
    {
        CloseHandle(m_hFM);
        m_hFM = NULL;
    }
#endif

    if (m_pbAppendResultDataBlock)
    {
        delete[] m_pbAppendResultDataBlock;
        m_pbAppendResultDataBlock = nullptr;
    }

    m_dwAppendResultDataSize = 0;

    m_pbBufLinkData = nullptr;
    m_dwBufLinkSize = 0;

    m_seekPosition = 0;
    m_dataOffset = 0;
    m_mapSize = 0;

    CFileBase::Destroy();
}

int CMappedFile::Seek(DWORD offset, int iSeekType)
{
    switch (iSeekType)
    {
    case SEEK_TYPE_BEGIN:
        offset = std::min(offset, m_dwSize);

        m_seekPosition = offset;
        break;

    case SEEK_TYPE_CURRENT:
        m_seekPosition = std::min(m_seekPosition + offset, Size());
        break;

    case SEEK_TYPE_END:
        m_seekPosition = (offset >= Size()) ? 0 : (Size() - offset);
        break;
    }

    return m_seekPosition;
}

int CMappedFile::Map(const void **dest, int offset, int size)
{
    m_dataOffset = offset;

    if (size == 0)
    {
        m_mapSize = m_dwSize;
    }
    else
    {
        m_mapSize = size;
    }

    if (m_dataOffset + m_mapSize > m_dwSize)
        return 0;

#ifdef _WIN32
    SYSTEM_INFO SysInfo;
    GetSystemInfo(&SysInfo);
    DWORD dwSysGran = SysInfo.dwAllocationGranularity;
    DWORD dwFileMapStart = (m_dataOffset / dwSysGran) * dwSysGran;
    DWORD dwMapViewSize = (m_dataOffset % dwSysGran) + m_mapSize;
    INT iViewDelta = m_dataOffset - dwFileMapStart;

    m_hFM = CreateFileMapping(m_hFile,                  // handle
                              NULL,                     // security
                              PAGE_READONLY,            // flProtect
                              0,                        // high
                              m_dataOffset + m_mapSize, // low
                              NULL);                    // name

    if (!m_hFM)
    {
        OutputDebugString("CMappedFile::Map !m_hFM\n");
        return 0;
    }

    m_lpMapData = MapViewOfFile(m_hFM, FILE_MAP_READ, 0, dwFileMapStart, dwMapViewSize);

    if (!m_lpMapData)
    {
        TraceError("CMappedFile::Map !m_lpMapData %lu", GetLastError());
        return 0;
    }

    m_lpData = (char *)m_lpMapData + iViewDelta;
#else
    long pageSize = sysconf(_SC_PAGE_SIZE);
    if (pageSize <= 0)
        pageSize = 4096;

    DWORD const dwFileMapStart = (m_dataOffset / static_cast<DWORD>(pageSize)) * static_cast<DWORD>(pageSize);
    DWORD const dwMapViewSize = (m_dataOffset % static_cast<DWORD>(pageSize)) + m_mapSize;
    int const iViewDelta = m_dataOffset - dwFileMapStart;

    int const fd = static_cast<int>((intptr_t)m_hFile);
    void *mapped = mmap(nullptr, dwMapViewSize, PROT_READ, MAP_SHARED, fd, static_cast<off_t>(dwFileMapStart));
    if (mapped == MAP_FAILED)
    {
        TraceError("CMappedFile::Map mmap failed: %s", strerror(errno));
        return 0;
    }

    m_lpMapData = mapped;
    m_lpData = static_cast<char *>(m_lpMapData) + iViewDelta;
#endif

    *dest = static_cast<char *>(m_lpData);
    m_seekPosition = 0;

    Link(m_mapSize, m_lpData);

    return (m_mapSize);
}

BYTE *CMappedFile::GetCurrentSeekPoint()
{
    return m_pbBufLinkData + m_seekPosition;
}

DWORD CMappedFile::Size() const
{
    return m_dwBufLinkSize;
}

DWORD CMappedFile::GetPosition() const
{
    return m_dataOffset;
}

BOOL CMappedFile::Read(void *dest, int bytes)
{
    if (m_seekPosition + bytes > Size())
        return FALSE;

    memcpy(dest, GetCurrentSeekPoint(), bytes);
    m_seekPosition += bytes;
    return TRUE;
}

DWORD CMappedFile::GetSeekPosition() const
{
    return m_seekPosition;
}

void CMappedFile::Unmap(LPCVOID data)
{
#ifdef _WIN32
    UnmapViewOfFile(data);
#else
    if (m_lpMapData)
    {
        long pageSize = sysconf(_SC_PAGE_SIZE);
        if (pageSize <= 0)
            pageSize = 4096;
        DWORD const dwMapViewSize = (m_dataOffset % static_cast<DWORD>(pageSize)) + m_mapSize;
        munmap(m_lpMapData, dwMapViewSize);
        m_lpMapData = nullptr;
    }
#endif
}
