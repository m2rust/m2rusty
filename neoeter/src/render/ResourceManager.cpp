#include <Neoeter/Render/StdAfx.h>
#ifdef _WIN32
#include <io.h>
#endif
#include <Neoeter/Platform/Base/CRC32.h>
#include <Neoeter/Platform/Base/Timer.h>
#include <Neoeter/Platform/Base/Stl.h>
#include <Neoeter/IO/EterPackManager.h>

#include <Neoeter/Render/ResourceManager.h>
#include <Neoeter/Render/GrpImage.h>

#include <algorithm>

#include <print>

int g_iLoadingDelayTime = 20;

const long c_Deleting_Wait_Time = 30000;           // 삭제 대기 시간 (30초)
const long c_DeletingCountPerFrame = 30;           // 프레임당 체크 리소스 갯수
const long c_Reference_Decrease_Wait_Time = 30000; // 선로딩 리소스의 해제 대기 시간 (30초)

CFileLoaderThread CResourceManager::ms_loadingThread;

void CResourceManager::LoadStaticCache(const char *c_szFileName)
{
    CResource *pkRes = GetResourcePointer(c_szFileName);
    if (!pkRes)
    {
        Lognf(1, "CResourceManager::LoadStaticCache %s - FAILED", c_szFileName);
        return;
    }

    DWORD const dwCacheKey = GetCRC32(c_szFileName, strlen(c_szFileName));
    auto f = m_pCacheMap.find(dwCacheKey);
    if (m_pCacheMap.end() != f)
        return;

    pkRes->AddReference();
    m_pCacheMap.insert(TResourcePointerMap::value_type(dwCacheKey, pkRes));
}

void CResourceManager::ProcessBackgroundLoading()
{
    auto itor = m_RequestMap.begin();

    while (itor != m_RequestMap.end())
    {
        DWORD const dwFileCRC = itor->first;
        std::string const &stFileName = itor->second;

        if (isResourcePointerData(dwFileCRC) || (m_WaitingMap.contains(dwFileCRC)))
        {
            // printf("SKP %s\n", stFileName.c_str());
            itor = m_RequestMap.erase(itor);
            continue;
        }

        // printf("REQ %s\n", stFileName.c_str());
        ms_loadingThread.Request(stFileName);
        m_WaitingMap.insert(TResourceRequestMap::value_type(dwFileCRC, stFileName));
        itor = m_RequestMap.erase(itor);
        // break; // NOTE: 여기서 break 하면 천천히 로딩 된다.
    }

    DWORD const dwCurrentTime = ELTimer_GetMSec();

    CFileLoaderThread::TData pData;
    while (ms_loadingThread.Fetch(pData))
    {
        // printf("LOD %s\n", pData->stFileName.c_str());
        CResource *pResource = GetResourcePointer(pData.fileName.c_str());

        if (pResource)
        {
            if (pResource->IsEmpty())
            {
                pResource->OnLoad(pData.data->size(), pData.data->data());
                pResource->AddReferenceOnly();

                // 여기서 올라간 레퍼런스 카운트를 일정 시간이 지난 뒤에 풀어주기
                // 위하여
                m_pResRefDecreaseWaitingMap.insert(
                    TResourceRefDecreaseWaitingMap::value_type(dwCurrentTime, pResource));
            }
        }

        m_WaitingMap.erase(GetCRC32(pData.fileName.c_str(), pData.fileName.size()));
    }

    // DO : 일정 시간이 지나고 난뒤 미리 로딩해 두었던 리소스의 레퍼런스 카운트를
    // 감소 시킨다 - [levites]
    long const lCurrentTime = ELTimer_GetMSec();

    auto itorRef = m_pResRefDecreaseWaitingMap.begin();

    while (itorRef != m_pResRefDecreaseWaitingMap.end())
    {
        const long &rCreatingTime = itorRef->first;

        if (lCurrentTime - rCreatingTime > c_Reference_Decrease_Wait_Time)
        {
            CResource *pResource = itorRef->second;

            // Decrease Reference Count
            pResource->Release();
            itorRef = m_pResRefDecreaseWaitingMap.erase(itorRef);
            // Tracef("Decrease Pre Loading Resource\n", rCreatingTime);
        }
        else
        {
            ++itorRef;
        }
    }
}

