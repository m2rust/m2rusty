#pragma once

#ifdef _MSC_VER
#pragma warning(disable : 4786) // character 255 넘어가는거 끄기
#endif

#if (defined(_WIN32) || defined(_WIN64)) && !defined(USE_PLATFORM_COMPAT)
#include <windows.h>
#else
#include <Platform/Platform.h>
#endif
#include <map>
#include <vector>

#include "Stl.h"

class CMemoryTextFileLoader
{
  public:
    CMemoryTextFileLoader();
    virtual ~CMemoryTextFileLoader();

    void Bind(int bufSize, const void *c_pvBuf);
    DWORD GetLineCount();
    bool CheckLineIndex(DWORD dwLine);
    bool SplitLine(DWORD dwLine, CTokenVector *pstTokenVector, const char *c_szDelimeter = " \t");
    int SplitLine2(DWORD dwLine, CTokenVector *pstTokenVector, const char *c_szDelimeter = " \t");
    bool SplitLineByTab(DWORD dwLine, CTokenVector *pstTokenVector);
    const std::string &GetLineString(DWORD dwLine);

  protected:
    std::vector<std::string> m_stLineVector;
};

class CMemoryFileLoader
{
  public:
    CMemoryFileLoader(int size, const void *c_pvMemoryFile);
    virtual ~CMemoryFileLoader();

    bool Read(int size, void *pvDst);

    int GetPosition() const;
    int GetSize() const;

  protected:
    bool IsReadableSize(int size) const;
    const char *GetCurrentPositionPointer();

    const char *m_pcBase;
    int m_size;
    int m_pos;
};

//////////////////////////////////////////////////////////////////////////////////////////////////
class CDiskFileLoader
{
  public:
    CDiskFileLoader();
    virtual ~CDiskFileLoader();

    void Close();
    bool Open(const char *c_szFileName);
    bool Read(int size, void *pvDst);

    int GetSize() const;

  protected:
    void Initialize();

    FILE *m_fp = nullptr;
    int m_size = 0;
};

using TStringMap = std::map<std::string, std::string>;