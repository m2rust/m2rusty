#include <Client/UI/StdAfx.h>
#include <Client/UI/PythonGuild.h>
#include <Client/UI/AbstractPlayer.h>
#include <Client/UI/MarkManager.h>

#include <algorithm>

#include <utility>

std::map<DWORD, DWORD> g_GuildSkillSlotToIndexMap;

void CPythonGuild::EnableGuild()
{
    m_bGuildEnable = TRUE;
}

void CPythonGuild::SetGuildMoney(DWORD dwMoney)
{
    m_GuildInfo.dwGuildMoney = dwMoney;
}

void CPythonGuild::SetGuildEXP(BYTE byLevel, DWORD dwEXP)
{
    m_GuildInfo.dwGuildLevel = byLevel;
    m_GuildInfo.dwCurrentExperience = dwEXP;
}

void CPythonGuild::SetGradeData(BYTE byGradeNumber, const TGuildGradeData &rGuildGradeData)
{
    m_GradeDataMap[byGradeNumber] = rGuildGradeData;
}

void CPythonGuild::SetGradeName(BYTE byGradeNumber, const char *c_szName)
{
    if (!__IsGradeData(byGradeNumber))
        return;

    TGuildGradeData &rGradeData = m_GradeDataMap.find(byGradeNumber)->second;
    rGradeData.strName = c_szName;
}

void CPythonGuild::SetGradeAuthority(BYTE byGradeNumber, BYTE byAuthority)
{
    if (!__IsGradeData(byGradeNumber))
        return;

    TGuildGradeData &rGradeData = m_GradeDataMap.find(byGradeNumber)->second;
    rGradeData.byAuthorityFlag = byAuthority;
}

void CPythonGuild::ClearComment()
{
    m_GuildBoardCommentVector.clear();
}

void CPythonGuild::RegisterComment(DWORD dwCommentID, const char *c_szName, const char *c_szComment)
{
    if (0 == strlen(c_szComment))
        return;

    TGuildBoardCommentData CommentData;
    CommentData.dwCommentID = dwCommentID;
    CommentData.strName = c_szName;
    CommentData.strComment = c_szComment;

    m_GuildBoardCommentVector.push_back(CommentData);
}

void CPythonGuild::RegisterMember(TGuildMemberData &rGuildMemberData)
{
    TGuildMemberData *pGuildMemberData = nullptr;
    if (GetMemberDataPtrByPID(rGuildMemberData.dwPID, &pGuildMemberData))
    {
        pGuildMemberData->byGeneralFlag = rGuildMemberData.byGeneralFlag;
        pGuildMemberData->byGrade = rGuildMemberData.byGrade;
        pGuildMemberData->byLevel = rGuildMemberData.byLevel;
        pGuildMemberData->dwOffer = rGuildMemberData.dwOffer;
    }
    else
    {
        m_GuildMemberDataVector.push_back(rGuildMemberData);
    }

    __CalculateLevelAverage();
    __SortMember();
}

struct CPythonGuild_FFindGuildMemberByPID
{
    CPythonGuild_FFindGuildMemberByPID(DWORD dwSearchingPID_) : dwSearchingPID(dwSearchingPID_)
    {
    }
    int operator()(CPythonGuild::TGuildMemberData &rGuildMemberData) const
    {
        return rGuildMemberData.dwPID == dwSearchingPID;
    }

    DWORD dwSearchingPID;
};

struct CPythonGuild_FFindGuildMemberByName
{
    CPythonGuild_FFindGuildMemberByName(const char *c_szSearchingName) : strSearchingName(c_szSearchingName)
    {
    }
    int operator()(CPythonGuild::TGuildMemberData &rGuildMemberData) const
    {
        return rGuildMemberData.strName.c_str() == strSearchingName;
    }

    std::string strSearchingName;
};

void CPythonGuild::ChangeGuildMemberGrade(DWORD dwPID, BYTE byGrade)
{
    TGuildMemberData *pGuildMemberData = nullptr;
    if (!GetMemberDataPtrByPID(dwPID, &pGuildMemberData))
        return;

    pGuildMemberData->byGrade = byGrade;
}

void CPythonGuild::ChangeGuildMemberGeneralFlag(DWORD dwPID, BYTE byFlag)
{
    TGuildMemberData *pGuildMemberData = nullptr;
    if (!GetMemberDataPtrByPID(dwPID, &pGuildMemberData))
        return;

    pGuildMemberData->byGeneralFlag = byFlag;
}

