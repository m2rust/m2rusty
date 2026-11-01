#include <Client/Game/StdAfx.h>
#include <Neoeter/IO/EterPackManager.h>

#include <Client/Game/PropertyManager.h>
#include <Client/Game/Property.h>

CPropertyManager::CPropertyManager() = default;

CPropertyManager::~CPropertyManager()
{
    Clear();
}

void CPropertyManager::Initialize()
{
    auto fileMap = CEterPackManager::Instance().GetFileMap();

    for (auto &itor : fileMap)
    {
        if (itor.first.starts_with("property/reserve"))
        {
            LoadReservedCRC(itor.first.c_str());
            continue;
        }

        if (itor.first.starts_with("property/"))
        {
            if (!Register(itor.first.c_str()))
                continue;
        }
    }
}

bool CPropertyManager::LoadReservedCRC(const char *c_pszFileName)
{
    CEterPackManager::TPackDataPtr data;
    CMemoryTextFileLoader textFileLoader;

    if (!CEterPackManager::Instance().Get(c_pszFileName, data))
        return false;

    textFileLoader.Bind(data->size(), data->data());

    for (DWORD i = 0; i < textFileLoader.GetLineCount(); ++i)
    {
        const char *pszLine = textFileLoader.GetLineString(i).c_str();

        if (!pszLine || !*pszLine)
            continue;

        ReserveCRC(atoi(pszLine));
    }

    return true;
}

void CPropertyManager::ReserveCRC(DWORD dwCRC)
{
    m_ReservedCRCSet.insert(dwCRC);
}

DWORD CPropertyManager::GetUniqueCRC(const char *c_szSeed)
{
    std::string stTmp = c_szSeed;

    while (true)
    {
        DWORD const dwCRC = GetCRC32(stTmp.c_str(), stTmp.length());

        if (!m_ReservedCRCSet.contains(dwCRC) && !m_PropertyByCRCMap.contains(dwCRC))
            return dwCRC;

        char szAdd[2];
        _snprintf(szAdd, sizeof(szAdd), "%d", random() % 10);
        stTmp += szAdd;
    }
}

bool CPropertyManager::Register(const char *c_pszFileName, CProperty **ppProperty)
{
    CEterPackManager::TPackDataPtr data;
    if (!CEterPackManager::Instance().Get(c_pszFileName, data))
        return false;

    auto *pProperty = new CProperty(c_pszFileName);

    if (!pProperty->ReadFromMemory(data->data(), data->size(), c_pszFileName))
    {
        delete pProperty;
        return false;
    }

    DWORD const dwCRC = pProperty->GetCRC();

    auto itor = m_PropertyByCRCMap.find(dwCRC);

    if (m_PropertyByCRCMap.end() != itor)
    {
        Tracef("Property already registered, replace %s to %s\n", itor->second->GetFileName(), c_pszFileName);

        delete itor->second;
        itor->second = pProperty;
    }
    else
    {
        m_PropertyByCRCMap.insert(TPropertyCRCMap::value_type(dwCRC, pProperty));
    }

    if (ppProperty)
        *ppProperty = pProperty;

    return true;
}

bool CPropertyManager::Get(const char *c_pszFileName, CProperty **ppProperty)
{
    return Register(c_pszFileName, ppProperty);
}

bool CPropertyManager::Get(DWORD dwCRC, CProperty **ppProperty)
{
    auto itor = m_PropertyByCRCMap.find(dwCRC);

    if (m_PropertyByCRCMap.end() == itor)
        return false;

    *ppProperty = itor->second;
    return true;
}

void CPropertyManager::Clear()
{
    stl_wipe_second(m_PropertyByCRCMap);
}