void CResourceManager::PushBackgroundLoadingSet(std::set<std::string> &LoadingSet)
{
    auto itor = LoadingSet.begin();

    while (itor != LoadingSet.end())
    {
        DWORD const dwFileCRC = __GetFileCRC(itor->c_str());

        if (NULL != isResourcePointerData(dwFileCRC))
        {
            ++itor;
            continue;
        }

        m_RequestMap.insert(TResourceRequestMap::value_type(dwFileCRC, itor->c_str()));
        ++itor;
    }
}

void CResourceManager::__DestroyCacheMap()
{
    TResourcePointerMap::iterator i;
    for (i = m_pCacheMap.begin(); i != m_pCacheMap.end(); ++i)
    {
        CResource *pResource = i->second;
        pResource->Release();
    }

    m_pCacheMap.clear();
}

void CResourceManager::__DestroyDeletingResourceMap()
{
    Tracenf("CResourceManager::__DestroyDeletingResourceMap %d", m_ResourceDeletingMap.size());
    for (auto &i : m_ResourceDeletingMap)
        (i.first)->Clear();

    m_ResourceDeletingMap.clear();
}

void CResourceManager::__DestroyResourceMap()
{
    Tracenf("CResourceManager::__DestroyResourceMap %d", m_pResMap.size());

    TResourcePointerMap::iterator i;
    for (i = m_pResMap.begin(); i != m_pResMap.end(); ++i)
    {
        CResource *pResource = i->second;
        pResource->Clear();
    }

    stl_wipe_second(m_pResMap);
}

void CResourceManager::DestroyDeletingList()
{
    CResource::SetDeleteImmediately(true);

    __DestroyCacheMap();
    __DestroyDeletingResourceMap();
}

void CResourceManager::Destroy()
{
    assert(m_ResourceDeletingMap.empty() && "CResourceManager::Destroy - YOU MUST CALL DestroyDeletingList");
    __DestroyResourceMap();
}

void CResourceManager::RegisterResourceNewFunctionPointer(const char *c_szFileExt,
                                                          CResource *(*pNewFunc)(const char *c_szFileName))
{
    m_pResNewFuncMap[c_szFileExt] = pNewFunc;
}

void CResourceManager::RegisterResourceNewFunctionByTypePointer(int iType,
                                                                CResource *(*pNewFunc)(const char *c_szFileName))
{
    assert(iType >= 0);
    m_pResNewFuncByTypeMap[iType] = pNewFunc;
}

CResource *CResourceManager::InsertResourcePointer(DWORD dwFileCRC, CResource *pResource)
{
    auto itor = m_pResMap.find(dwFileCRC);

    if (m_pResMap.end() != itor)
    {
        TraceError("CResource::InsertResourcePointer: %s is already registered\n", pResource->GetFileName());
        assert(!"CResource::InsertResourcePointer: Resource already resistered");
        delete pResource;
        return itor->second;
    }

    m_pResMap.insert(TResourcePointerMap::value_type(dwFileCRC, pResource));
    return pResource;
}

int __ConvertPathName(const char *c_szPathName, char *pszRetPathName, int retLen)
{
    const char *pc = nullptr;
    int len = 0;

    for (pc = c_szPathName; *pc && len < retLen; ++pc, ++len)
    {
        if (*pc == '/')
        {
            *(pszRetPathName++) = '\\';
        }
        else
        {
            *(pszRetPathName++) = korean_tolower(*pc);
        }
    }

    *pszRetPathName = '\0';
    return len;
}