void CPythonGuild::RemoveMember(DWORD dwPID)
{
    TGuildMemberDataVector::iterator itor;
    itor = std::ranges::find_if(m_GuildMemberDataVector, CPythonGuild_FFindGuildMemberByPID(dwPID));

    if (m_GuildMemberDataVector.end() == itor)
        return;

    m_GuildMemberDataVector.erase(itor);
}

void CPythonGuild::RegisterGuildName(DWORD dwID, const char *c_szName)
{
    m_GuildNameMap.insert(make_pair(dwID, std::string(c_szName)));
}

BOOL CPythonGuild::IsMainPlayer(DWORD dwPID)
{
    TGuildMemberData *pGuildMemberData = nullptr;
    if (!GetMemberDataPtrByPID(dwPID, &pGuildMemberData))
        return FALSE;

    IAbstractPlayer &rPlayer = IAbstractPlayer::GetSingleton();
    return rPlayer.GetName() == pGuildMemberData->strName;
}

BOOL CPythonGuild::IsGuildEnable() const
{
    return m_bGuildEnable;
}

CPythonGuild::TGuildInfo &CPythonGuild::GetGuildInfoRef()
{
    return m_GuildInfo;
}

BOOL CPythonGuild::GetGradeDataPtr(DWORD dwGradeNumber, TGuildGradeData **ppData)
{
    auto itor = m_GradeDataMap.find(dwGradeNumber);
    if (m_GradeDataMap.end() == itor)
        return FALSE;

    *ppData = &(itor->second);

    return TRUE;
}

const CPythonGuild::TGuildBoardCommentDataVector &CPythonGuild::GetGuildBoardCommentVector()
{
    return m_GuildBoardCommentVector;
}

DWORD CPythonGuild::GetMemberCount()
{
    return m_GuildMemberDataVector.size();
}

BOOL CPythonGuild::GetMemberDataPtr(DWORD dwIndex, TGuildMemberData **ppData)
{
    if (dwIndex >= m_GuildMemberDataVector.size())
        return FALSE;

    *ppData = &m_GuildMemberDataVector[dwIndex];

    return TRUE;
}

BOOL CPythonGuild::GetMemberDataPtrByPID(DWORD dwPID, TGuildMemberData **ppData)
{
    TGuildMemberDataVector::iterator itor;
    itor = std::ranges::find_if(m_GuildMemberDataVector, CPythonGuild_FFindGuildMemberByPID(dwPID));

    if (m_GuildMemberDataVector.end() == itor)
        return FALSE;

    *ppData = &(*itor);
    return TRUE;
}

BOOL CPythonGuild::GetMemberDataPtrByName(const char *c_szName, TGuildMemberData **ppData)
{
    TGuildMemberDataVector::iterator itor;
    itor = std::ranges::find_if(m_GuildMemberDataVector, CPythonGuild_FFindGuildMemberByName(c_szName));

    if (m_GuildMemberDataVector.end() == itor)
        return FALSE;

    *ppData = &(*itor);
    return TRUE;
}

DWORD CPythonGuild::GetGuildMemberLevelSummary() const
{
    return m_dwMemberLevelSummary;
}

DWORD CPythonGuild::GetGuildMemberLevelAverage() const
{
    return m_dwMemberLevelAverage;
}

DWORD CPythonGuild::GetGuildExperienceSummary() const
{
    return m_dwMemberExperienceSummary;
}

CPythonGuild::TGuildSkillData &CPythonGuild::GetGuildSkillDataRef()
{
    return m_GuildSkillData;
}

bool CPythonGuild::GetGuildName(DWORD dwID, std::string *pstrGuildName)
{
    if (!m_GuildNameMap.contains(dwID))
        return false;

    *pstrGuildName = m_GuildNameMap[dwID];

    return true;
}

DWORD CPythonGuild::GetGuildID() const
{
    return m_GuildInfo.dwGuildID;
}

BOOL CPythonGuild::HasGuildLand() const
{
    return m_GuildInfo.bHasLand;
}

void CPythonGuild::StartGuildWar(DWORD dwEnemyGuildID)
{
    int i = 0;

    for (i = 0; i < ENEMY_GUILD_SLOT_MAX_COUNT; ++i)
    {
        if (dwEnemyGuildID == m_adwEnemyGuildID[i])
            return;
    }

    for (i = 0; i < ENEMY_GUILD_SLOT_MAX_COUNT; ++i)
    {
        if (0 == m_adwEnemyGuildID[i])
        {
            m_adwEnemyGuildID[i] = dwEnemyGuildID;
            break;
        }
    }
}

