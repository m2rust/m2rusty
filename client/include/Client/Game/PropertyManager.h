#pragma once

class CPropertyManager : public CSingleton<CPropertyManager>
{
  public:
    CPropertyManager();
    ~CPropertyManager() override;

    void Clear();

    bool LoadReservedCRC(const char *c_pszFileName);
    void ReserveCRC(DWORD dwCRC);
    DWORD GetUniqueCRC(const char *c_szSeed);

    void Initialize();
    bool Register(const char *c_pszFileName, CProperty **ppProperty = nullptr);

    bool Get(DWORD dwCRC, CProperty **ppProperty);
    bool Get(const char *c_pszFileName, CProperty **ppProperty);

  protected:
    using TPropertyCRCMap = std::map<DWORD, CProperty *>;
    using TCRCSet = std::set<DWORD>;

    TPropertyCRCMap m_PropertyByCRCMap;
    TCRCSet m_ReservedCRCSet;
};
