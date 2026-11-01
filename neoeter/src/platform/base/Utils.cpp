#include <Neoeter/Platform/Base/StdAfx.h>

#include <cstdlib>
#ifdef _WIN32
#include <direct.h>
#include <io.h>
#else
#include <math.h>
#include <sys/stat.h>
#include <unistd.h>
#endif
#include <Neoeter/Platform/Base/FileDir.h>
#include <Neoeter/Platform/Base/Utils.h>
#include <cassert>

#include <print>

char korean_tolower(char c);

const char *CreateTempFileName(const char *c_pszPrefix)
{
    static char szTempName[MAX_PATH + 1];

#ifdef _WIN32
    char szTempPath[MAX_PATH + 1];
    GetTempPath(MAX_PATH, szTempPath);

    GetTempFileName(szTempPath,                        // directory for temp files
                    c_pszPrefix ? c_pszPrefix : "etb", // temp file name prefix
                    c_pszPrefix ? true : false,        // create unique name
                    szTempName);                       // buffer for name
#else
    const char *tmpDir = getenv("TMPDIR");
    if (!tmpDir)
        tmpDir = "/tmp";
    snprintf(szTempName, sizeof(szTempName), "%s/%s_XXXXXX", tmpDir, c_pszPrefix ? c_pszPrefix : "etb");
    int const fd = mkstemp(szTempName);
    if (fd >= 0)
        close(fd);
#endif

    return (szTempName);
}

void GetFilePathNameExtension(const char *c_szFile, int len, std::string *pstPath, std::string *pstName,
                              std::string *pstExt)
{
    assert(pstPath != nullptr);
    assert(pstName != nullptr);
    assert(pstExt != nullptr);

    int ext = len;
    int pos = len;

    while (pos > 0)
    {
        --pos;
        char const c = c_szFile[pos];

        if (ext == len && c == '.')
        {
            ext = pos;
            break;
        }

        if (c == '/' || c == '\\')
            break;
    }

    while (pos > 0)
    {
        --pos;
        char const c = c_szFile[pos];

        if (c == '/' || c == '\\')
            break;
    }

    if (pos)
    {
        ++pos;
        pstPath->append(c_szFile, pos);
    }

    if (ext > pos)
        pstName->append(c_szFile + pos, ext - pos);

    ++ext;

    if (len > ext)
        pstExt->append(c_szFile + ext, len - ext);
}

void GetFileExtension(const char *c_szFile, int len, std::string *pstExt)
{
    int ext = len;
    int pos = len;
    while (pos > 0)
    {
        --pos;
        char const c = c_szFile[pos];
        if (ext == len && c == '.')
        {
            ext = pos;
            break;
        }

        if (c == '/')
        {
            break;
        }
        if (c == '\\')
            break;
    }

    ++ext;
    if (len > ext)
        pstExt->append(c_szFile + ext, len - ext);
}

void GetFileNameParts(const char *c_szFile, int len, char *pszPath, char *pszName, char *pszExt)
{
    assert(pszPath != nullptr);
    assert(pszName != nullptr);
    assert(pszExt != nullptr);

    int ext = len;
    int pos = len;
    while (pos > 0)
    {
        --pos;
        char const c = c_szFile[pos];
        if (ext == len && c == '.')
        {
            ext = pos;
            break;
        }

        if (c == '/')
        {
            break;
        }
        if (c == '\\')
            break;
    }

    while (pos > 0)
    {
        --pos;
        char const c = c_szFile[pos];

        if (c == '/')
        {
            break;
        }
        if (c == '\\')
            break;
    }

    if (pos)
    {
        ++pos;
        for (int i = 0; i < pos; ++i)
        {
            pszPath[i] = c_szFile[i];
        }
        pszPath[pos] = '\0';
    }

    if (ext > pos)
    {
        int count = 0;
        for (int i = pos; i < ext; ++i)
        {
            pszName[count++] = c_szFile[i];
        }
        pszName[count] = '\0';
    }

    ++ext;
    if (len > ext)
    {
        int count = 0;
        for (int i = ext; i < len; ++i)
        {
            pszExt[count++] = c_szFile[i];
        }
        pszExt[count] = '\0';
    }
}

void GetOldIndexingName(char *szName, int Index)
{
    char Temp[512];
    snprintf(Temp, sizeof(Temp), "%d", Index);
    strcat(szName, Temp);
}