void CPythonGuild::EndGuildWar(DWORD dwEnemyGuildID)
{
    for (unsigned int &i : m_adwEnemyGuildID)
    {
        if (dwEnemyGuildID == i)
            i = 0;
    }
}

DWORD CPythonGuild::GetEnemyGuildID(DWORD dwIndex)
{
    if (dwIndex >= ENEMY_GUILD_SLOT_MAX_COUNT)
        return 0;

    return m_adwEnemyGuildID[dwIndex];
}

BOOL CPythonGuild::IsDoingGuildWar()
{
    for (unsigned int const i : m_adwEnemyGuildID)
    {
        if (0 != i)
        {
            return TRUE;
        }
    }

    return FALSE;
}

void CPythonGuild::__CalculateLevelAverage()
{
    m_dwMemberLevelSummary = 0;
    m_dwMemberLevelAverage = 0;
    m_dwMemberExperienceSummary = 0;

    if (m_GuildMemberDataVector.empty())
        return;

    TGuildMemberDataVector::iterator itor;

    // Sum Level & Experience
    itor = m_GuildMemberDataVector.begin();
    for (; itor != m_GuildMemberDataVector.end(); ++itor)
    {
        TGuildMemberData const &rGuildMemberData = *itor;
        m_dwMemberLevelSummary += rGuildMemberData.byLevel;
        m_dwMemberExperienceSummary += rGuildMemberData.dwOffer;
    }

    assert(!m_GuildMemberDataVector.empty());
    m_dwMemberLevelAverage = m_dwMemberLevelSummary / m_GuildMemberDataVector.size();
}

struct CPythonGuild_SLessMemberGrade
{
    bool operator()(CPythonGuild::TGuildMemberData &rleft, CPythonGuild::TGuildMemberData &rright)
    {
        return rleft.byGrade < rright.byGrade;
    }
};

void CPythonGuild::__SortMember()
{
    std::ranges::sort(m_GuildMemberDataVector, CPythonGuild_SLessMemberGrade());
}

BOOL CPythonGuild::__IsGradeData(BYTE byGradeNumber)
{
    return m_GradeDataMap.contains(byGradeNumber);
}

void CPythonGuild::__Initialize()
{
    ZeroMemory(&m_GuildInfo, sizeof(m_GuildInfo));
    ZeroMemory(&m_GuildSkillData, sizeof(m_GuildSkillData));
    ZeroMemory(&m_adwEnemyGuildID, ENEMY_GUILD_SLOT_MAX_COUNT * sizeof(DWORD));
    m_GradeDataMap.clear();
    m_GuildMemberDataVector.clear();
    m_dwMemberLevelSummary = 0;
    m_dwMemberLevelAverage = 0;
    m_bGuildEnable = FALSE;
    m_GuildNameMap.clear();
}

void CPythonGuild::Destroy()
{
    __Initialize();
}

CPythonGuild::CPythonGuild()
{
    __Initialize();
}
CPythonGuild::~CPythonGuild() = default;

///////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////

PyObject *guildIsGuildEnable(PyObject *poSelf, PyObject *poArgs)
{
    return Py_BuildValue("i", CPythonGuild::Instance().IsGuildEnable());
}

PyObject *guildGetGuildID(PyObject *poSelf, PyObject *poArgs)
{
    return Py_BuildValue("i", CPythonGuild::Instance().GetGuildID());
}

PyObject *guildHasGuildLand(PyObject *poSelf, PyObject *poArgs)
{
    return Py_BuildValue("i", CPythonGuild::Instance().HasGuildLand());
}

PyObject *guildGetGuildName(PyObject *poSelf, PyObject *poArgs)
{
    int iGuildID = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iGuildID))
    {
        CPythonGuild::TGuildInfo &rGuildInfo = CPythonGuild::Instance().GetGuildInfoRef();
        return Py_BuildValue("s", rGuildInfo.szGuildName);
    }

    std::string strGuildName;
    if (!CPythonGuild::Instance().GetGuildName(iGuildID, &strGuildName))
        return Py_BuildValue("s", "Noname");

    return Py_BuildValue("s", strGuildName.c_str());
}

