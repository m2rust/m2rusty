#include <Neoeter/Platform/Base/StdAfx.h>

#include <Neoeter/Platform/Base/CPostIt.h>
#include <Neoeter/Platform/Base/Utils.h>

class _CPostItMemoryBlock
{
  public:
    _CPostItMemoryBlock();
    ~_CPostItMemoryBlock();

    BOOL Assign(HANDLE hBlock);
    HANDLE CreateHandle();
    BOOL DestroyHandle();

    LPSTR Find(LPCSTR lpszKeyName);
    BOOL Put(LPCSTR lpBuffer);
    BOOL Put(LPCSTR lpszKeyName, LPCSTR lpBuffer);
    BOOL Get(LPCSTR lpszKeyName, LPSTR lpBuffer, DWORD nSize);

  protected:
    using StrList = std::list<CHAR *>;
    using StrListItr = StrList::iterator;

    HANDLE m_hHandle;
    StrList m_StrList;
};

CPostIt::CPostIt(LPCSTR szAppName) : m_pMemoryBlock(nullptr)
{
    Init(szAppName);
}

CPostIt::~CPostIt()
{
    Destroy();
}

BOOL CPostIt::Init(LPCSTR szAppName)
{
    if (szAppName == nullptr || !*szAppName)
    {
        strcpy(m_szClipFormatName, "YMCF");
    }
    else
    {
        strcpy(m_szClipFormatName, "YMCF_");
        strcat(m_szClipFormatName, szAppName);
    }
    return TRUE;
}

BOOL CPostIt::CopyTo(CPostIt *pPostIt, LPCSTR lpszKeyName)
{
    if (m_pMemoryBlock == nullptr)
        return FALSE;
    LPSTR szText = m_pMemoryBlock->Find(lpszKeyName);
    if (szText == nullptr)
        return FALSE;
    return pPostIt->Set(szText);
}

BOOL CPostIt::Flush()
{
    if (m_bModified == FALSE)
        return FALSE;
    if (m_pMemoryBlock == nullptr)
        return FALSE;

#ifdef _WIN32
    UINT uDGPFormat;

    uDGPFormat = ::RegisterClipboardFormatA(m_szClipFormatName);
    if (::OpenClipboard(NULL) == FALSE)
        return FALSE;
    if (::SetClipboardData(uDGPFormat, m_pMemoryBlock->CreateHandle()) == NULL)
    {
        //		DWORD dwLastError = ::GetLastError();
        m_pMemoryBlock->DestroyHandle();
        ::CloseClipboard();
        m_bModified = FALSE;
        return FALSE;
    }
    ::CloseClipboard();
#endif

    m_bModified = FALSE;
    return TRUE;
}

void CPostIt::Empty()
{
    SAFE_DELETE(m_pMemoryBlock);

#ifdef _WIN32
    UINT uDGPFormat;

    uDGPFormat = ::RegisterClipboardFormatA(m_szClipFormatName);
    if (::OpenClipboard(NULL) == FALSE)
        return;
    HANDLE hClipboardMemory = ::GetClipboardData(uDGPFormat);
    if (hClipboardMemory)
    {
        //		::GlobalFree( hClipboardMemory );
        ::SetClipboardData(uDGPFormat, NULL);
    }
    ::CloseClipboard();
#endif

    m_bModified = FALSE;
}

void CPostIt::Destroy()
{
    Flush();
    SAFE_DELETE(m_pMemoryBlock);
}

BOOL CPostIt::Set(LPCSTR lpszKeyName, LPCSTR lpBuffer)
{
    if (m_pMemoryBlock == nullptr)
        m_pMemoryBlock = new _CPostItMemoryBlock;
    m_pMemoryBlock->Put(lpszKeyName, lpBuffer);
    m_bModified = TRUE;
    return TRUE;
}

BOOL CPostIt::Set(LPCSTR lpszKeyName, DWORD dwValue)
{
    CHAR szValue[12];

    _snprintf(szValue, 12, "%d", dwValue);
    return Set(lpszKeyName, szValue);
}

BOOL CPostIt::Set(LPCSTR lpBuffer)
{
    if (lpBuffer == nullptr)
        return FALSE;
    if (m_pMemoryBlock == nullptr)
        m_pMemoryBlock = new _CPostItMemoryBlock;
    m_pMemoryBlock->Put(lpBuffer);
    m_bModified = TRUE;
    return TRUE;
}

BOOL CPostIt::Get(LPCSTR lpszKeyName, LPSTR lpBuffer, DWORD nSize)
{
    if (m_pMemoryBlock == nullptr)
    {
#ifdef _WIN32
        UINT uDGPFormat;

        uDGPFormat = ::RegisterClipboardFormatA(m_szClipFormatName);
        if (::OpenClipboard(NULL) == FALSE)
            return FALSE;

        HANDLE hClipboardMemory = ::GetClipboardData(uDGPFormat);

        if (hClipboardMemory == NULL)
        {
            ::CloseClipboard();
            return FALSE;
        }
        m_pMemoryBlock = new _CPostItMemoryBlock;
        m_pMemoryBlock->Assign(hClipboardMemory);

        ::CloseClipboard();
#else
        return FALSE;
#endif
    }

    return m_pMemoryBlock->Get(lpszKeyName, lpBuffer, nSize);
}

_CPostItMemoryBlock::_CPostItMemoryBlock() : m_hHandle(nullptr)
{
}

