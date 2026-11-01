#pragma once

#include "Resource.h"
#include "FileLoaderThread.h"
#include <Neoeter/Platform/Base/Singleton.h>

#include <set>
#include <map>
#include <string>

class CResourceManager : public CSingleton<CResourceManager>
{
  public:
    CResourceManager();
    ~CResourceManager() override;

    void LoadStaticCache(const char *c_szFileName);

    void DestroyDeletingList();
    void Destroy();

    void BeginThreadLoading();
    void EndThreadLoading();

    CResource *InsertResourcePointer(DWORD dwFileCRC, CResource *pResource);
    CResource *FindResourcePointer(DWORD dwFileCRC);
    CResource *GetResourcePointer(const char *c_szFileName);
    CResource *GetTypeResourcePointer(const char *c_szFileName, int iType = -1);

    // 추가
    bool isResourcePointerData(DWORD dwFileCRC);

    void RegisterResourceNewFunctionPointer(const char *c_szFileExt, CResource *(*pNewFunc)(const char *c_szFileName));
    void RegisterResourceNewFunctionByTypePointer(int iType, CResource *(*pNewFunc)(const char *c_szFileName));

    void DumpFileListToTextFile(const char *c_szFileName);
    bool IsFileExist(const char *c_szFileName);

    void Update();
    void ReserveDeletingResource(CResource *pResource);

    void ProcessBackgroundLoading();
    void PushBackgroundLoadingSet(std::set<std::string> &LoadingSet);

  protected:
    void __DestroyDeletingResourceMap();
    void __DestroyResourceMap();
    void __DestroyCacheMap();

    DWORD
    __GetFileCRC(const char *c_szFileName, const char **c_ppszLowerFileName = nullptr);

    using TResourcePointerMap = std::map<DWORD, CResource *>;
    using TResourceNewFunctionPointerMap = std::map<std::string, CResource *(*)(const char *)>;
    using TResourceNewFunctionByTypePointerMap = std::map<int, CResource *(*)(const char *)>;
    using TResourceDeletingMap = std::map<CResource *, DWORD>;
    using TResourceRequestMap = std::map<DWORD, std::string>;
    using TResourceRefDecreaseWaitingMap = std::map<long, CResource *>;

    TResourcePointerMap m_pCacheMap;
    TResourcePointerMap m_pResMap;
    TResourceNewFunctionPointerMap m_pResNewFuncMap;
    TResourceNewFunctionByTypePointerMap m_pResNewFuncByTypeMap;
    TResourceDeletingMap m_ResourceDeletingMap;
    TResourceRequestMap m_RequestMap; // 쓰레드로 로딩 요청한 리스트
    TResourceRequestMap m_WaitingMap;
    TResourceRefDecreaseWaitingMap m_pResRefDecreaseWaitingMap;

    static CFileLoaderThread ms_loadingThread;
};

extern int g_iLoadingDelayTime;