PyObject *guildGetGuildMasterName(PyObject *poSelf, PyObject *poArgs)
{
    CPythonGuild::TGuildInfo const &rGuildInfo = CPythonGuild::Instance().GetGuildInfoRef();

    CPythonGuild::TGuildMemberData *pData = nullptr;
    if (!CPythonGuild::Instance().GetMemberDataPtrByPID(rGuildInfo.dwMasterPID, &pData))
        return Py_BuildValue("s", "Noname");

    return Py_BuildValue("s", pData->strName.c_str());
}

PyObject *guildGetEnemyGuildName(PyObject *poSelf, PyObject *poArgs)
{
    int iIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iIndex))
        return Py_BuildValue("s", "");

    DWORD const dwEnemyGuildID = CPythonGuild::Instance().GetEnemyGuildID(iIndex);

    std::string strEnemyGuildName;
    if (!CPythonGuild::Instance().GetGuildName(dwEnemyGuildID, &strEnemyGuildName))
        return Py_BuildValue("s", "");

    return Py_BuildValue("s", strEnemyGuildName.c_str());
}

PyObject *guildGetGuildMoney(PyObject *poSelf, PyObject *poArgs)
{
    CPythonGuild::TGuildInfo const &rGuildInfo = CPythonGuild::Instance().GetGuildInfoRef();
    return Py_BuildValue("i", rGuildInfo.dwGuildMoney);
}

PyObject *guildGetGuildBoardCommentCount(PyObject *poSelf, PyObject *poArgs)
{
    const CPythonGuild::TGuildBoardCommentDataVector &rCommentVector =
        CPythonGuild::Instance().GetGuildBoardCommentVector();
    return Py_BuildValue("i", rCommentVector.size());
}

PyObject *guildGetGuildBoardCommentData(PyObject *poSelf, PyObject *poArgs)
{
    int iIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iIndex))
        return Py_BuildException();

    const CPythonGuild::TGuildBoardCommentDataVector &c_rCommentVector =
        CPythonGuild::Instance().GetGuildBoardCommentVector();
    if (static_cast<DWORD>(iIndex) >= c_rCommentVector.size())
        return Py_BuildValue("iss", 0, "Noname", "Noname");

    const CPythonGuild::TGuildBoardCommentData &c_rData = c_rCommentVector[iIndex];

    return Py_BuildValue("iss", c_rData.dwCommentID, c_rData.strName.c_str(), c_rData.strComment.c_str());
}

PyObject *guildGetGuildLevel(PyObject *poSelf, PyObject *poArgs)
{
    CPythonGuild::TGuildInfo const &rGuildInfo = CPythonGuild::Instance().GetGuildInfoRef();
    return Py_BuildValue("i", rGuildInfo.dwGuildLevel);
}

PyObject *guildGetGuildExperience(PyObject *poSelf, PyObject *poArgs)
{
    CPythonGuild::TGuildInfo const &rGuildInfo = CPythonGuild::Instance().GetGuildInfoRef();

    int const GULID_MAX_LEVEL = 20;
    if (rGuildInfo.dwGuildLevel >= GULID_MAX_LEVEL)
        return Py_BuildValue("ii", 0, 0);

    unsigned const lastExp = LocaleService_GetLastExp(rGuildInfo.dwGuildLevel);

    return Py_BuildValue("ii", rGuildInfo.dwCurrentExperience, lastExp - rGuildInfo.dwCurrentExperience);
}

PyObject *guildGetGuildMemberCount(PyObject *poSelf, PyObject *poArgs)
{
    CPythonGuild::TGuildInfo const &rGuildInfo = CPythonGuild::Instance().GetGuildInfoRef();
    return Py_BuildValue("ii", rGuildInfo.dwCurrentMemberCount, rGuildInfo.dwMaxMemberCount);
}

PyObject *guildGetGuildMemberLevelSummary(PyObject *poSelf, PyObject *poArgs)
{
    return Py_BuildValue("i", CPythonGuild::Instance().GetGuildMemberLevelSummary());
}

PyObject *guildGetGuildMemberLevelAverage(PyObject *poSelf, PyObject *poArgs)
{
    return Py_BuildValue("i", CPythonGuild::Instance().GetGuildMemberLevelAverage());
}

PyObject *guildGetGuildExperienceSummary(PyObject *poSelf, PyObject *poArgs)
{
    return Py_BuildValue("i", CPythonGuild::Instance().GetGuildExperienceSummary());
}