void GetIndexingName(char *szName, DWORD Index)
{
    sprintf(szName + strlen(szName), "%u", Index);
}

void GetOnlyFileName(const char *sz_Name, std::string &strFileName)
{
    strFileName = "";

    int i = 0;
    for (i = strlen(sz_Name) - 1; i >= 0; --i)
    {
        if ('\\' == sz_Name[i] || '/' == sz_Name[i])
        {
            ++i;
            break;
        }
    }

    if (i == -1)
        i = 0;

    for (size_t j = i; j < strlen(sz_Name); ++j)
    {
        strFileName += sz_Name[j];
    }

    strFileName += "\0";
}

void GetExceptionPathName(const char *sz_Name, std::string &OnlyFileName)
{
    GetOnlyFileName(sz_Name, OnlyFileName);
}

void GetOnlyPathName(const char *sz_Name, std::string &OnlyPathName)
{
    int i = 0;
    for (i = strlen(sz_Name) - 1; i >= 0; --i)
    {
        if ('\\' == sz_Name[i] || '/' == sz_Name[i])
        {
            ++i;
            break;
        }
    }

    if (i == -1)
        i = 0;

    OnlyPathName.reserve(strlen(sz_Name));
    OnlyPathName = "";

    for (int j = 0; j < i; ++j)
    {
        OnlyPathName += sz_Name[j];
    }
    OnlyPathName += "\0";
}

const char *GetOnlyPathName(const char *c_szName)
{
    static std::string strPathName;
    GetOnlyPathName(c_szName, strPathName);

    return strPathName.c_str();
}

bool GetLocalFileName(const char *c_szGlobalPath, const char *c_szFullPathFileName, std::string *pstrLocalFileName)
{
    std::string const strLocalFileName;

    std::string strGlobalPath;
    std::string strFullPathFileName;
    StringPath(c_szGlobalPath, strGlobalPath);
    StringPath(c_szFullPathFileName, strFullPathFileName);

    if (strGlobalPath.length() >= strFullPathFileName.length())
        return false;

    DWORD const length = std::min(strGlobalPath.length(), strFullPathFileName.length());
    for (DWORD dwPos = 0; dwPos < length; ++dwPos)
    {
        if (strGlobalPath[dwPos] != strFullPathFileName[dwPos])
            return false;
    }

    *pstrLocalFileName = &c_szFullPathFileName[length];

    return true;
}

void GetWorkingFolder(std::string &strFileName)
{
    char buf[128 + 1];
    _getcwd(buf, 128);
    strcat(buf, "/");
    strFileName = buf;
}

void StringLowers(char *String)
{
    for (DWORD i = 0; i < strlen(String); ++i)
    {
        String[i] = korean_tolower(String[i]);
    }
}

void StringPath(std::string &rString)
{
    for (DWORD i = 0; i < rString.length(); ++i)
    {
        if (rString[i] == '\\')
        {
            rString[i] = '/';
        }
        else
        {
            rString[i] = korean_tolower(rString[i]);
        }
    }
}

void StringPath(char *pString)
{
    for (DWORD i = 0; i < strlen(pString); ++i)
    {
        if (pString[i] == '\\')
        {
            pString[i] = '/';
        }
        else
        {
            pString[i] = korean_tolower(pString[i]);
        }
    }
}

void StringPath(const char *c_szSrc, char *szDest)
{
    for (DWORD i = 0; i < strlen(c_szSrc); ++i)
    {
        if (c_szSrc[i] == '\\')
        {
            szDest[i] = '/';
        }
        else
        {
            szDest[i] = korean_tolower(c_szSrc[i]);
        }
    }
}

void StringPath(const char *c_szSrc, std::string &rString)
{
    rString = "";
    rString.resize(strlen(c_szSrc));

    for (DWORD i = 0; i < strlen(c_szSrc); ++i)
    {
        if (c_szSrc[i] == '\\')
        {
            rString[i] = '/';
        }
        else
        {
            rString[i] = korean_tolower(c_szSrc[i]);
        }
    }
}

#define ishprint(x) ((((x) & 0xE0) > 0x90) || isprint(x))