CResource *CResourceManager::GetTypeResourcePointer(const char *c_szFileName, int iType)
{
    if (!c_szFileName || !*c_szFileName)
    {
        assert(c_szFileName != nullptr && *c_szFileName != '\0');
        return nullptr;
    }

    const char *c_pszFile = nullptr;
    DWORD const dwFileCRC = __GetFileCRC(c_szFileName, &c_pszFile);
    CResource *pResource = FindResourcePointer(dwFileCRC);

    if (pResource) // 이미 리소스가 있으면 리턴 한다.
        return pResource;

    CResource *(*newFunc)(const char *) = nullptr;

    if (iType != -1)
    {
        auto f = m_pResNewFuncByTypeMap.find(iType);

        if (m_pResNewFuncByTypeMap.end() != f)
            newFunc = f->second;
    }
    else
    {
        const char *pcFileExt = strrchr(c_pszFile, '.');

        if (pcFileExt)
        {
            static char s_szFileExt[8 + 1];
            strncpy(s_szFileExt, pcFileExt + 1, 8);

            auto f = m_pResNewFuncMap.find(s_szFileExt);

            if (m_pResNewFuncMap.end() != f)
                newFunc = f->second;
        }
    }

    if (!newFunc)
    {
        TraceError("ResourceManager::GetResourcePointer: NOT SUPPORT FILE %s", c_pszFile);
        return nullptr;
    }

    pResource = InsertResourcePointer(dwFileCRC, newFunc(c_pszFile));
    return pResource;
}

CResource *CResourceManager::GetResourcePointer(const char *c_szFileName)
{
    if (!c_szFileName || !*c_szFileName)
    {
        TraceError("CResourceManager::GetResourcePointer: filename error!");
        return nullptr;
    }

    const char *c_pszFile = nullptr;
    DWORD const dwFileCRC = __GetFileCRC(c_szFileName, &c_pszFile);
    CResource *pResource = FindResourcePointer(dwFileCRC);

    if (pResource) // 이미 리소스가 있으면 리턴 한다.
        return pResource;

    const char *pcFileExt = strrchr(c_pszFile, '.');

#ifdef _DEBUG
    if (!IsFileExist(c_szFileName))
    {
        if (pcFileExt == NULL || (stricmp(pcFileExt, ".fnt") != 0))
        {
            TraceError("CResourceManager::GetResourcePointer: File not exist %s", c_szFileName);
        }
    }
#endif

    CResource *(*newFunc)(const char *) = nullptr;

    if (pcFileExt)
    {
        static char s_szFileExt[8 + 1];
        strncpy(s_szFileExt, pcFileExt + 1, 8);

        auto f = m_pResNewFuncMap.find(s_szFileExt);

        if (m_pResNewFuncMap.end() != f)
            newFunc = f->second;
    }

    if (!newFunc)
    {
        TraceError("ResourceManager::GetResourcePointer: NOT SUPPORT FILE %s", c_pszFile);
        return nullptr;
    }

    pResource = InsertResourcePointer(dwFileCRC, newFunc(c_pszFile));
    return pResource;
}

CResource *CResourceManager::FindResourcePointer(DWORD dwFileCRC)
{
    auto itor = m_pResMap.find(dwFileCRC);

    if (m_pResMap.end() == itor)
        return nullptr;

    return itor->second;
}

bool CResourceManager::isResourcePointerData(DWORD dwFileCRC)
{
    auto itor = m_pResMap.find(dwFileCRC);

    if (m_pResMap.end() == itor)
        return NULL;

    return (itor->second)->IsData();
}

DWORD CResourceManager::__GetFileCRC(const char *c_szFileName, const char **c_ppszLowerFileName)
{
    static char s_szFullPathFileName[MAX_PATH];
    const char *src = c_szFileName;
    char *dst = s_szFullPathFileName;
    int len = 0;

    while (src[len])
    {
        if (src[len] == '/')
        {
            dst[len] = '\\';
        }
        else
        {
            dst[len] = korean_tolower(src[len]);
        }

        ++len;
    }

    dst[len] = '\0';

    if (c_ppszLowerFileName)
        *c_ppszLowerFileName = &s_szFullPathFileName[0];

    return (GetCRC32(s_szFullPathFileName, len));
}

using TDumpData = struct SDumpData
{
    const char *filename;
    float KB;
    DWORD cost;
};

bool DumpKBCompare(const TDumpData &lhs, const TDumpData &rhs)
{
    return lhs.KB > rhs.KB;
}