PyObject *guildGetGuildSkillPoint(PyObject *poSelf, PyObject *poArgs)
{
    const CPythonGuild::TGuildSkillData &c_rSkillData = CPythonGuild::Instance().GetGuildSkillDataRef();
    return Py_BuildValue("i", c_rSkillData.bySkillPoint);
}

PyObject *guildGetDragonPowerPoint(PyObject *poSelf, PyObject *poArgs)
{
    const CPythonGuild::TGuildSkillData &c_rSkillData = CPythonGuild::Instance().GetGuildSkillDataRef();
    return Py_BuildValue("ii", c_rSkillData.wGuildPoint, c_rSkillData.wMaxGuildPoint);
}

PyObject *guildGetGuildSkillLevel(PyObject *poSelf, PyObject *poArgs)
{
    assert(FALSE && !"guildGetGuildSkillLevel - \xBB\xE7\xBF\xEB\xC7\xCF\xC1\xF6 "
                     "\xBE\xCA\xB4\xC2 "
                     "\xC7\xD4\xBC\xF6\xC0\xD4\xB4\xCF\xB4\xD9.");

    int iSkillIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iSkillIndex))
        return Py_BuildException();

    const CPythonGuild::TGuildSkillData &c_rSkillData = CPythonGuild::Instance().GetGuildSkillDataRef();
    return Py_BuildValue("i", c_rSkillData.bySkillLevel[iSkillIndex]);
}

PyObject *guildGetSkillLevel(PyObject *poSelf, PyObject *poArgs)
{
    int iSlotIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iSlotIndex))
        return Py_BuildException();

    auto itor = g_GuildSkillSlotToIndexMap.find(iSlotIndex);

    if (g_GuildSkillSlotToIndexMap.end() == itor)
        return Py_BuildValue("i", 0);

    DWORD const dwSkillIndex = itor->second;
    assert(dwSkillIndex < CPythonGuild::GUILD_SKILL_MAX_NUM);

    const CPythonGuild::TGuildSkillData &c_rSkillData = CPythonGuild::Instance().GetGuildSkillDataRef();
    return Py_BuildValue("i", c_rSkillData.bySkillLevel[dwSkillIndex]);
}

PyObject *guildGetSkillMaxLevelNew(PyObject *poSelf, PyObject *poArgs)
{
    int iSlotIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iSlotIndex))
        return Py_BuildException();

    auto itor = g_GuildSkillSlotToIndexMap.find(iSlotIndex);

    if (g_GuildSkillSlotToIndexMap.end() == itor)
        return Py_BuildValue("i", 0);

    DWORD const dwSkillIndex = itor->second;
    assert(dwSkillIndex < CPythonGuild::GUILD_SKILL_MAX_NUM);

    const CPythonGuild::TGuildSkillData &c_rSkillData = CPythonGuild::Instance().GetGuildSkillDataRef();
    return Py_BuildValue("i", c_rSkillData.bySkillLevel[dwSkillIndex]);
}

PyObject *guildSetSkillIndex(PyObject *poSelf, PyObject *poArgs)
{
    int iSlotIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iSlotIndex))
        return Py_BuildException();
    int iSkillIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 1, &iSkillIndex))
        return Py_BuildException();

    g_GuildSkillSlotToIndexMap.insert(std::make_pair(iSlotIndex, iSkillIndex));

    return Py_BuildNone();
}

PyObject *guildGetSkillIndex(PyObject *poSelf, PyObject *poArgs)
{
    int iSlotIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iSlotIndex))
        return Py_BuildException();

    auto itor = g_GuildSkillSlotToIndexMap.find(iSlotIndex);

    if (g_GuildSkillSlotToIndexMap.end() == itor)
        return Py_BuildValue("i", 0);

    DWORD const dwSkillIndex = itor->second;
    return Py_BuildValue("i", dwSkillIndex);
}

PyObject *guildGetGradeData(PyObject *poSelf, PyObject *poArgs)
{
    int iGradeNumber = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iGradeNumber))
        return Py_BuildException();

    CPythonGuild::TGuildGradeData *pData = nullptr;
    if (!CPythonGuild::Instance().GetGradeDataPtr(iGradeNumber, &pData))
        return Py_BuildValue("si", "?", 0);

    return Py_BuildValue("si", pData->strName.c_str(), pData->byAuthorityFlag);
}

