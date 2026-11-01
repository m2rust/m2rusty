#pragma once

#include <Neoeter/Platform/Base/FileDir.h>

class CPropertyManager;

class CPropertyLoader : public CDir
{
  public:
    CPropertyLoader();
    ~CPropertyLoader() override;

    void SetPropertyManager(CPropertyManager *pPropertyManager);
    DWORD RegisterFile(const char *c_szPathName, const char *c_szFileName);

    bool OnFolder(const char *c_szFilter, const char *c_szPathName, const char *c_szFileName) override;
    bool OnFile(const char *c_szPathName, const char *c_szFileName) override;

  protected:
    CPropertyManager *m_pPropertyManager;
};