bool DumpCostCompare(const TDumpData &lhs, const TDumpData &rhs)
{
    return lhs.cost > rhs.cost;
}

struct FDumpPrint
{
    FILE *m_fp;
    static float m_totalKB;

    void operator()(TDumpData &data) const
    {
        m_totalKB += data.KB;
        std::println(m_fp, "{:6.1f} {}", data.KB, data.filename);
    }
};

float FDumpPrint::m_totalKB;

struct FDumpCostPrint
{
    FILE *m_fp;

    void operator()(TDumpData &data) const
    {
        std::println(m_fp, "{:<4} {}", data.cost, data.filename);
    }
};

void CResourceManager::DumpFileListToTextFile(const char *c_szFileName)
{
    std::vector<TDumpData> dumpVector;

    for (auto &i : m_pResMap)
    {
        CResource *pResource = i.second;
        TDumpData data;

        if (pResource->IsEmpty())
            continue;

        data.filename = pResource->GetFileName();

        int filesize = 0;

        const char *ext = strrchr(data.filename, '.');

        if (pResource->IsType(CGraphicImage::Type()) && strnicmp(ext, ".sub", 4) != 0)
        {
            filesize = ((CGraphicImage *)pResource)->GetWidth() * ((CGraphicImage *)pResource)->GetHeight() * 4;
        }
        else
        {
            FILE *fp2 = fopen(data.filename, "rb");

            if (fp2)
            {
                fseek(fp2, 0L, SEEK_END);
                filesize = ftell(fp2);
                fclose(fp2);
            }
            else
            {
                filesize = 0;
            }
        }

        data.KB = static_cast<float>(filesize) / static_cast<float>(1024);
        data.cost = pResource->GetLoadCostMilliSecond();

        dumpVector.push_back(data);
    }

    FILE *fp = fopen(c_szFileName, "w");

    if (fp)
    {
        std::ranges::sort(dumpVector, DumpKBCompare);

        FDumpPrint DumpPrint{};
        DumpPrint.m_fp = fp;
        FDumpPrint::m_totalKB = 0;

        std::ranges::for_each(dumpVector, DumpPrint);
        std::print(fp, "total: {:.2f}mb", FDumpPrint::m_totalKB / 1024.0f);

        FDumpCostPrint DumpCostPrint{};
        DumpCostPrint.m_fp = fp;

        std::ranges::sort(dumpVector, DumpCostCompare);
        std::ranges::for_each(dumpVector, DumpCostPrint);
        std::print(fp, "total: {:.2f}mb", FDumpPrint::m_totalKB / 1024.0f);

        fclose(fp);
    }
}

bool CResourceManager::IsFileExist(const char *c_szFileName)
{
    return CEterPackManager::Instance().isExist(c_szFileName);
}

void CResourceManager::Update()
{
    DWORD const CurrentTime = ELTimer_GetMSec();
    CResource *pResource = nullptr;
    int Count = 0;

    auto itor = m_ResourceDeletingMap.begin();

    while (itor != m_ResourceDeletingMap.end())
    {
        pResource = itor->first;

        if (CurrentTime >= itor->second)
        {
            if (pResource->canDestroy())
            {
                // Tracef("Resource Clear %s\n", pResource->GetFileName());
                pResource->Clear();
            }

            itor = m_ResourceDeletingMap.erase(itor);

            if (++Count >= c_DeletingCountPerFrame)
                break;
        }
        else
        {
            ++itor;
        }
    }

    ProcessBackgroundLoading();
}

void CResourceManager::ReserveDeletingResource(CResource *pResource)
{
    DWORD const dwCurrentTime = ELTimer_GetMSec();
    m_ResourceDeletingMap.insert(TResourceDeletingMap::value_type(pResource, dwCurrentTime + c_Deleting_Wait_Time));
}

CResourceManager::CResourceManager()
{
    // ms_loadingThread.Create(0);
}

CResourceManager::~CResourceManager()
{
    Destroy();
    // ms_loadingThread.Shutdown();
}