PyObject *guildGetGradeName(PyObject *poSelf, PyObject *poArgs)
{
    int iGradeNumber = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iGradeNumber))
        return Py_BuildException();

    CPythonGuild::TGuildGradeData *pData = nullptr;
    if (!CPythonGuild::Instance().GetGradeDataPtr(iGradeNumber, &pData))
        return Py_BuildValue("s", "?");

    return Py_BuildValue("s", pData->strName.c_str());
}

PyObject *guildGetMemberCount(PyObject *poSelf, PyObject *poArgs)
{
    return Py_BuildValue("i", CPythonGuild::Instance().GetMemberCount());
}

PyObject *guildGetMemberData(PyObject *poSelf, PyObject *poArgs)
{
    int iIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iIndex))
        return Py_BuildException();

    CPythonGuild::TGuildMemberData *pData = nullptr;
    if (!CPythonGuild::Instance().GetMemberDataPtr(iIndex, &pData))
        return Py_BuildValue("isiiiii", -1, "", 0, 0, 0, 0, 0);

    return Py_BuildValue("isiiiii", pData->dwPID, pData->strName.c_str(), pData->byGrade, pData->byJob, pData->byLevel,
                         pData->dwOffer, pData->byGeneralFlag);
}

PyObject *guildMemberIndexToPID(PyObject *poSelf, PyObject *poArgs)
{
    int iIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iIndex))
        return Py_BuildException();

    CPythonGuild::TGuildMemberData *pData = nullptr;
    if (!CPythonGuild::Instance().GetMemberDataPtr(iIndex, &pData))
        return Py_BuildValue("i", -1);

    return Py_BuildValue("i", pData->dwPID);
}

PyObject *guildIsMember(PyObject *poSelf, PyObject *poArgs)
{
    int iIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iIndex))
        return Py_BuildException();

    CPythonGuild::TGuildMemberData *pData = nullptr;
    if (CPythonGuild::Instance().GetMemberDataPtr(iIndex, &pData))
        return Py_BuildValue("i", TRUE);

    return Py_BuildValue("i", FALSE);
}

PyObject *guildIsMemberByName(PyObject *poSelf, PyObject *poArgs)
{
    char *szName = nullptr;
    if (!PyTuple_GetString(poArgs, 0, &szName))
        return Py_BuildException();

    CPythonGuild::TGuildMemberData *pData = nullptr;
    if (CPythonGuild::Instance().GetMemberDataPtrByName(szName, &pData))
        return Py_BuildValue("i", TRUE);

    return Py_BuildValue("i", FALSE);
}

PyObject *guildMainPlayerHasAuthority(PyObject *poSelf, PyObject *poArgs)
{
    int iAuthority = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iAuthority))
        return Py_BuildException();

    IAbstractPlayer &rPlayer = IAbstractPlayer::GetSingleton();
    const char *c_szMainPlayerName = rPlayer.GetName();

    CPythonGuild::TGuildMemberData *pMemberData = nullptr;
    if (!CPythonGuild::Instance().GetMemberDataPtrByName(c_szMainPlayerName, &pMemberData))
        return Py_BuildValue("i", FALSE);

    CPythonGuild::TGuildGradeData *pGradeData = nullptr;
    if (!CPythonGuild::Instance().GetGradeDataPtr(pMemberData->byGrade, &pGradeData))
        return Py_BuildValue("i", FALSE);

    return Py_BuildValue("i", iAuthority == (pGradeData->byAuthorityFlag & iAuthority));
}

PyObject *guildDestroy(PyObject *poSelf, PyObject *poArgs)
{
    CPythonGuild::Instance().Destroy();
    g_GuildSkillSlotToIndexMap.clear();
    return Py_BuildNone();
}

PyObject *guildGuildIDToMarkID(PyObject *poSelf, PyObject *poArgs)
{
    int guild_id = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &guild_id))
        return Py_BuildException();

    return Py_BuildValue("i", CGuildMarkManager::Instance().GetMarkID(guild_id));
}

PyObject *guildGetMarkImageFilenameByMarkID(PyObject *poSelf, PyObject *poArgs)
{
    int markID = 0;

    if (!PyTuple_GetInteger(poArgs, 0, &markID))
        return Py_BuildException();

    std::string imagePath;
    CGuildMarkManager::Instance().GetMarkImageFilename(markID / CGuildMarkImage::MARK_TOTAL_COUNT, imagePath);
    return Py_BuildValue("s", imagePath.c_str());
}