_CPostItMemoryBlock::~_CPostItMemoryBlock()
{
    for (auto itr = m_StrList.begin(); itr != m_StrList.end();)
    {
        LPSTR lpszText = *itr;
        SAFE_DELETE_ARRAY(lpszText);
        itr = m_StrList.erase(itr);
    }
}

BOOL _CPostItMemoryBlock::Assign(HANDLE hBlock)
{
    if (hBlock == nullptr || hBlock == INVALID_HANDLE_VALUE)
        return FALSE;
    auto *lpBuffer = static_cast<LPBYTE>(::GlobalLock(hBlock));

    if (lpBuffer == nullptr)
        return FALSE;
    DWORD const dwCount = *(reinterpret_cast<LPDWORD>(lpBuffer));
    lpBuffer += sizeof(DWORD);
    for (DWORD dwI = 0; dwI < dwCount; dwI++)
    {
        WORD const wLen = *(reinterpret_cast<LPWORD>(lpBuffer));
        lpBuffer += sizeof(WORD);

        auto *lpszText = new CHAR[wLen + 1];
        ::CopyMemory(lpszText, lpBuffer, wLen);
        lpszText[wLen] = '\0';

        lpBuffer += wLen;

        Put(lpszText);
    }
    ::GlobalUnlock(hBlock);
    return TRUE;
}

HANDLE _CPostItMemoryBlock::CreateHandle()
{
    if (m_StrList.empty())
        return INVALID_HANDLE_VALUE;
    DWORD dwBlockSize = sizeof(DWORD);
    StrListItr itr;

    // Calculation for Memory Block Size
    for (itr = m_StrList.begin(); itr != m_StrList.end(); ++itr)
    {
        dwBlockSize += sizeof(WORD);
        dwBlockSize += static_cast<DWORD>(strlen(*itr));
    }

    HANDLE hBlock = ::GlobalAlloc(GMEM_ZEROINIT | GMEM_MOVEABLE, dwBlockSize);
    if (hBlock == nullptr)
        return INVALID_HANDLE_VALUE;
    auto *lpBuffer = static_cast<LPBYTE>(::GlobalLock(hBlock));
    if (lpBuffer == nullptr)
    {
        ::GlobalFree(hBlock);
        return INVALID_HANDLE_VALUE;
    }
    *(reinterpret_cast<LPDWORD>(lpBuffer)) = static_cast<DWORD>(m_StrList.size());
    lpBuffer += sizeof(DWORD);
    for (itr = m_StrList.begin(); itr != m_StrList.end(); ++itr)
    {
        *(reinterpret_cast<LPWORD>(lpBuffer)) = static_cast<WORD>(strlen(*itr));
        lpBuffer += sizeof(WORD);
        ::CopyMemory(lpBuffer, *itr, strlen(*itr));
        lpBuffer += strlen(*itr);
    }
    ::GlobalUnlock(hBlock);

    m_hHandle = hBlock;
    return hBlock;
}

BOOL _CPostItMemoryBlock::DestroyHandle()
{
    ::GlobalFree(m_hHandle);
    m_hHandle = nullptr;
    return TRUE;
}

LPSTR _CPostItMemoryBlock::Find(LPCSTR lpszKeyName)
{
    for (auto *lpszText : m_StrList)
    {
        if (_strnicmp(lpszText, lpszKeyName, strlen(lpszKeyName)) != 0)
            continue;
        if (*(lpszText + strlen(lpszKeyName)) != '=')
            continue;
        return lpszText;
    }
    return nullptr;
}

BOOL _CPostItMemoryBlock::Put(LPCSTR lpszKeyName, LPCSTR lpBuffer)
{
    LPSTR lpszText = nullptr;

    if ((lpszText = Find(lpszKeyName)) != nullptr)
    {
        for (auto itr = m_StrList.begin(); itr != m_StrList.end(); ++itr)
        {
            if (lpszText == *itr)
            {
                SAFE_DELETE_ARRAY(lpszText);
                m_StrList.erase(itr);
                break;
            }
        }
    }
    if (lpBuffer == nullptr || !*lpBuffer)
        return TRUE;
    size_t const nStrLen = strlen(lpszKeyName) + 1 /* '=' */ + strlen(lpBuffer);
    lpszText = new CHAR[nStrLen + 1];
    ::CopyMemory(lpszText, lpszKeyName, strlen(lpszKeyName));
    *(lpszText + strlen(lpszKeyName)) = '=';
    ::CopyMemory(lpszText + strlen(lpszKeyName) + 1, lpBuffer, strlen(lpBuffer));
    *(lpszText + nStrLen) = '\0';

    m_StrList.push_back(lpszText);
    return TRUE;
}

BOOL _CPostItMemoryBlock::Put(LPCSTR lpBuffer)
{
    LPSTR lpszText = nullptr;

    if (lpBuffer == nullptr || !*lpBuffer)
        return TRUE;
    size_t const nStrLen = strlen(lpBuffer);
    lpszText = new CHAR[nStrLen + 1];
    ::CopyMemory(lpszText, lpBuffer, nStrLen);
    *(lpszText + nStrLen) = '\0';

    m_StrList.push_back(lpszText);
    return TRUE;
}

BOOL _CPostItMemoryBlock::Get(LPCSTR lpszKeyName, LPSTR lpBuffer, DWORD nSize)
{
    LPSTR lpszText = Find(lpszKeyName);
    if (lpszText == nullptr)
        return FALSE;
    lpszText += (strlen(lpszKeyName) + 1);
    ::ZeroMemory(lpBuffer, nSize);
    strncpy(lpBuffer, lpszText, (nSize < strlen(lpszText)) ? nSize : strlen(lpszText));
    return TRUE;
}
