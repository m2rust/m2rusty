#pragma once

#include <Neoeter/Effect/StdAfx.h>
#include <Neoeter/Render/Resource.h>
#include <Neoeter/Render/ResourceManager.h>
#include <vector>

enum EResourceTypes
{
    RES_TYPE_UNKNOWN,
};

class CPythonResource : public CSingleton<CPythonResource>
{
  public:
    using TDestroyHandler = void (*)();

    CPythonResource();
    ~CPythonResource() override;

    void Destroy();

    void DumpFileList(const char *c_szFileName);

    static void RegisterDestroyHandler(TDestroyHandler pfnHandler);

  protected:
    CResourceManager m_resManager;
    static std::vector<TDestroyHandler> ms_destroyHandlers;
};