PyObject *guildGetMarkIndexByMarkID(PyObject *poSelf, PyObject *poArgs)
{
    int markID = 0;

    if (!PyTuple_GetInteger(poArgs, 0, &markID))
        return Py_BuildException();

    return Py_BuildValue("i", markID % CGuildMarkImage::MARK_TOTAL_COUNT);
}

void initguild()
{
    static PyMethodDef s_methods[] = {
        // Enable
        {"IsGuildEnable", guildIsGuildEnable, METH_VARARGS},
        {"GuildIDToMarkID", guildGuildIDToMarkID, METH_VARARGS},
        {"GetMarkImageFilenameByMarkID", guildGetMarkImageFilenameByMarkID, METH_VARARGS},
        {"GetMarkIndexByMarkID", guildGetMarkIndexByMarkID, METH_VARARGS},

        // GuildInfo
        {"GetGuildID", guildGetGuildID, METH_VARARGS},
        {"HasGuildLand", guildHasGuildLand, METH_VARARGS},
        {"GetGuildName", guildGetGuildName, METH_VARARGS},
        {"GetGuildMasterName", guildGetGuildMasterName, METH_VARARGS},
        {"GetEnemyGuildName", guildGetEnemyGuildName, METH_VARARGS},
        {"GetGuildMoney", guildGetGuildMoney, METH_VARARGS},

        // BoardPage
        {"GetGuildBoardCommentCount", guildGetGuildBoardCommentCount, METH_VARARGS},
        {"GetGuildBoardCommentData", guildGetGuildBoardCommentData, METH_VARARGS},

        // MemberPage
        {"GetGuildLevel", guildGetGuildLevel, METH_VARARGS},
        {"GetGuildExperience", guildGetGuildExperience, METH_VARARGS},
        {"GetGuildMemberCount", guildGetGuildMemberCount, METH_VARARGS},
        {"GetGuildMemberLevelSummary", guildGetGuildMemberLevelSummary, METH_VARARGS},
        {"GetGuildMemberLevelAverage", guildGetGuildMemberLevelAverage, METH_VARARGS},
        {"GetGuildExperienceSummary", guildGetGuildExperienceSummary, METH_VARARGS},

        // SkillPage
        {"GetGuildSkillPoint", guildGetGuildSkillPoint, METH_VARARGS},
        {"GetDragonPowerPoint", guildGetDragonPowerPoint, METH_VARARGS},
        {"GetGuildSkillLevel", guildGetGuildSkillLevel, METH_VARARGS},
        {"GetSkillLevel", guildGetSkillLevel, METH_VARARGS},
        {"GetSkillMaxLevelNew", guildGetSkillMaxLevelNew, METH_VARARGS},

        {"SetSkillIndex", guildSetSkillIndex, METH_VARARGS},
        {"GetSkillIndex", guildGetSkillIndex, METH_VARARGS},

        // GradePage
        {"GetGradeData", guildGetGradeData, METH_VARARGS},
        {"GetGradeName", guildGetGradeName, METH_VARARGS},

        // About Member
        {"GetMemberCount", guildGetMemberCount, METH_VARARGS},
        {"GetMemberData", guildGetMemberData, METH_VARARGS},
        {"MemberIndexToPID", guildMemberIndexToPID, METH_VARARGS},
        {"IsMember", guildIsMember, METH_VARARGS},
        {"IsMemberByName", guildIsMemberByName, METH_VARARGS},
        {"MainPlayerHasAuthority", guildMainPlayerHasAuthority, METH_VARARGS},

        // Guild
        {"Destroy", guildDestroy, METH_VARARGS},

        {nullptr, nullptr, NULL},
    };

    PyObject *poModule = Py_InitModule("guild", s_methods);
    PyModule_AddIntConstant(poModule, "AUTH_ADD_MEMBER", GUILD_AUTH_ADD_MEMBER);
    PyModule_AddIntConstant(poModule, "AUTH_REMOVE_MEMBER", GUILD_AUTH_REMOVE_MEMBER);
    PyModule_AddIntConstant(poModule, "AUTH_NOTICE", GUILD_AUTH_NOTICE);
    PyModule_AddIntConstant(poModule, "AUTH_SKILL", GUILD_AUTH_SKILL);
    PyModule_AddIntConstant(poModule, "ENEMY_GUILD_SLOT_MAX_COUNT", CPythonGuild::ENEMY_GUILD_SLOT_MAX_COUNT);
}