void PrintAsciiData(const void *void_data, int bytes)
{
    int i = 0;
    int j = 0;
    int k = 0;
    const unsigned char *p = nullptr;
    const unsigned char *data = nullptr;

    data = static_cast<const unsigned char *>(void_data);

    std::println(stdout, "------------------------------------------------------------------");
    j = bytes;
    while (true)
    {
        k = j >= 16 ? 16 : j;

        p = data;
        for (i = 0; i < 16; ++i)
        {
            if (i >= k)
            {
                std::print(stdout, "   ");
            }
            else
            {
                std::print(stdout, "{:02x} ", *p);
            }
            p++;
        }

        std::print(stdout, "| ");

        p = data;
        for (i = 0; i < k; ++i)
        {
            if (i >= k)
            {
                std::print(stdout, " ");
            }
            else
            {
                std::print(stdout, "{:c}", ishprint(*p) ? *p : '.');
            }
            p++;
        }

        std::println(stdout, "");

        j -= 16;
        data += 16;

        if (j <= 0)
            break;
    }

    std::println(stdout, "------------------------------------------------------------------");
}

int MIN(int a, int b)
{
    return a < b ? a : b;
}

int MAX(int a, int b)
{
    return a > b ? a : b;
}

int MINMAX(int min, int value, int max)
{
    if (max < min)
        return MAX(min, value);

    int tv = 0;
    tv = (min > value ? min : value);
    return (max < tv) ? max : tv;
}

float fMIN(float a, float b)
{
    return a < b ? a : b;
}

float fMAX(float a, float b)
{
    return a > b ? a : b;
}

float fMINMAX(float min, float value, float max)
{
    float tv = NAN;

    tv = (min > value ? min : value);
    return (max < tv) ? max : tv;
}

bool IsFile(const char *filename)
{
    return _access(filename, 0) == 0;
}

bool IsGlobalFileName(const char *c_szFileName)
{
    return strchr(c_szFileName, ':') != nullptr;
}

void MyCreateDirectory(const char *path)
{
    if (!path || !*path)
        return;

    char *dir = nullptr;
    const char *p = nullptr;

    if (strlen(path) >= 3)
    {
        if (*(path + 1) == ':') // C:, D: 같은 경우를 체크
            path += 3;
    }

    p = path;

    int const len = strlen(path) + 1;
    dir = new char[len];

    while (*p)
    {
        if (*p == '/' || *p == '\\')
        {
            memset(dir, 0, len);
            strncpy(dir, path, p - path);
#ifdef _WIN32
            CreateDirectory(dir, NULL);
#else
            mkdir(dir, 0755);
#endif
        }

        ++p;
    }

    delete[] dir;
}

class CDirRemover : public CDir
{
  public:
    CDirRemover() = default;
    ~CDirRemover() override = default;

    bool OnFolder(const char *c_szFilter, const char *c_szPathName, const char *c_szFileName) override
    {
        std::string strFullPathName;
        strFullPathName = c_szPathName;
        strFullPathName += c_szFileName;

        CDirRemover remover;
        remover.Create(c_szFilter, strFullPathName.c_str());

        std::string strWorkingFolder;
        GetWorkingFolder(strWorkingFolder);
        strWorkingFolder += strFullPathName;
        strWorkingFolder += "/";
        StringPath(strWorkingFolder);
        ms_strDirectoryDeque.push_back(strWorkingFolder);
        return true;
    }
    bool OnFile(const char *c_szPathName, const char *c_szFileName) override
    {
        std::string strFullPathName;
        strFullPathName = c_szPathName;
        strFullPathName += c_szFileName;
#ifdef _WIN32
        _chmod(strFullPathName.c_str(), _S_IWRITE);
        DeleteFile(strFullPathName.c_str());
#else
        chmod(strFullPathName.c_str(), 0644);
        unlink(strFullPathName.c_str());
#endif
        return true;
    }

    static void RemoveAllDirectory()
    {
        for (auto &c_rstrDirectory : ms_strDirectoryDeque)
        {

#ifdef _WIN32
            RemoveDirectory(c_rstrDirectory.c_str());
#else
            rmdir(c_rstrDirectory.c_str());
#endif
        }

        ms_strDirectoryDeque.clear();
    }

  protected:
    static std::deque<std::string> ms_strDirectoryDeque;
};

std::deque<std::string> CDirRemover::ms_strDirectoryDeque;

void RemoveAllDirectory(const char *c_szDirectoryName)
{
    {
        CDirRemover remover;
        remover.Create("*.*", c_szDirectoryName);
        CDirRemover::RemoveAllDirectory();
    }
#ifdef _WIN32
    RemoveDirectory(c_szDirectoryName);
#else
    rmdir(c_szDirectoryName);
#endif
}

void StringExceptCharacter(std::string *pstrString, const char *c_szCharacter)
{
    int const icurPos = 0;
    int iNextPos = 0;

    while ((iNextPos = pstrString->find_first_of(c_szCharacter, icurPos)) >= 0)
    {
        std::string const strFront = pstrString->substr(icurPos, iNextPos - icurPos);
        std::string const strBack = pstrString->substr(iNextPos + 1, pstrString->length() - iNextPos - 1);
        *pstrString = strFront + strBack;
    }
}

bool SplitLine(const char *c_szLine, const char *c_szDelimeter, std::vector<std::string> *pkVec_strToken)
{
    pkVec_strToken->reserve(10);
    pkVec_strToken->clear();

    std::string strLine = c_szLine;

    DWORD basePos = 0;

    do
    {
        int beginPos = strLine.find_first_not_of(c_szDelimeter, basePos);
        if (beginPos < 0)
            return false;

        int endPos = 0;

        if (strLine[beginPos] == '"')
        {
            ++beginPos;
            endPos = strLine.find_first_of('\"', beginPos);

            if (endPos < 0)
                return false;

            basePos = endPos + 1;
        }
        else
        {
            endPos = strLine.find_first_of(c_szDelimeter, beginPos);
            basePos = endPos;
        }

        pkVec_strToken->push_back(strLine.substr(beginPos, endPos - beginPos));
    } while (basePos < strLine.length());

    return true;
}

void GetExcutedFileName(std::string &r_str)
{
    char szPath[MAX_PATH + 1];

#ifdef _WIN32
    GetModuleFileName(NULL, szPath, MAX_PATH);
    szPath[MAX_PATH] = '\0';
    r_str = szPath;
#else
    ssize_t const len = readlink("/proc/self/exe", szPath, MAX_PATH);
    if (len > 0)
    {
        szPath[len] = '\0';
        r_str = szPath;
    }
    else
    {
        r_str = "";
    }
#endif
}

const char *_getf(const char *c_szFormat, ...)
{
    static char szBuf[256];

    va_list args;
    va_start(args, c_szFormat);
    _vsnprintf(szBuf, sizeof(szBuf), c_szFormat, args);
    va_end(args);

    return szBuf;
}

PCHAR *CommandLineToArgv(PCHAR CmdLine, int *_argc)
{
    PCHAR *argv = nullptr;
    PCHAR _argv = nullptr;
    ULONG len = 0;
    ULONG argc = 0;
    CHAR a = 0;
    ULONG i = 0;
    ULONG j = 0;

    BOOLEAN in_QM = 0;
    BOOLEAN in_TEXT = 0;
    BOOLEAN in_SPACE = 0;

    len = strlen(CmdLine);
    i = (((len + 2) / 2) * sizeof(PVOID)) + sizeof(PVOID);

    argv = static_cast<PCHAR *>(GlobalAlloc(GMEM_FIXED, i + ((len + 2) * sizeof(CHAR))));

    _argv = reinterpret_cast<PCHAR>((reinterpret_cast<PUCHAR>(argv)) + i);

    argc = 0;
    argv[argc] = _argv;
    in_QM = FALSE;
    in_TEXT = FALSE;
    in_SPACE = TRUE;
    i = 0;
    j = 0;

    while ((a = CmdLine[i]))
    {
        if (in_QM)
        {
            if (a == '\"')
            {
                in_QM = FALSE;
            }
            else
            {
                _argv[j] = a;
                j++;
            }
        }
        else
        {
            switch (a)
            {
            case '\"':
                in_QM = TRUE;
                in_TEXT = TRUE;
                if (in_SPACE)
                {
                    argv[argc] = _argv + j;
                    argc++;
                }
                in_SPACE = FALSE;
                break;
            case ' ':
            case '\t':
            case '\n':
            case '\r':
                if (in_TEXT)
                {
                    _argv[j] = '\0';
                    j++;
                }
                in_TEXT = FALSE;
                in_SPACE = TRUE;
                break;
            default:
                in_TEXT = TRUE;
                if (in_SPACE)
                {
                    argv[argc] = _argv + j;
                    argc++;
                }
                _argv[j] = a;
                j++;
                in_SPACE = FALSE;
                break;
            }
        }
        i++;
    }
    _argv[j] = '\0';
    argv[argc] = nullptr;

    (*_argc) = argc;
    return argv;
}
