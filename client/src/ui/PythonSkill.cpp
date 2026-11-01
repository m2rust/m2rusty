#include <Client/UI/StdAfx.h>
#include <Client/UI/PythonSkill.h>

#include <Neoeter/Platform/Base/Poly/Poly.h>
#include <Neoeter/IO/EterPackManager.h>
#include <Client/UI/InstanceBase.h>
#include <Client/UI/PythonPlayer.h>
#include <math.h>

#include <algorithm>
#include <print>

#include <utility>

std::map<std::string, DWORD> CPythonSkill::SSkillData::ms_StatusNameMap;
std::map<std::string, DWORD> CPythonSkill::SSkillData::ms_NewMinStatusNameMap;
std::map<std::string, DWORD> CPythonSkill::SSkillData::ms_NewMaxStatusNameMap;
DWORD CPythonSkill::SSkillData::ms_dwTimeIncreaseSkillNumber = 0;

BOOL SKILL_EFFECT_UPGRADE_ENABLE = FALSE;

int SplitLine(const char *c_szText, CTokenVector *pstTokenVector, const char *c_szDelimeter)
{
    pstTokenVector->reserve(10);
    pstTokenVector->clear();

    std::string const stToken;
    std::string stLine = c_szText;

    DWORD basePos = 0;

    do
    {
        int beginPos = stLine.find_first_not_of(c_szDelimeter, basePos);

        if (beginPos < 0)
            return -1;

        int endPos = 0;

        if (stLine[beginPos] == '"')
        {
            ++beginPos;
            endPos = stLine.find_first_of('\"', beginPos);

            if (endPos < 0)
                return -2;

            basePos = endPos + 1;
        }
        else
        {
            endPos = stLine.find_first_of(c_szDelimeter, beginPos);
            basePos = endPos;
        }

        pstTokenVector->push_back(stLine.substr(beginPos, endPos - beginPos));

        // 추가 코드. 맨뒤에 탭이 있는 경우를 체크한다. - [levites]
        if (static_cast<int>(stLine.find_first_not_of(c_szDelimeter, basePos)) < 0)
            break;
    } while (basePos < stLine.length());

    return 0;
}

// OVERWRITE_SKILLPROTO_POLY
void string_replace_word(const char *base, int base_len, const char *src, int src_len, const char *dst, int dst_len,
                         std::string &result)
{
    result.resize(0);
    if (base_len > 0 && dst_len > src_len)
    {
        result.reserve(base_len + ((dst_len - src_len) * (base_len / src_len)));
    }
    else
    {
        result.reserve(base_len);
    }

    int prev = 0;
    int cur = 0;
    while (cur < base_len)
    {
        if (memcmp(base + cur, src, src_len) == 0)
        {
            result.append(base + prev, cur - prev);
            result.append(dst, dst_len);
            cur += src_len;
            prev = cur;
        }
        else
        {
            ++cur;
        }
    }
    result.append(base + prev, cur - prev);
}
// END_OF_OVERWRITE_SKILLPROTO_POLY

bool CPythonSkill::RegisterSkillTable(const char *c_szFileName)
{
    CEterPackManager::TPackDataPtr data;
    CMemoryTextFileLoader textFileLoader;

    if (!CEterPackManager::Instance().Get(c_szFileName, data))
        return false;

    textFileLoader.Bind(data->size(), data->data());

    // OVERWRITE_SKILLPROTO_POLY
    std::string src_poly_rand;
    std::string src_poly_atk;
    std::string src_poly_mwep;
    // END_OF_OVERWRITE_SKILLPROTO_POLY

    CTokenVector TokenVector;
    for (DWORD i = 0; i < textFileLoader.GetLineCount(); ++i)
    {
        if (!textFileLoader.SplitLineByTab(i, &TokenVector))
            continue;

        std::string const strLine = textFileLoader.GetLineString(i);

        if (TABLE_TOKEN_TYPE_MAX_NUM != TokenVector.size())
        {
            Tracef("CPythonSkill::RegisterSkillTable(%s) - Strange Token Count "
                   "[Line:%d / TokenCount:%d]\n",
                   c_szFileName, i, TokenVector.size());
            continue;
        }

        int const iVnum = atoi(TokenVector[TABLE_TOKEN_TYPE_VNUM].c_str());

        auto itor = m_SkillDataMap.find(iVnum);
        if (m_SkillDataMap.end() == itor)
        {
            Tracef("CPythonSkill::RegisterSkillTable(%s) - NOT EXIST SkillDesc "
                   "[Vnum:%d Line:%d]\n",
                   c_szFileName, iVnum, i);
            continue;
        }

        TSkillData &rSkillData = itor->second;

        const std::string &c_strSPCostPoly = TokenVector[TABLE_TOKEN_TYPE_SP_COST_POLY];
        if (!c_strSPCostPoly.empty())
        {
            rSkillData.strNeedSPFormula = c_strSPCostPoly;
        }

        const std::string &c_strCooldownPoly = TokenVector[TABLE_TOKEN_TYPE_COOLDOWN_POLY];
        if (!c_strCooldownPoly.empty())
        {
            rSkillData.strCoolTimeFormula = c_strCooldownPoly;
        }

        const std::string &c_strDurationSPCostPoly = TokenVector[TABLE_TOKEN_TYPE_DURATION_SP_COST_POLY];
        if (!c_strDurationSPCostPoly.empty())
        {
            rSkillData.strContinuationSPFormula = c_strDurationSPCostPoly;
        }

        const std::string &c_strTargetRange = TokenVector[TABLE_TOKEN_TYPE_TARGET_RANGE];
        if (!c_strTargetRange.empty())
        {
            rSkillData.dwTargetRange = atoi(c_strTargetRange.c_str());
        }

        rSkillData.strDuration = TokenVector[TABLE_TOKEN_TYPE_DURATION_POLY];

        const DWORD LevelEmpty = 1;

        const std::string &c_strMaxLevel = TokenVector[TABLE_TOKEN_TYPE_MAX_LEVEL];
        if (!c_strMaxLevel.empty())
        {
            int const maxLevel = atoi(c_strMaxLevel.c_str());
            if (maxLevel > LevelEmpty)
                rSkillData.byMaxLevel = maxLevel;
        }

        const std::string &c_strLevelLimit = TokenVector[TABLE_TOKEN_TYPE_LEVEL_LIMIT];
        if (!c_strLevelLimit.empty())
        {
            int const levelLimit = atoi(c_strLevelLimit.c_str());
            if (rSkillData.byLevelLimit > LevelEmpty)
                rSkillData.byLevelLimit = levelLimit;
        }
        const std::string &c_strPointPoly = TokenVector[TABLE_TOKEN_TYPE_POINT_POLY];

        // OVERWRITE_SKILLPROTO_POLY
        bool USE_SKILL_PROTO = !LocaleService_IsCHEONMA();

        switch (iVnum)
        {
        case 34:

        // GUILD_SKILL_DISPLAY_BUG_FIX
        case 151:
        case 152:
        case 153:
        case 154:
        case 155:
        case 156:
        case 157:
            // END_OF_GUILD_SKILL_DISPLAY_BUG_FIX
            USE_SKILL_PROTO = false;
            break;
        }

        if (!rSkillData.AffectDataVector.empty() && USE_SKILL_PROTO)
        {
            TAffectData &affect = rSkillData.AffectDataVector[0];

            if (strstr(c_strPointPoly.c_str(), "atk") != nullptr || strstr(c_strPointPoly.c_str(), "mwep") != nullptr ||
                strstr(c_strPointPoly.c_str(), "number") != nullptr)
            {
                src_poly_rand = "";
                src_poly_atk = "";
                src_poly_mwep = "";

                // MIN
                string_replace_word(c_strPointPoly.c_str(), c_strPointPoly.length(), "number", 6, "min", 3,
                                    src_poly_rand);
                string_replace_word(src_poly_rand.c_str(), src_poly_rand.length(), "atk", 3, "minatk", 6, src_poly_atk);
                string_replace_word(src_poly_atk.c_str(), src_poly_atk.length(), "mwep", 4, "minmwep", 7,
                                    affect.strAffectMinFormula);
                // END_OF_MIN

                // MAX
                string_replace_word(c_strPointPoly.c_str(), c_strPointPoly.length(), "number", 6, "max", 3,
                                    src_poly_rand);
                string_replace_word(src_poly_rand.c_str(), src_poly_rand.length(), "atk", 3, "maxatk", 6, src_poly_atk);
                string_replace_word(src_poly_atk.c_str(), src_poly_atk.length(), "mwep", 4, "maxmwep", 7,
                                    affect.strAffectMaxFormula);
                // END_OF_MAX

                switch (iVnum)
                {
                case 1: // 삼연참
                    affect.strAffectMinFormula += "* 3";
                    affect.strAffectMaxFormula += "* 3";
                    break;
                }
            }
            else
            {
                affect.strAffectMinFormula = c_strPointPoly;
                affect.strAffectMaxFormula = "";
            }
        }
        // END_OF_OVERWRITE_SKILLPROTO_POLY
    }

    return true;
}

void CPythonSkill::__RegisterGradeIconImage(TSkillData &rData, const char *c_szHeader, const char *c_szImageName)
{
    for (int j = 0; j < SKILL_GRADE_COUNT; ++j)
    {
        TGradeData &rGradeData = rData.GradeData[j];

        char szCount[8 + 1];
        _snprintf(szCount, sizeof(szCount), "_%02d", j + 1);

        std::string strFileName;
        strFileName += c_szHeader;
        strFileName += c_szImageName;
        strFileName += szCount;
        strFileName += ".sub";
        rGradeData.pImage = (CGraphicImage *)CResourceManager::Instance().GetResourcePointer(strFileName.c_str());
    }
}

void CPythonSkill::__RegisterNormalIconImage(TSkillData &rData, const char *c_szHeader, const char *c_szImageName)
{
    std::string strFileName;
    strFileName += c_szHeader;
    strFileName += c_szImageName;
    strFileName += ".sub";
    rData.pImage = (CGraphicImage *)CResourceManager::Instance().GetResourcePointer(strFileName.c_str());
    for (int j = 0; j < SKILL_GRADE_COUNT; ++j)
    {
        TGradeData &rGradeData = rData.GradeData[j];
        rGradeData.pImage = rData.pImage;
    }
}
extern const DWORD c_iSkillIndex_Riding;
bool CPythonSkill::RegisterSkillDesc(const char *c_szFileName)
{
    CEterPackManager::TPackDataPtr data;
    CMemoryTextFileLoader textFileLoader;

    if (!CEterPackManager::Instance().Get(c_szFileName, data))
        return false;

    textFileLoader.Bind(data->size(), data->data());

    CTokenVector TokenVector;
    for (DWORD i = 0; i < textFileLoader.GetLineCount(); ++i)
    {
        if (!textFileLoader.SplitLineByTab(i, &TokenVector))
            continue;

        if (DESC_TOKEN_TYPE_VNUM >= TokenVector.size())
        {
            TraceError("SkillDesc.line(%d).NO_VNUM_ERROR\n", i + 1);
            continue;
        }

        DWORD const iSkillIndex = atoi(TokenVector[DESC_TOKEN_TYPE_VNUM].c_str());
        if (iSkillIndex == 0)
        {
            TraceError("SkillDesc.line(%d).NO_INDEX_ERROR\n", i + 1);
            continue;
        }

        if (DESC_TOKEN_TYPE_JOB >= TokenVector.size())
        {
            TraceError("SkillDesc.line(%d).NO_JOB_ERROR\n", i + 1);
            continue;
        }

        m_SkillDataMap.insert(std::make_pair(iSkillIndex, TSkillData()));
        TSkillData &rSkillData = m_SkillDataMap[iSkillIndex];

        // Vnum
        rSkillData.dwSkillIndex = iSkillIndex;

        // Type
        {
            std::string strJob = TokenVector[DESC_TOKEN_TYPE_JOB];
            stl_lowers(strJob);

            auto it = m_SkillTypeIndexMap.find(strJob);
            if (m_SkillTypeIndexMap.end() == it)
            {
                TraceError("CPythonSkill::RegisterSkillDesc(dwSkillIndex=%d, "
                           "strType=%s).STRANGE_SKILL_TYPE",
                           iSkillIndex, strJob.c_str());
                continue;
            }

            rSkillData.byType = static_cast<BYTE>(it->second);
        }

        // Name
        {
            rSkillData.strName = TokenVector[DESC_TOKEN_TYPE_NAME1];
            rSkillData.GradeData[0].strName = TokenVector[DESC_TOKEN_TYPE_NAME1];
            rSkillData.GradeData[1].strName = TokenVector[DESC_TOKEN_TYPE_NAME2];
            rSkillData.GradeData[2].strName = TokenVector[DESC_TOKEN_TYPE_NAME3];

            if (rSkillData.GradeData[1].strName.empty())
                rSkillData.GradeData[1].strName = TokenVector[DESC_TOKEN_TYPE_NAME1];
            if (rSkillData.GradeData[2].strName.empty())
                rSkillData.GradeData[2].strName = TokenVector[DESC_TOKEN_TYPE_NAME1];
        }

        // Description
        {
            rSkillData.strDescription = TokenVector[DESC_TOKEN_TYPE_DESCRIPTION];
        }

        // Condition
        {
            rSkillData.ConditionDataVector.clear();
            for (int j = 0; j < CONDITION_COLUMN_COUNT; ++j)
            {
                const std::string &c_rstrCondition = TokenVector[DESC_TOKEN_TYPE_CONDITION1 + j];
                if (!c_rstrCondition.empty())
                    rSkillData.ConditionDataVector.push_back(c_rstrCondition);
            }
        }

        // Attribute
        {
            const std::string &c_rstrAttribute = TokenVector[DESC_TOKEN_TYPE_ATTRIBUTE];
            CTokenVector AttributeTokenVector;
            SplitLine(c_rstrAttribute.c_str(), &AttributeTokenVector, "|");

            for (auto &rstrToken : AttributeTokenVector)
            {
                auto it2 = m_SkillAttributeIndexMap.find(rstrToken);
                if (m_SkillAttributeIndexMap.end() == it2)
                {
                    TraceError("CPythonSkill::RegisterSkillDesc(dwSkillIndex=%d) "
                               "- Strange Skill Attribute(%s)",
                               iSkillIndex, rstrToken.c_str());
                    continue;
                }
                rSkillData.dwSkillAttribute |= it2->second;
            }
        }

        // Weapon
        {
            const std::string &c_rstrWeapon = TokenVector[DESC_TOKEN_TYPE_WEAPON];
            CTokenVector WeaponTokenVector;
            SplitLine(c_rstrWeapon.c_str(), &WeaponTokenVector, "|");

            for (auto &rstrToken : WeaponTokenVector)
            {
                auto it2 = m_SkillNeedWeaponIndexMap.find(rstrToken);
                if (m_SkillNeedWeaponIndexMap.end() == it2)
                {
                    TraceError("CPythonSkill::RegisterSkillDesc(dwSkillIndex=%d) "
                               "- Strange Skill Need Weapon(%s)",
                               iSkillIndex, rstrToken.c_str());
                    continue;
                }
                rSkillData.dwNeedWeapon |= it2->second;
            }
        }

        // Icon Name
        {
            const std::string &c_rstrJob = TokenVector[DESC_TOKEN_TYPE_JOB];
            const std::string &c_rstrIconName = TokenVector[DESC_TOKEN_TYPE_ICON_NAME];

            // NOTE : Support 스킬임에도 3단계를 지원해야 해서 예외 처리 - [levites]
            if (c_iSkillIndex_Riding == iSkillIndex)
            {
                char szIconFileNameHeader[64 + 1];
                _snprintf(szIconFileNameHeader, sizeof(szIconFileNameHeader), "%sskill/common/support/",
                          g_strImagePath.c_str());

                __RegisterGradeIconImage(rSkillData, szIconFileNameHeader, c_rstrIconName.c_str());
            }
            else if (m_PathNameMap.contains(c_rstrJob))
            {
                char szIconFileNameHeader[64 + 1];
                _snprintf(szIconFileNameHeader, sizeof(szIconFileNameHeader), "%sskill/%s/", g_strImagePath.c_str(),
                          m_PathNameMap[c_rstrJob].c_str());

                switch (rSkillData.byType)
                {
                case SKILL_TYPE_ACTIVE: {
                    __RegisterGradeIconImage(rSkillData, szIconFileNameHeader, c_rstrIconName.c_str());
                    break;
                }
                case SKILL_TYPE_SUPPORT:
                case SKILL_TYPE_GUILD:
                case SKILL_TYPE_HORSE: {
                    __RegisterNormalIconImage(rSkillData, szIconFileNameHeader, c_rstrIconName.c_str());
                    break;
                }
                }
            }
        }

        // Motion Index
        if (TokenVector.size() > DESC_TOKEN_TYPE_MOTION_INDEX)
        {
            const std::string &c_rstrMotionIndex = TokenVector[DESC_TOKEN_TYPE_MOTION_INDEX];
            if (c_rstrMotionIndex.empty())
            {
                rSkillData.bNoMotion = TRUE;
                rSkillData.wMotionIndex = 0;
            }
            else
            {
                rSkillData.bNoMotion = FALSE;
                rSkillData.wMotionIndex = static_cast<WORD>(atoi(c_rstrMotionIndex.c_str()));
            }
        }
        else
        {
            rSkillData.wMotionIndex = 0;
        }

        if (TokenVector.size() > DESC_TOKEN_TYPE_TARGET_COUNT_FORMULA)
            rSkillData.strTargetCountFormula = TokenVector[DESC_TOKEN_TYPE_TARGET_COUNT_FORMULA];
        if (TokenVector.size() > DESC_TOKEN_TYPE_MOTION_LOOP_COUNT_FORMULA)
            rSkillData.strMotionLoopCountFormula = TokenVector[DESC_TOKEN_TYPE_MOTION_LOOP_COUNT_FORMULA];

        rSkillData.AffectDataNewVector.clear();
        rSkillData.AffectDataNewVector.reserve(3);

        // Affect
        for (int j = 0; j < AFFECT_COLUMN_COUNT; ++j)
        {
            int const iDescriptionSlotIndex = DESC_TOKEN_TYPE_AFFECT_DESCRIPTION_1 + (j * AFFECT_STEP_COUNT);
            int const iMinSlotIndex = DESC_TOKEN_TYPE_AFFECT_MIN_1 + (j * AFFECT_STEP_COUNT);
            int const iMaxSlotIndex = DESC_TOKEN_TYPE_AFFECT_MAX_1 + (j * AFFECT_STEP_COUNT);

            if (TokenVector.size() > iDescriptionSlotIndex)
            {
                if (!TokenVector[iDescriptionSlotIndex].empty())
                {
                    rSkillData.AffectDataVector.emplace_back();

                    TAffectData &rAffectData = *rSkillData.AffectDataVector.rbegin();
                    rAffectData.strAffectDescription = "";
                    rAffectData.strAffectMinFormula = "";
                    rAffectData.strAffectMaxFormula = "";

                    rAffectData.strAffectDescription = TokenVector[iDescriptionSlotIndex];
                    if (TokenVector.size() > iMinSlotIndex)
                    {
                        if (!TokenVector[iMinSlotIndex].empty())
                            rAffectData.strAffectMinFormula = TokenVector[iMinSlotIndex];
                    }
                    if (TokenVector.size() > iMaxSlotIndex)
                    {
                        if (!TokenVector[iMaxSlotIndex].empty())
                            rAffectData.strAffectMaxFormula = TokenVector[iMaxSlotIndex];
                    }
                }
            }
        }

        if (TokenVector.size() > DESC_TOKEN_TYPE_MOTION_INDEX_GRADE_NUM)
        {
            int const numGrade = atoi(TokenVector[DESC_TOKEN_TYPE_MOTION_INDEX_GRADE_NUM].c_str());
            if (SKILL_EFFECT_COUNT < numGrade)
            {
                TraceError("%s[%s] has exceeded its rating limit [%d].", rSkillData.strName.c_str(),
                           TokenVector[DESC_TOKEN_TYPE_MOTION_INDEX_GRADE_NUM].c_str(), SKILL_EFFECT_COUNT);
                return false;
            }
            for (int iGrade = 0; iGrade < numGrade; iGrade++)
            {
                // if (iGrade == SKILL_GRADE_COUNT)
                //	rSkillData.GradeData[iGrade] =
                // rSkillData.GradeData[iGrade-1];
                rSkillData.GradeData[iGrade].wMotionIndex = rSkillData.wMotionIndex + (iGrade * SKILL_GRADEGAP);
            }
            for (int iSpace = numGrade; iSpace < SKILL_EFFECT_COUNT; iSpace++)
            {
                rSkillData.GradeData[iSpace].wMotionIndex = rSkillData.wMotionIndex;
            }
        }

        if (TokenVector.size() > DESC_TOKEN_TYPE_LEVEL_LIMIT)
        {
            const std::string &c_rstrLevelLimit = TokenVector[DESC_TOKEN_TYPE_LEVEL_LIMIT];
            if (c_rstrLevelLimit.empty())
            {
                rSkillData.byLevelLimit = 0;
            }
            else
            {
                rSkillData.byLevelLimit = static_cast<WORD>(atoi(c_rstrLevelLimit.c_str()));
            }
        }

        if (TokenVector.size() > DESC_TOKEN_TYPE_MAX_LEVEL)
        {
            const std::string &c_rstrMaxLevel = TokenVector[DESC_TOKEN_TYPE_MAX_LEVEL];
            const DWORD LevelLimitEmpty = 1;
            if (c_rstrMaxLevel.empty())
            {
                if (rSkillData.byLevelLimit > LevelLimitEmpty)
                {
                    rSkillData.byMaxLevel = rSkillData.byLevelLimit;
                }
                else
                {
                    rSkillData.byMaxLevel = 20;
                }
            }
            else
            {
                rSkillData.byMaxLevel = static_cast<BYTE>(atoi(c_rstrMaxLevel.c_str()));
            }
        }
    }

    return true;
}

void CPythonSkill::Destroy()
{
    m_SkillDataMap.clear();
}

bool CPythonSkill::RegisterSkill(DWORD dwSkillIndex, const char *c_szFileName)
{
    CTextFileLoader TextFileLoader;
    if (!TextFileLoader.Load(c_szFileName))
    {
        TraceError("CPythonSkill::RegisterSkill(dwSkillIndex=%d, "
                   "c_szFileName=%s) - Failed to find file",
                   dwSkillIndex, c_szFileName);
        return false;
    }

    TextFileLoader.SetTop();

    TSkillData SkillData;
    SkillData.dwSkillIndex = dwSkillIndex;

    std::string strTypeName;
    if (TextFileLoader.GetTokenString("type", &strTypeName))
    {
        stl_lowers(strTypeName);
        auto it = m_SkillTypeIndexMap.find(strTypeName);
        if (m_SkillTypeIndexMap.end() == it)
        {
            TraceError("Strange Skill Type - "
                       "CPythonSkill::RegisterSkill(dwSkillIndex=%d, c_szFileName=%s)",
                       dwSkillIndex, c_szFileName);
            return false;
        }

        SkillData.byType = static_cast<BYTE>(it->second);
    }
    else
    {
        TraceError("CPythonSkill::RegisterSkill(dwSkillIndex=%d, "
                   "c_szFileName=%s) - Failed to find key [type]",
                   dwSkillIndex, c_szFileName);
        return false;
    }

    CTokenVector *pAttributeTokenVector = nullptr;
    if (TextFileLoader.GetTokenVector("attribute", &pAttributeTokenVector))
    {
        for (auto &rstrToken : *pAttributeTokenVector)
        {
            auto it2 = m_SkillAttributeIndexMap.find(rstrToken);
            if (m_SkillAttributeIndexMap.end() == it2)
            {
                TraceError("Strange Skill Attribute - "
                           "CPythonSkill::RegisterSkill(dwSkillIndex=%d, "
                           "c_szFileName=%s)",
                           dwSkillIndex, c_szFileName);
                continue;
            }
            SkillData.dwSkillAttribute |= it2->second;
        }
    }

    CTokenVector *pNeedWeaponVector = nullptr;
    if (TextFileLoader.GetTokenVector("needweapon", &pNeedWeaponVector))
    {
        for (auto &rstrToken : *pNeedWeaponVector)
        {
            auto it2 = m_SkillNeedWeaponIndexMap.find(rstrToken);
            if (m_SkillNeedWeaponIndexMap.end() == it2)
            {
                TraceError("Strange Skill Need Weapon - "
                           "CPythonSkill::RegisterSkill(dwSkillIndex=%d, "
                           "c_szFileName=%s)",
                           dwSkillIndex, c_szFileName);
                continue;
            }
            SkillData.dwNeedWeapon |= it2->second;
        }
    }

    {
        char szName[256];
        sprintf(szName, "%dname", LocaleService_GetCodePage());
        if (!TextFileLoader.GetTokenString(szName, &SkillData.strName))
        {
            if (!TextFileLoader.GetTokenString("name", &SkillData.strName))
            {
                TraceError("CPythonSkill::RegisterSkill(dwSkillIndex=%d, "
                           "c_szFileName=%s) - Failed to find [%s]",
                           dwSkillIndex, c_szFileName, szName);
                return false;
            }
        }
    }

    {
        char szName[256];
        sprintf(szName, "%ddescription", LocaleService_GetCodePage());
        if (!TextFileLoader.GetTokenString(szName, &SkillData.strDescription))
            TextFileLoader.GetTokenString("description", &SkillData.strDescription);
    }

    if (!TextFileLoader.GetTokenString("iconfilename", &SkillData.strIconFileName))
    {
        TraceError("CPythonSkill::RegisterSkill(dwSkillIndex=%d, "
                   "c_szFileName=%s) - Failed to find [iconfilename]",
                   dwSkillIndex, c_szFileName);
        return false;
    }

    {
        CTokenVector *pConditionDataVector = nullptr;

        char szConditionData[256];
        sprintf(szConditionData, "%dconditiondata", LocaleService_GetCodePage());

        bool isConditionData = true;
        if (!TextFileLoader.GetTokenVector(szConditionData, &pConditionDataVector))
        {
            if (!TextFileLoader.GetTokenVector("conditiondata", &pConditionDataVector))
                isConditionData = false;
        }

        if (isConditionData)
        {
            DWORD const dwSize = pConditionDataVector->size();
            SkillData.ConditionDataVector.clear();
            SkillData.ConditionDataVector.resize(dwSize);
            for (DWORD i = 0; i < dwSize; ++i)
            {
                SkillData.ConditionDataVector[i] = pConditionDataVector->at(i);
            }
        }
    }

    {
        CTokenVector *pAffectDataVector = nullptr;

        char szAffectData[256];
        sprintf(szAffectData, "%daffectdata", LocaleService_GetCodePage());

        bool isAffectData = true;
        if (!TextFileLoader.GetTokenVector(szAffectData, &pAffectDataVector))
        {
            if (!TextFileLoader.GetTokenVector("affectdata", &pAffectDataVector))
                isAffectData = false;
        }

        if (isAffectData)
        {
            DWORD const dwSize = pAffectDataVector->size() / 3;
            SkillData.AffectDataVector.clear();
            SkillData.AffectDataVector.resize(dwSize);
            for (DWORD i = 0; i < dwSize; ++i)
            {
                SkillData.AffectDataVector[i].strAffectDescription = pAffectDataVector->at((i * 3) + 0);
                SkillData.AffectDataVector[i].strAffectMinFormula = pAffectDataVector->at((i * 3) + 1);
                SkillData.AffectDataVector[i].strAffectMaxFormula = pAffectDataVector->at((i * 3) + 2);
            }
        }
    }

    {
        CTokenVector *pGradeDataVector = nullptr;

        char szGradeData[256];
        sprintf(szGradeData, "%dgradedata", LocaleService_GetCodePage());

        if (TextFileLoader.GetTokenVector(szGradeData, &pGradeDataVector))
        {
            if (SKILL_GRADE_COUNT * 2 != pGradeDataVector->size())
            {
                TraceError("CPythonSkill::RegisterSkill(dwSkillIndex=%d, "
                           "c_szFileName=%s) - Strange Grade Data Count",
                           dwSkillIndex, c_szFileName);
            }

            for (DWORD i = 0; i < min(SKILL_GRADE_COUNT, pGradeDataVector->size() / 2); ++i)
            {
                SkillData.GradeData[i].strName = pGradeDataVector->at((i * 2) + 0);
                std::string const strIconFileName = g_strImagePath + pGradeDataVector->at((i * 2) + 1);
                SkillData.GradeData[i].pImage =
                    (CGraphicImage *)CResourceManager::Instance().GetResourcePointer(strIconFileName.c_str());
            }
        }
    }

    TextFileLoader.GetTokenString("cooltimeformula", &SkillData.strCoolTimeFormula);
    TextFileLoader.GetTokenString("targetcountformula", &SkillData.strTargetCountFormula);
    TextFileLoader.GetTokenString("motionloopcountformula", &SkillData.strMotionLoopCountFormula);
    TextFileLoader.GetTokenString("needspformula", &SkillData.strNeedSPFormula);
    TextFileLoader.GetTokenString("continuationspformula", &SkillData.strContinuationSPFormula);
    TextFileLoader.GetTokenDoubleWord("range", &SkillData.dwTargetRange);
    TextFileLoader.GetTokenByte("maxlevel", &SkillData.byMaxLevel);
    TextFileLoader.GetTokenByte("leveluppoint", &SkillData.byLevelUpPoint);

    WORD wMotionIndex = 0;
    if (TextFileLoader.GetTokenWord("motionindex", &wMotionIndex))
    {
        SkillData.wMotionIndex = wMotionIndex;
    }
    else
    {
        SkillData.wMotionIndex = 0;
    }

    WORD wMotionIndexForMe = 0;
    if (TextFileLoader.GetTokenWord("motionindexforme", &wMotionIndexForMe))
    {
        SkillData.wMotionIndexForMe = wMotionIndexForMe;
    }
    else
    {
        SkillData.wMotionIndexForMe = 0;
    }

    SkillData.strIconFileName = g_strImagePath + SkillData.strIconFileName;
    SkillData.pImage =
        (CGraphicImage *)CResourceManager::Instance().GetResourcePointer(SkillData.strIconFileName.c_str());

    m_SkillDataMap.insert(TSkillDataMap::value_type(dwSkillIndex, SkillData));

    /////

    if (SkillData.IsTimeIncreaseSkill())
    {
        CPythonSkill::SSkillData::ms_dwTimeIncreaseSkillNumber = SkillData.dwSkillIndex;
    }

    return true;
}

BOOL CPythonSkill::GetSkillData(DWORD dwSkillIndex, TSkillData **ppSkillData)
{
    auto it = m_SkillDataMap.find(dwSkillIndex);

    if (m_SkillDataMap.end() == it)
        return FALSE;

    *ppSkillData = &(it->second);
    return TRUE;
}

bool CPythonSkill::GetSkillDataByName(const char *c_szName, TSkillData **ppSkillData)
{
    auto itor = m_SkillDataMap.begin();
    for (; itor != m_SkillDataMap.end(); ++itor)
    {
        TSkillData *pData = &(itor->second);
        if (c_szName == pData->strName)
        {
            *ppSkillData = &(itor->second);
            return true;
        }
    }
    return false;
}

void CPythonSkill::SetPathName(const char *c_szFileName)
{
    m_strPathName = c_szFileName;
}

const char *CPythonSkill::GetPathName()
{
    return m_strPathName.c_str();
}

void CPythonSkill::TEST()
{
    BOOL isFirst = 0;
    std::map<std::string, DWORD>::iterator itorSub;

    FILE *File = fopen("test.txt", "w");

    for (auto &itor : m_SkillDataMap)
    {
        TSkillData &rSkillData = itor.second;

        std::string strLine;
        strLine += rSkillData.strName;
        strLine += "\t";
        // Name2
        strLine += "\t";
        // Name3
        strLine += "\t";
        strLine += rSkillData.strDescription;
        strLine += "\t";
        if (!rSkillData.ConditionDataVector.empty())
            strLine += rSkillData.ConditionDataVector[0];
        strLine += "\t";
        if (rSkillData.ConditionDataVector.size() > 1)
            strLine += rSkillData.ConditionDataVector[1];
        strLine += "\t";
        if (rSkillData.ConditionDataVector.size() > 2)
            strLine += rSkillData.ConditionDataVector[2];
        strLine += "\t";
        if (rSkillData.ConditionDataVector.size() > 3)
            strLine += rSkillData.ConditionDataVector[3];

        strLine += "\t";
        isFirst = TRUE;
        for (itorSub = m_SkillAttributeIndexMap.begin(); itorSub != m_SkillAttributeIndexMap.end(); ++itorSub)
        {
            if (itorSub->second & rSkillData.dwSkillAttribute)
            {
                if (isFirst)
                {
                    isFirst = FALSE;
                }
                else
                {
                    strLine += "|";
                }
                strLine += itorSub->first;
            }
        }

        strLine += "\t";
        isFirst = TRUE;
        for (itorSub = m_SkillNeedWeaponIndexMap.begin(); itorSub != m_SkillNeedWeaponIndexMap.end(); ++itorSub)
        {
            if (itorSub->second & rSkillData.dwNeedWeapon)
            {
                if (isFirst)
                {
                    isFirst = FALSE;
                }
                else
                {
                    strLine += "|";
                }
                strLine += itorSub->first;
            }
        }

        strLine += "\t";
        std::string strFileName = rSkillData.strIconFileName;
        int const iPos = strFileName.find_last_of('/', rSkillData.strIconFileName.length());
        if (iPos > 0)
            strFileName = strFileName.substr(iPos + 1, strFileName.length() - iPos - 4 - 1);
        strLine += strFileName;

        strLine += "\t";
        char szMotionIndex[32 + 1];
        _snprintf(szMotionIndex, sizeof(szMotionIndex), "%d", rSkillData.wMotionIndex);
        strLine += szMotionIndex;

        strLine += "\t";
        if (rSkillData.wMotionIndexForMe > 1)
        {
            char szMotionIndexForMe[32 + 1];
            _snprintf(szMotionIndexForMe, sizeof(szMotionIndexForMe), "%d", rSkillData.wMotionIndexForMe);
            strLine += szMotionIndexForMe;
        }

        assert(rSkillData.AffectDataVector.size() <= 3);
        strLine += "\t";
        if (!rSkillData.AffectDataVector.empty())
        {
            strLine += rSkillData.AffectDataVector[0].strAffectDescription;
            strLine += "\t";
            strLine += rSkillData.AffectDataVector[0].strAffectMinFormula;
            strLine += "\t";
            strLine += rSkillData.AffectDataVector[0].strAffectMaxFormula;
        }
        strLine += "\t";
        if (rSkillData.AffectDataVector.size() > 1)
        {
            strLine += rSkillData.AffectDataVector[1].strAffectDescription;
            strLine += "\t";
            strLine += rSkillData.AffectDataVector[1].strAffectMinFormula;
            strLine += "\t";
            strLine += rSkillData.AffectDataVector[1].strAffectMaxFormula;
        }
        strLine += "\t";
        if (rSkillData.AffectDataVector.size() > 2)
        {
            strLine += rSkillData.AffectDataVector[2].strAffectDescription;
            strLine += "\t";
            strLine += rSkillData.AffectDataVector[2].strAffectMinFormula;
            strLine += "\t";
            strLine += rSkillData.AffectDataVector[2].strAffectMaxFormula;
        }

        std::println(File, "{}", strLine);
    }

    fclose(File);
}

CPythonSkill::CPythonSkill()
{
    m_SkillTypeIndexMap.insert(std::map<std::string, DWORD>::value_type("none", SKILL_TYPE_NONE));
    m_SkillTypeIndexMap.insert(std::map<std::string, DWORD>::value_type("active", SKILL_TYPE_ACTIVE));
    m_SkillTypeIndexMap.insert(std::map<std::string, DWORD>::value_type("support", SKILL_TYPE_SUPPORT));
    m_SkillTypeIndexMap.insert(std::map<std::string, DWORD>::value_type("guild", SKILL_TYPE_GUILD));
    m_SkillTypeIndexMap.insert(std::map<std::string, DWORD>::value_type("horse", SKILL_TYPE_HORSE));
    m_SkillTypeIndexMap.insert(std::map<std::string, DWORD>::value_type("warrior", SKILL_TYPE_ACTIVE));
    m_SkillTypeIndexMap.insert(std::map<std::string, DWORD>::value_type("assassin", SKILL_TYPE_ACTIVE));
    m_SkillTypeIndexMap.insert(std::map<std::string, DWORD>::value_type("sura", SKILL_TYPE_ACTIVE));
    m_SkillTypeIndexMap.insert(std::map<std::string, DWORD>::value_type("shaman", SKILL_TYPE_ACTIVE));
    m_SkillTypeIndexMap.insert(std::map<std::string, DWORD>::value_type("passive", SKILL_TYPE_ACTIVE));

    m_SkillAttributeIndexMap.insert(
        std::map<std::string, DWORD>::value_type("NEED_POISON_BOTTLE", SKILL_ATTRIBUTE_NEED_POISON_BOTTLE));
    m_SkillAttributeIndexMap.insert(
        std::map<std::string, DWORD>::value_type("NEED_EMPTY_BOTTLE", SKILL_ATTRIBUTE_NEED_EMPTY_BOTTLE));
    m_SkillAttributeIndexMap.insert(
        std::map<std::string, DWORD>::value_type("CAN_USE_IF_NOT_ENOUGH", SKILL_ATTRIBUTE_CAN_USE_IF_NOT_ENOUGH));
    m_SkillAttributeIndexMap.insert(std::map<std::string, DWORD>::value_type("FAN_RANGE", SKILL_ATTRIBUTE_FAN_RANGE));
    m_SkillAttributeIndexMap.insert(
        std::map<std::string, DWORD>::value_type("NEED_CORPSE", SKILL_ATTRIBUTE_NEED_CORPSE));
    m_SkillAttributeIndexMap.insert(
        std::map<std::string, DWORD>::value_type("NEED_TARGET", SKILL_ATTRIBUTE_NEED_TARGET));
    m_SkillAttributeIndexMap.insert(std::map<std::string, DWORD>::value_type("TOGGLE", SKILL_ATTRIBUTE_TOGGLE));
    m_SkillAttributeIndexMap.insert(
        std::map<std::string, DWORD>::value_type("WEAPON_LIMITATION", SKILL_ATTRIBUTE_WEAPON_LIMITATION));
    m_SkillAttributeIndexMap.insert(
        std::map<std::string, DWORD>::value_type("MELEE_ATTACK", SKILL_ATTRIBUTE_MELEE_ATTACK));
    m_SkillAttributeIndexMap.insert(
        std::map<std::string, DWORD>::value_type("CHARGE_ATTACK", SKILL_ATTRIBUTE_CHARGE_ATTACK));
    m_SkillAttributeIndexMap.insert(std::map<std::string, DWORD>::value_type("USE_HP", SKILL_ATTRIBUTE_USE_HP));
    m_SkillAttributeIndexMap.insert(
        std::map<std::string, DWORD>::value_type("CAN_CHANGE_DIRECTION", SKILL_ATTRIBUTE_CAN_CHANGE_DIRECTION));
    m_SkillAttributeIndexMap.insert(
        std::map<std::string, DWORD>::value_type("STANDING_SKILL", SKILL_ATTRIBUTE_STANDING_SKILL));
    m_SkillAttributeIndexMap.insert(
        std::map<std::string, DWORD>::value_type("ONLY_FOR_ALLIANCE", SKILL_ATTRIBUTE_ONLY_FOR_ALLIANCE));
    m_SkillAttributeIndexMap.insert(
        std::map<std::string, DWORD>::value_type("CAN_USE_FOR_ME", SKILL_ATTRIBUTE_CAN_USE_FOR_ME));
    m_SkillAttributeIndexMap.insert(
        std::map<std::string, DWORD>::value_type("ATTACK_SKILL", SKILL_ATTRIBUTE_ATTACK_SKILL));
    m_SkillAttributeIndexMap.insert(
        std::map<std::string, DWORD>::value_type("MOVING_SKILL", SKILL_ATTRIBUTE_MOVING_SKILL));
    m_SkillAttributeIndexMap.insert(
        std::map<std::string, DWORD>::value_type("HORSE_SKILL", SKILL_ATTRIBUTE_HORSE_SKILL));
    m_SkillAttributeIndexMap.insert(
        std::map<std::string, DWORD>::value_type("TIME_INCREASE_SKILL", SKILL_ATTRIBUTE_TIME_INCREASE_SKILL));
    m_SkillAttributeIndexMap.insert(std::map<std::string, DWORD>::value_type("PASSIVE", SKILL_ATTRIBUTE_PASSIVE));
    m_SkillAttributeIndexMap.insert(
        std::map<std::string, DWORD>::value_type("CANNOT_LEVEL_UP", SKILL_ATTRIBUTE_CANNOT_LEVEL_UP));
    m_SkillAttributeIndexMap.insert(
        std::map<std::string, DWORD>::value_type("ONLY_FOR_GUILD_WAR", SKILL_ATTRIBUTE_ONLY_FOR_GUILD_WAR));
    m_SkillAttributeIndexMap.insert(
        std::map<std::string, DWORD>::value_type("CIRCLE_RANGE", SKILL_ATTRIBUTE_CIRCLE_RANGE));
    m_SkillAttributeIndexMap.insert(
        std::map<std::string, DWORD>::value_type("SEARCH_TARGET", SKILL_ATTRIBUTE_SEARCH_TARGET));

    m_SkillNeedWeaponIndexMap.insert(std::map<std::string, DWORD>::value_type("SWORD", SKILL_NEED_WEAPON_SWORD));
    m_SkillNeedWeaponIndexMap.insert(std::map<std::string, DWORD>::value_type("DAGGER", SKILL_NEED_WEAPON_DAGGER));
    m_SkillNeedWeaponIndexMap.insert(std::map<std::string, DWORD>::value_type("BOW", SKILL_NEED_WEAPON_BOW));
    m_SkillNeedWeaponIndexMap.insert(
        std::map<std::string, DWORD>::value_type("TWO_HANDED", SKILL_NEED_WEAPON_TWO_HANDED));
    m_SkillNeedWeaponIndexMap.insert(
        std::map<std::string, DWORD>::value_type("DOUBLE_SWORD", SKILL_NEED_WEAPON_DOUBLE_SWORD));
    m_SkillNeedWeaponIndexMap.insert(std::map<std::string, DWORD>::value_type("BELL", SKILL_NEED_WEAPON_BELL));
    m_SkillNeedWeaponIndexMap.insert(std::map<std::string, DWORD>::value_type("FAN", SKILL_NEED_WEAPON_FAN));
    m_SkillNeedWeaponIndexMap.insert(std::map<std::string, DWORD>::value_type("ARROW", SKILL_NEED_WEAPON_ARROW));
    m_SkillNeedWeaponIndexMap.insert(
        std::map<std::string, DWORD>::value_type("EMPTY_HAND", SKILL_NEED_WEAPON_EMPTY_HAND));

    m_SkillWeaponTypeIndexMap.insert(make_pair(std::string("SWORD"), CItemData::WEAPON_SWORD));
    m_SkillWeaponTypeIndexMap.insert(make_pair(std::string("DAGGER"), CItemData::WEAPON_DAGGER));
    m_SkillWeaponTypeIndexMap.insert(make_pair(std::string("BOW"), CItemData::WEAPON_BOW));
    m_SkillWeaponTypeIndexMap.insert(make_pair(std::string("TWO_HANDED"), CItemData::WEAPON_TWO_HANDED));
    m_SkillWeaponTypeIndexMap.insert(make_pair(std::string("DOUBLE_SWORD"), CItemData::WEAPON_DAGGER));
    m_SkillWeaponTypeIndexMap.insert(make_pair(std::string("BELL"), CItemData::WEAPON_BELL));
    m_SkillWeaponTypeIndexMap.insert(make_pair(std::string("FAN"), CItemData::WEAPON_FAN));
    m_SkillWeaponTypeIndexMap.insert(make_pair(std::string("ARROW"), CItemData::WEAPON_ARROW));

    SSkillData::ms_StatusNameMap.insert(make_pair(std::string("chain"), POINT_NONE));
    SSkillData::ms_StatusNameMap.insert(make_pair(std::string("HR"), POINT_HIT_RATE));
    SSkillData::ms_StatusNameMap.insert(make_pair(std::string("LV"), POINT_LEVEL));
    SSkillData::ms_StatusNameMap.insert(make_pair(std::string("Level"), POINT_LEVEL));
    SSkillData::ms_StatusNameMap.insert(make_pair(std::string("MaxHP"), POINT_MAX_HP));
    SSkillData::ms_StatusNameMap.insert(make_pair(std::string("MaxSP"), POINT_MAX_SP));
    SSkillData::ms_StatusNameMap.insert(make_pair(std::string("MinMWEP"), POINT_MIN_WEP));
    SSkillData::ms_StatusNameMap.insert(make_pair(std::string("MaxMWEP"), POINT_MAX_WEP));
    SSkillData::ms_StatusNameMap.insert(make_pair(std::string("MinWEP"), POINT_MIN_WEP));
    SSkillData::ms_StatusNameMap.insert(make_pair(std::string("MaxWEP"), POINT_MAX_WEP));
    SSkillData::ms_StatusNameMap.insert(make_pair(std::string("MinATK"), POINT_MIN_ATK));
    SSkillData::ms_StatusNameMap.insert(make_pair(std::string("MaxATK"), POINT_MAX_ATK));
    SSkillData::ms_StatusNameMap.insert(make_pair(std::string("ATKSPD"), POINT_ATT_SPEED));
    SSkillData::ms_StatusNameMap.insert(make_pair(std::string("AttackPower"), POINT_MIN_ATK));
    SSkillData::ms_StatusNameMap.insert(make_pair(std::string("AtkMin"), POINT_MIN_ATK));
    SSkillData::ms_StatusNameMap.insert(make_pair(std::string("AtkMax"), POINT_MAX_ATK));
    SSkillData::ms_StatusNameMap.insert(make_pair(std::string("DefencePower"), POINT_DEF_GRADE));
    SSkillData::ms_StatusNameMap.insert(make_pair(std::string("DEF"), POINT_DEF_GRADE));
    SSkillData::ms_StatusNameMap.insert(make_pair(std::string("MWEP"), POINT_MAGIC_ATT_GRADE));

    SSkillData::ms_StatusNameMap.insert(make_pair(std::string("MagicAttackPower"), POINT_MAGIC_ATT_GRADE));
    SSkillData::ms_StatusNameMap.insert(make_pair(std::string("INT"), POINT_IQ));
    SSkillData::ms_StatusNameMap.insert(make_pair(std::string("STR"), POINT_ST));
    SSkillData::ms_StatusNameMap.insert(make_pair(std::string("DEX"), POINT_DX));
    SSkillData::ms_StatusNameMap.insert(make_pair(std::string("CON"), POINT_HT));

    SSkillData::ms_StatusNameMap.insert(make_pair(std::string("minatk"), POINT_MIN_ATK));
    SSkillData::ms_StatusNameMap.insert(make_pair(std::string("maxatk"), POINT_MAX_ATK));
    SSkillData::ms_StatusNameMap.insert(make_pair(std::string("minmtk"), POINT_MIN_WEP));
    SSkillData::ms_StatusNameMap.insert(make_pair(std::string("maxmtk"), POINT_MAX_WEP));

    // GUILD_SKILL_DISPLAY_BUG_FIX
    SSkillData::ms_StatusNameMap.insert(make_pair(std::string("maxhp"), POINT_MAX_HP));
    SSkillData::ms_StatusNameMap.insert(make_pair(std::string("maxsp"), POINT_MAX_SP));
    SSkillData::ms_StatusNameMap.insert(make_pair(std::string("odef"), POINT_DEF_GRADE));
    // END_OF_GUILD_SKILL_DISPLAY_BUG_FIX

    SSkillData::ms_StatusNameMap.insert(make_pair(std::string("minwep"), POINT_MIN_WEP));
    SSkillData::ms_StatusNameMap.insert(make_pair(std::string("maxwep"), POINT_MAX_WEP));
    SSkillData::ms_StatusNameMap.insert(make_pair(std::string("minmwep"), POINT_MIN_MAGIC_WEP));
    SSkillData::ms_StatusNameMap.insert(make_pair(std::string("maxmwep"), POINT_MAX_MAGIC_WEP));
    SSkillData::ms_StatusNameMap.insert(make_pair(std::string("lv"), POINT_LEVEL));
    SSkillData::ms_StatusNameMap.insert(make_pair(std::string("ar"), POINT_HIT_RATE));
    SSkillData::ms_StatusNameMap.insert(make_pair(std::string("iq"), POINT_IQ));
    SSkillData::ms_StatusNameMap.insert(make_pair(std::string("str"), POINT_ST));
    SSkillData::ms_StatusNameMap.insert(make_pair(std::string("dex"), POINT_DX));
    SSkillData::ms_StatusNameMap.insert(make_pair(std::string("con"), POINT_HT));

    /////

    SSkillData::ms_NewMinStatusNameMap.insert(make_pair(std::string("atk"), POINT_MIN_ATK));
    SSkillData::ms_NewMinStatusNameMap.insert(make_pair(std::string("mtk"), POINT_MIN_WEP));
    SSkillData::ms_NewMinStatusNameMap.insert(make_pair(std::string("wep"), POINT_MIN_WEP));
    SSkillData::ms_NewMinStatusNameMap.insert(make_pair(std::string("lv"), POINT_LEVEL));
    SSkillData::ms_NewMinStatusNameMap.insert(make_pair(std::string("ar"), POINT_HIT_RATE));
    SSkillData::ms_NewMinStatusNameMap.insert(make_pair(std::string("iq"), POINT_IQ));
    SSkillData::ms_NewMinStatusNameMap.insert(make_pair(std::string("str"), POINT_ST));
    SSkillData::ms_NewMinStatusNameMap.insert(make_pair(std::string("dex"), POINT_DX));
    SSkillData::ms_NewMinStatusNameMap.insert(make_pair(std::string("con"), POINT_HT));

    SSkillData::ms_NewMaxStatusNameMap.insert(make_pair(std::string("atk"), POINT_MAX_ATK));
    SSkillData::ms_NewMaxStatusNameMap.insert(make_pair(std::string("mtk"), POINT_MAX_WEP));
    SSkillData::ms_NewMinStatusNameMap.insert(make_pair(std::string("wep"), POINT_MAX_WEP));
    SSkillData::ms_NewMaxStatusNameMap.insert(make_pair(std::string("lv"), POINT_LEVEL));
    SSkillData::ms_NewMaxStatusNameMap.insert(make_pair(std::string("ar"), POINT_HIT_RATE));
    SSkillData::ms_NewMaxStatusNameMap.insert(make_pair(std::string("iq"), POINT_IQ));
    SSkillData::ms_NewMaxStatusNameMap.insert(make_pair(std::string("str"), POINT_ST));
    SSkillData::ms_NewMaxStatusNameMap.insert(make_pair(std::string("dex"), POINT_DX));
    SSkillData::ms_NewMaxStatusNameMap.insert(make_pair(std::string("con"), POINT_HT));

    m_PathNameMap.insert(make_pair(std::string("WARRIOR"), std::string("warrior")));
    m_PathNameMap.insert(make_pair(std::string("ASSASSIN"), std::string("assassin")));
    m_PathNameMap.insert(make_pair(std::string("SURA"), std::string("sura")));
    m_PathNameMap.insert(make_pair(std::string("SHAMAN"), std::string("shaman")));
    m_PathNameMap.insert(make_pair(std::string("PASSIVE"), std::string("passive")));
    m_PathNameMap.insert(make_pair(std::string("SUPPORT"), std::string("common/support")));
    m_PathNameMap.insert(make_pair(std::string("GUILD"), std::string("common/guild")));
    m_PathNameMap.insert(make_pair(std::string("HORSE"), std::string("common/horse")));
}
CPythonSkill::~CPythonSkill() = default;

///////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////

DWORD CPythonSkill::SSkillData::MELEE_SKILL_TARGET_RANGE = 170;

std::string CPythonSkill::SSkillData::GetName() const
{
    return strName;
}

BYTE CPythonSkill::SSkillData::GetType() const
{
    return byType;
}

BOOL CPythonSkill::SSkillData::IsMeleeSkill() const
{
    if (dwSkillAttribute & SKILL_ATTRIBUTE_MELEE_ATTACK)
        return TRUE;

    return FALSE;
}

BOOL CPythonSkill::SSkillData::IsChargeSkill() const
{
    if (dwSkillAttribute & SKILL_ATTRIBUTE_CHARGE_ATTACK)
        return TRUE;

    return FALSE;
}

BOOL CPythonSkill::SSkillData::IsOnlyForGuildWar() const
{
    if (dwSkillAttribute & SKILL_ATTRIBUTE_ONLY_FOR_GUILD_WAR)
        return TRUE;

    return FALSE;
}

DWORD CPythonSkill::SSkillData::GetTargetRange() const
{
    if (dwSkillAttribute & SKILL_ATTRIBUTE_MELEE_ATTACK)
        return MELEE_SKILL_TARGET_RANGE;

    if (dwSkillAttribute & SKILL_ATTRIBUTE_CHARGE_ATTACK)
        return MELEE_SKILL_TARGET_RANGE;

    return dwTargetRange;
}

BOOL CPythonSkill::SSkillData::CanChangeDirection() const
{
    return 0 != (dwSkillAttribute & SKILL_ATTRIBUTE_CAN_CHANGE_DIRECTION);
}

BOOL CPythonSkill::SSkillData::IsFanRange() const
{
    return 0 != (dwSkillAttribute & SKILL_ATTRIBUTE_FAN_RANGE);
}

BOOL CPythonSkill::SSkillData::IsCircleRange() const
{
    return 0 != (dwSkillAttribute & SKILL_ATTRIBUTE_CIRCLE_RANGE);
}

BOOL CPythonSkill::SSkillData::IsAutoSearchTarget() const
{
    return 0 != (dwSkillAttribute & SKILL_ATTRIBUTE_SEARCH_TARGET);
}

BOOL CPythonSkill::SSkillData::IsNeedCorpse() const
{
    return 0 != (dwSkillAttribute & SKILL_ATTRIBUTE_NEED_CORPSE);
}

BOOL CPythonSkill::SSkillData::IsNeedTarget() const
{
    return 0 != (dwSkillAttribute & SKILL_ATTRIBUTE_NEED_TARGET);
}

BOOL CPythonSkill::SSkillData::IsToggleSkill() const
{
    return 0 != (dwSkillAttribute & SKILL_ATTRIBUTE_TOGGLE);
}

BOOL CPythonSkill::SSkillData::IsUseHPSkill() const
{
    return 0 != (dwSkillAttribute & SKILL_ATTRIBUTE_USE_HP);
}

BOOL CPythonSkill::SSkillData::IsStandingSkill() const
{
    return 0 != (dwSkillAttribute & SKILL_ATTRIBUTE_STANDING_SKILL);
}

BOOL CPythonSkill::SSkillData::CanUseWeaponType(DWORD dwWeaponType) const
{
    if (!(dwSkillAttribute & SKILL_ATTRIBUTE_WEAPON_LIMITATION))
        return TRUE;

    return 0 != (dwNeedWeapon & (1 << dwWeaponType));
}

BOOL CPythonSkill::SSkillData::IsOnlyForAlliance() const
{
    return 0 != (dwSkillAttribute & SKILL_ATTRIBUTE_ONLY_FOR_ALLIANCE);
}

BOOL CPythonSkill::SSkillData::CanUseForMe() const
{
    return 0 != (dwSkillAttribute & SKILL_ATTRIBUTE_CAN_USE_FOR_ME);
}

BOOL CPythonSkill::SSkillData::CanUseIfNotEnough() const
{
    return 0 != (dwSkillAttribute & SKILL_ATTRIBUTE_CAN_USE_IF_NOT_ENOUGH);
}

BOOL CPythonSkill::SSkillData::IsNeedEmptyBottle() const
{
    return 0 != (dwSkillAttribute & SKILL_ATTRIBUTE_NEED_EMPTY_BOTTLE);
}

BOOL CPythonSkill::SSkillData::IsNeedPoisonBottle() const
{
    return 0 != (dwSkillAttribute & SKILL_ATTRIBUTE_NEED_POISON_BOTTLE);
}

BOOL CPythonSkill::SSkillData::IsNeedBow() const
{
    if (!(dwSkillAttribute & SKILL_ATTRIBUTE_WEAPON_LIMITATION))
        return FALSE;

    return 0 != (dwNeedWeapon & SKILL_NEED_WEAPON_BOW);
}

BOOL CPythonSkill::SSkillData::IsHorseSkill() const
{
    return 0 != (dwSkillAttribute & SKILL_ATTRIBUTE_HORSE_SKILL);
}

BOOL CPythonSkill::SSkillData::IsMovingSkill() const
{
    return 0 != (dwSkillAttribute & SKILL_ATTRIBUTE_MOVING_SKILL);
}

BOOL CPythonSkill::SSkillData::IsAttackSkill() const
{
    return 0 != (dwSkillAttribute & SKILL_ATTRIBUTE_ATTACK_SKILL);
}

BOOL CPythonSkill::SSkillData::IsTimeIncreaseSkill() const
{
    return 0 != (dwSkillAttribute & SKILL_ATTRIBUTE_TIME_INCREASE_SKILL);
}

bool CPythonSkill::SSkillData::GetState(const char *c_szStateName, int *piState, int iMinMaxType)
{
    std::map<std::string, DWORD> *pStatusNameMap = nullptr;

    switch (iMinMaxType)
    {
    case VALUE_TYPE_FREE:
        pStatusNameMap = &ms_StatusNameMap;
        break;
    case VALUE_TYPE_MIN:
        pStatusNameMap = &ms_NewMinStatusNameMap;
        break;
    case VALUE_TYPE_MAX:
        pStatusNameMap = &ms_NewMaxStatusNameMap;
        break;
    default:
        return false;
        break;
    }

    auto it = pStatusNameMap->find(c_szStateName);

    if (it != pStatusNameMap->end())
    {
        *piState = CPythonPlayer::Instance().GetStatus(it->second);
    }
    else if (0 == strncmp(c_szStateName, "JeungJi", 7))
    {
        *piState = 0;

        // 증지술 임시 제외
        /*
                if (0 != ms_dwTimeIncreaseSkillNumber)
                {
                    DWORD dwSkillSlotIndex;
                    if
           (!CPythonPlayer::Instance().FindSkillSlotIndexBySkillIndex(ms_dwTimeIncreaseSkillNumber,
           &dwSkillSlotIndex)) return false;

                    *piState =
           CPythonPlayer::Instance().GetSkillLevel(dwSkillSlotIndex);
                }
        */
    }

    return true;
}

float CPythonSkill::SSkillData::ProcessFormula(CPoly *pPoly, float fSkillLevel, int iMinMaxType)
{
    if (pPoly->Analyze())
    {
        for (DWORD i = 0; i < pPoly->GetVarCount(); ++i)
        {
            const char *c_szVarName = pPoly->GetVarName(i);
            float fState = NAN;
            if (!strcmp("SkillPoint", c_szVarName) || !strcmp("k", c_szVarName))
            {
                fState = fSkillLevel;
            }
            else
            {
                int iState = 0;
                if (!GetState(c_szVarName, &iState, iMinMaxType))
                    return 0.0f;

                fState = static_cast<float>(iState);

                if (!strcmp("ar", c_szVarName))
                    fState /= 100.0f;
            }
            pPoly->SetVar(c_szVarName, fState);
        }
    }
    else
    {
        TraceError("skillGetAffect - Strange Formula [%s]", strName.c_str());
        return 0.0f;
    }

    return pPoly->Eval();
}

const char *CPythonSkill::SSkillData::GetAffectDescription(DWORD dwIndex, float fSkillLevel)
{
    if (dwIndex >= AffectDataVector.size())
        return nullptr;

    const std::string &c_rstrAffectDescription = AffectDataVector[dwIndex].strAffectDescription;
    const std::string &c_rstrAffectMinFormula = AffectDataVector[dwIndex].strAffectMinFormula;
    const std::string &c_rstrAffectMaxFormula = AffectDataVector[dwIndex].strAffectMaxFormula;

    CPoly minPoly;
    CPoly maxPoly;
    minPoly.SetStr(c_rstrAffectMinFormula);
    maxPoly.SetStr(c_rstrAffectMaxFormula);

    // OVERWRITE_SKILLPROTO_POLY
    float fMinValue = ProcessFormula(&minPoly, fSkillLevel);
    float fMaxValue = ProcessFormula(&maxPoly, fSkillLevel);

    if (fMinValue < 0.0)
        fMinValue = -fMinValue;
    if (fMaxValue < 0.0)
        fMaxValue = -fMaxValue;

    if (CP_ARABIC == ::GetDefaultCodePage())
    {
        // #0000870: [M2AE] 한국어 모드일때 특정 아랍어 문장에서 크래쉬 발생
        static std::string strDescription;
        strDescription = c_rstrAffectDescription;
        int const first = strDescription.find("%.0f");
        if (first >= 0)
        {
            fMinValue = floorf(fMinValue);

            char szMinValue[256];
            _snprintf(szMinValue, sizeof(szMinValue), "%.0f", fMinValue);
            strDescription.replace(first, 4, szMinValue);

            int const second = strDescription.find("%.0f", first);
            if (second >= 0)
            {
                fMaxValue = floorf(fMaxValue);

                char szMaxValue[256];
                _snprintf(szMaxValue, sizeof(szMaxValue), "%.0f", fMaxValue);
                strDescription.replace(second, 4, szMaxValue);
            }
        }
        return strDescription.c_str();
    }

    if (strstr(c_rstrAffectDescription.c_str(), "%.0f"))
    {
        fMinValue = floorf(fMinValue);
        fMaxValue = floorf(fMaxValue);
    }

    static char szDescription[64 + 1];
    _snprintf(szDescription, sizeof(szDescription), c_rstrAffectDescription.c_str(), fMinValue, fMaxValue);

    return szDescription;
}

DWORD CPythonSkill::SSkillData::GetSkillCoolTime(float fSkillPoint)
{
    if (strCoolTimeFormula.empty())
        return 0;

    CPoly poly;
    poly.SetStr(strCoolTimeFormula);

    return static_cast<DWORD>(ProcessFormula(&poly, fSkillPoint));
}

DWORD CPythonSkill::SSkillData::GetTargetCount(float fSkillPoint)
{
    if (strTargetCountFormula.empty())
        return 0;

    CPoly poly;
    poly.SetStr(strTargetCountFormula);

    return static_cast<DWORD>(ProcessFormula(&poly, fSkillPoint));
}

DWORD CPythonSkill::SSkillData::GetSkillMotionIndex(int iGrade)
{
    if (-1 != iGrade && SKILL_EFFECT_UPGRADE_ENABLE)
    {
        assert(iGrade >= 0 && iGrade < SKILL_EFFECT_COUNT);
        /*
        if (SKILL_GRADE_COUNT == iGrade)
        {
            return GradeData[SKILL_GRADE_COUNT-1].wMotionIndex;
        }
        else */
        if (iGrade >= 0 && iGrade < SKILL_EFFECT_COUNT)
        {
            return GradeData[iGrade].wMotionIndex;
        }
    }

    return wMotionIndex;
}

BYTE CPythonSkill::SSkillData::GetMaxLevel() const
{
    return byMaxLevel;
}

BYTE CPythonSkill::SSkillData::GetLevelUpPoint() const
{
    return byLevelUpPoint;
}

BOOL CPythonSkill::SSkillData::IsNoMotion() const
{
    return bNoMotion;
}

bool CPythonSkill::SSkillData::IsCanUseSkill() const
{
    return 0 == (dwSkillAttribute & SKILL_ATTRIBUTE_PASSIVE);
}

DWORD CPythonSkill::SSkillData::GetMotionLoopCount(float fSkillPoint)
{
    if (strMotionLoopCountFormula.empty())
        return 0;

    CPoly poly;
    poly.SetStr(strMotionLoopCountFormula);

    return static_cast<DWORD>(ProcessFormula(&poly, fSkillPoint));
}

int CPythonSkill::SSkillData::GetNeedSP(float fSkillPoint)
{
    if (strNeedSPFormula.empty())
        return 0;

    CPoly poly;
    poly.SetStr(strNeedSPFormula);

    return static_cast<int>(ProcessFormula(&poly, fSkillPoint));
}

DWORD CPythonSkill::SSkillData::GetContinuationSP(float fSkillPoint)
{
    if (strContinuationSPFormula.empty())
        return 0;

    CPoly poly;
    poly.SetStr(strContinuationSPFormula);

    return static_cast<DWORD>(ProcessFormula(&poly, fSkillPoint));
}

DWORD CPythonSkill::SSkillData::GetDuration(float fSkillPoint)
{
    if (strDuration.empty())
        return 0;

    CPoly poly;
    poly.SetStr(strDuration);

    return static_cast<DWORD>(ProcessFormula(&poly, fSkillPoint));
}

CPythonSkill::SSkillData::SSkillData()
    : byType(SKILL_TYPE_ACTIVE), bNoMotion(FALSE), byLevelLimit(0), byLevelUpPoint(1), byMaxLevel(20),
      byRequireSkillLevel(0), dwNeedWeapon(0), dwSkillAttribute(0), dwTargetRange(0), isRequirement(FALSE),
      pImage(nullptr), strContinuationSPFormula(""), strCoolTimeFormula(""), strDuration(""),
      strMotionLoopCountFormula(""), strName(""), strNeedSPFormula(""), strRequireSkillName("")
{

    for (int j = 0; j < SKILL_GRADE_COUNT; ++j)
    {
        TGradeData &rGradeData = GradeData[j];
        rGradeData.strName = "";
        rGradeData.pImage = nullptr;
        rGradeData.wMotionIndex = 0;
    }
}

///////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////

PyObject *skillSetPathName(PyObject *poSelf, PyObject *poArgs)
{
    char *szPathName = nullptr;
    if (!PyTuple_GetString(poArgs, 0, &szPathName))
        return Py_BadArgument();

    CPythonSkill::Instance().SetPathName(szPathName);
    return Py_BuildNone();
}

PyObject *skillRegisterSkill(PyObject *poSelf, PyObject *poArgs)
{
    int iSkillIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iSkillIndex))
        return Py_BadArgument();
    char *szFileName = nullptr;
    if (!PyTuple_GetString(poArgs, 1, &szFileName))
        return Py_BadArgument();

    std::string strFullFileName;
    strFullFileName = CPythonSkill::Instance().GetPathName();
    strFullFileName += szFileName;

    if (!CPythonSkill::Instance().RegisterSkill(iSkillIndex, strFullFileName.c_str()))
    {
        return Py_BuildException("skill.RegisterSkill - Failed to find skill data file : %d, %s", iSkillIndex,
                                 strFullFileName.c_str());
    }

    return Py_BuildNone();
}

PyObject *skillLoadSkillData(PyObject *poSelf, PyObject *poArgs)
{
    return Py_BuildNone();
}

PyObject *skillRegisterSkillDesc(PyObject *poSelf, PyObject *poArgs)
{
    char *szFileName = nullptr;
    if (!PyTuple_GetString(poArgs, 0, &szFileName))
        return Py_BadArgument();

    if (!CPythonSkill::Instance().RegisterSkillDesc(szFileName))
        return Py_BuildException("Failed to load Skill Desc");

    return Py_BuildNone();
}

PyObject *skillClearSkillData(PyObject *poSelf, PyObject *poArgs)
{
    CPythonSkill::SSkillData::ms_dwTimeIncreaseSkillNumber = 0;
    return Py_BuildNone();
}

PyObject *skillGetSkillName(PyObject *poSelf, PyObject *poArgs)
{
    int iSkillIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iSkillIndex))
        return Py_BadArgument();

    int iSkillGrade = -1;
    PyTuple_GetInteger(poArgs, 1, &iSkillGrade);

    CPythonSkill::SSkillData *c_pSkillData = nullptr;
    if (!CPythonSkill::Instance().GetSkillData(iSkillIndex, &c_pSkillData))
        return Py_BuildException("skill.GetSkillName - Failed to find skill by %d", iSkillIndex);

    if (-1 != iSkillGrade)
    {
        if (iSkillGrade >= 0 && iSkillGrade < CPythonSkill::SKILL_GRADE_COUNT)
        {
            return Py_BuildValue("s", c_pSkillData->GradeData[iSkillGrade].strName.c_str());
        }
    }

    return Py_BuildValue("s", c_pSkillData->strName.c_str());
}

PyObject *skillGetSkillDescription(PyObject *poSelf, PyObject *poArgs)
{
    int iSkillIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iSkillIndex))
        return Py_BadArgument();

    CPythonSkill::SSkillData *c_pSkillData = nullptr;
    if (!CPythonSkill::Instance().GetSkillData(iSkillIndex, &c_pSkillData))
        return Py_BuildException("skill.GetSkillDescription - Failed to find skill by %d", iSkillIndex);

    return Py_BuildValue("s", c_pSkillData->strDescription.c_str());
}

PyObject *skillGetSkillType(PyObject *poSelf, PyObject *poArgs)
{
    int iSkillIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iSkillIndex))
        return Py_BadArgument();

    CPythonSkill::SSkillData *c_pSkillData = nullptr;
    if (!CPythonSkill::Instance().GetSkillData(iSkillIndex, &c_pSkillData))
        return Py_BuildException("skill.GetSkillType - Failed to find skill by %d", iSkillIndex);

    return Py_BuildValue("i", c_pSkillData->byType);
}

PyObject *skillGetSkillConditionDescriptionCount(PyObject *poSelf, PyObject *poArgs)
{
    int iSkillIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iSkillIndex))
        return Py_BadArgument();

    CPythonSkill::SSkillData *c_pSkillData = nullptr;
    if (!CPythonSkill::Instance().GetSkillData(iSkillIndex, &c_pSkillData))
        return Py_BuildException("skill.GetSkillConditionDescriptionCount - Failed to find skill by %d", iSkillIndex);

    return Py_BuildValue("i", c_pSkillData->ConditionDataVector.size());
}

PyObject *skillGetSkillConditionDescription(PyObject *poSelf, PyObject *poArgs)
{
    int iSkillIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iSkillIndex))
        return Py_BadArgument();

    int iConditionIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 1, &iConditionIndex))
        return Py_BadArgument();

    CPythonSkill::SSkillData *c_pSkillData = nullptr;
    if (!CPythonSkill::Instance().GetSkillData(iSkillIndex, &c_pSkillData))
        return Py_BuildException("skill.GetSkillConditionDescription() - Failed to find skill by %d", iSkillIndex);

    if (iConditionIndex >= c_pSkillData->ConditionDataVector.size())
        return Py_BuildValue("None");

    return Py_BuildValue("s", c_pSkillData->ConditionDataVector[iConditionIndex].c_str());
}

PyObject *skillGetSkillAffectDescriptionCount(PyObject *poSelf, PyObject *poArgs)
{
    int iSkillIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iSkillIndex))
        return Py_BadArgument();

    CPythonSkill::SSkillData *c_pSkillData = nullptr;
    if (!CPythonSkill::Instance().GetSkillData(iSkillIndex, &c_pSkillData))
        return Py_BuildException("skill.GetSkillAffectDescriptionCount - Failed to find skill by %d", iSkillIndex);

    return Py_BuildValue("i", c_pSkillData->AffectDataVector.size());
}

PyObject *skillGetSkillAffectDescription(PyObject *poSelf, PyObject *poArgs)
{
    int iSkillIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iSkillIndex))
        return Py_BadArgument();

    int iAffectIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 1, &iAffectIndex))
        return Py_BadArgument();

    float fSkillPoint = NAN;
    if (!PyTuple_GetFloat(poArgs, 2, &fSkillPoint))
        return Py_BadArgument();

    CPythonSkill::SSkillData *c_pSkillData = nullptr;
    if (!CPythonSkill::Instance().GetSkillData(iSkillIndex, &c_pSkillData))
        return Py_BuildException("skill.GetSkillAffectDescription - Failed to find skill by %d", iSkillIndex);

    return Py_BuildValue("s", c_pSkillData->GetAffectDescription(iAffectIndex, fSkillPoint));
}

PyObject *skillGetSkillCoolTime(PyObject *poSelf, PyObject *poArgs)
{
    int iSkillIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iSkillIndex))
        return Py_BadArgument();

    float fSkillPoint = NAN;
    if (!PyTuple_GetFloat(poArgs, 1, &fSkillPoint))
        return Py_BadArgument();

    CPythonSkill::SSkillData *c_pSkillData = nullptr;
    if (!CPythonSkill::Instance().GetSkillData(iSkillIndex, &c_pSkillData))
        return Py_BuildException("skill.GetSkillCoolTime - Failed to find skill by %d", iSkillIndex);

    return Py_BuildValue("i", c_pSkillData->GetSkillCoolTime(fSkillPoint));
}

PyObject *skillGetSkillNeedSP(PyObject *poSelf, PyObject *poArgs)
{
    int iSkillIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iSkillIndex))
        return Py_BadArgument();

    float fSkillPoint = NAN;
    if (!PyTuple_GetFloat(poArgs, 1, &fSkillPoint))
        return Py_BadArgument();

    CPythonSkill::SSkillData *c_pSkillData = nullptr;
    if (!CPythonSkill::Instance().GetSkillData(iSkillIndex, &c_pSkillData))
        return Py_BuildException("skill.GetSkillNeedSP Failed to find skill by %d", iSkillIndex);

    return Py_BuildValue("i", c_pSkillData->GetNeedSP(fSkillPoint));
}

PyObject *skillGetSkillContinuationSP(PyObject *poSelf, PyObject *poArgs)
{
    int iSkillIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iSkillIndex))
        return Py_BadArgument();

    float fSkillPoint = NAN;
    if (!PyTuple_GetFloat(poArgs, 1, &fSkillPoint))
        return Py_BadArgument();

    CPythonSkill::SSkillData *c_pSkillData = nullptr;
    if (!CPythonSkill::Instance().GetSkillData(iSkillIndex, &c_pSkillData))
        return Py_BuildException("skill.GetSkillContinuationSP - Failed to find skill by %d", iSkillIndex);

    return Py_BuildValue("i", c_pSkillData->GetContinuationSP(fSkillPoint));
}

PyObject *skillGetSkillMaxLevel(PyObject *poSelf, PyObject *poArgs)
{
    int iSkillIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iSkillIndex))
        return Py_BadArgument();

    CPythonSkill::SSkillData *c_pSkillData = nullptr;
    if (!CPythonSkill::Instance().GetSkillData(iSkillIndex, &c_pSkillData))
        return Py_BuildException("skill.GetSkillMaxLevel - Failed to find skill by %d", iSkillIndex);

    return Py_BuildValue("i", c_pSkillData->GetMaxLevel());
}

PyObject *skillGetSkillLevelUpPoint(PyObject *poSelf, PyObject *poArgs)
{
    int iSkillIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iSkillIndex))
        return Py_BadArgument();

    CPythonSkill::SSkillData *c_pSkillData = nullptr;
    if (!CPythonSkill::Instance().GetSkillData(iSkillIndex, &c_pSkillData))
        return Py_BuildException("skill.GetSkillLevelUpPoint - Failed to find skill by %d", iSkillIndex);

    return Py_BuildValue("i", c_pSkillData->GetLevelUpPoint());
}

PyObject *skillGetSkillLevelLimit(PyObject *poSelf, PyObject *poArgs)
{
    int iSkillIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iSkillIndex))
        return Py_BadArgument();

    CPythonSkill::SSkillData *c_pSkillData = nullptr;
    if (!CPythonSkill::Instance().GetSkillData(iSkillIndex, &c_pSkillData))
        return Py_BuildException("skill.GetSkillLevelLimit - Failed to find skill by %d", iSkillIndex);

    return Py_BuildValue("i", c_pSkillData->byLevelLimit);
}

PyObject *skillIsSkillRequirement(PyObject *poSelf, PyObject *poArgs)
{
    int iSkillIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iSkillIndex))
        return Py_BadArgument();

    CPythonSkill::SSkillData *c_pSkillData = nullptr;
    if (!CPythonSkill::Instance().GetSkillData(iSkillIndex, &c_pSkillData))
        return Py_BuildException("skill.IsSkillRequirement - Failed to find skill by %d", iSkillIndex);

    if (c_pSkillData->isRequirement)
    {
        CPythonSkill::SSkillData *pRequireSkillData = nullptr;
        if (!CPythonSkill::Instance().GetSkillDataByName(c_pSkillData->strRequireSkillName.c_str(), &pRequireSkillData))
        {
            TraceError("skill.IsSkillRequirement - Failed to find skill : "
                       "[%d/%s] can't find [%s]\n",
                       c_pSkillData->dwSkillIndex, c_pSkillData->strName.c_str(),
                       c_pSkillData->strRequireSkillName.c_str());
            return Py_BuildValue("i", FALSE);
        }

        DWORD dwRequireSkillSlotIndex = 0;
        if (!CPythonPlayer::Instance().FindSkillSlotIndexBySkillIndex(pRequireSkillData->dwSkillIndex,
                                                                      &dwRequireSkillSlotIndex))
            return Py_BuildValue("i", FALSE);
    }

    return Py_BuildValue("i", c_pSkillData->isRequirement);
}

PyObject *skillGetSkillRequirementData(PyObject *poSelf, PyObject *poArgs)
{
    int iSkillIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iSkillIndex))
        return Py_BadArgument();

    CPythonSkill::SSkillData *c_pSkillData = nullptr;
    if (!CPythonSkill::Instance().GetSkillData(iSkillIndex, &c_pSkillData))
        return Py_BuildException("skill.GetSkillRequirementData - Failed to find skill by %d", iSkillIndex);

    CPythonSkill::SSkillData *pRequireSkillData = nullptr;
    if (!CPythonSkill::Instance().GetSkillDataByName(c_pSkillData->strRequireSkillName.c_str(), &pRequireSkillData))
        return Py_BuildValue("si", 0, "None", 0);

    int const ireqLevel = static_cast<int>(ceil(static_cast<float>(c_pSkillData->byRequireSkillLevel) /
                                                static_cast<float>(max(1, pRequireSkillData->byLevelUpPoint))));
    return Py_BuildValue("si", c_pSkillData->strRequireSkillName.c_str(), ireqLevel);
}

PyObject *skillGetSkillRequireStatCount(PyObject *poSelf, PyObject *poArgs)
{
    int iSkillIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iSkillIndex))
        return Py_BadArgument();

    CPythonSkill::SSkillData *c_pSkillData = nullptr;
    if (!CPythonSkill::Instance().GetSkillData(iSkillIndex, &c_pSkillData))
        return Py_BuildException("skill.GetSkillRequireStatCount - Failed to find skill by %d", iSkillIndex);

    return Py_BuildValue("i", c_pSkillData->RequireStatDataVector.size());
}

PyObject *skillGetSkillRequireStatData(PyObject *poSelf, PyObject *poArgs)
{
    int iSkillIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iSkillIndex))
        return Py_BadArgument();
    int iStatIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 1, &iStatIndex))
        return Py_BadArgument();

    CPythonSkill::SSkillData *c_pSkillData = nullptr;
    if (!CPythonSkill::Instance().GetSkillData(iSkillIndex, &c_pSkillData))
        return Py_BuildException("skill.GetSkillRequireStatData - Failed to find skill by %d", iSkillIndex);

    if (iStatIndex >= c_pSkillData->RequireStatDataVector.size())
        return Py_BuildValue("ii", 0, 0);

    const CPythonSkill::TRequireStatData &c_rRequireStatData = c_pSkillData->RequireStatDataVector[iStatIndex];

    return Py_BuildValue("ii", c_rRequireStatData.byPoint, c_rRequireStatData.byLevel);
}

PyObject *skillCanLevelUpSkill(PyObject *poSelf, PyObject *poArgs)
{
    int iSkillIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iSkillIndex))
        return Py_BadArgument();
    int iSkillLevel = 0;
    if (!PyTuple_GetInteger(poArgs, 1, &iSkillLevel))
        return Py_BadArgument();

    CPythonSkill::SSkillData *c_pSkillData = nullptr;
    if (!CPythonSkill::Instance().GetSkillData(iSkillIndex, &c_pSkillData))
        return Py_BuildException("skill.CanLevelUpSkill - Failed to find skill by %d", iSkillIndex);

    if (iSkillLevel >= c_pSkillData->GetMaxLevel())
        return Py_BuildValue("i", FALSE);

    if (c_pSkillData->isRequirement)
    {
        CPythonSkill::SSkillData *pRequireSkillData = nullptr;
        if (CPythonSkill::Instance().GetSkillDataByName(c_pSkillData->strRequireSkillName.c_str(), &pRequireSkillData))
        {
            DWORD dwRequireSkillSlotIndex = 0;
            if (CPythonPlayer::Instance().FindSkillSlotIndexBySkillIndex(pRequireSkillData->dwSkillIndex,
                                                                         &dwRequireSkillSlotIndex))
            {
                int const iSkillGrade = CPythonPlayer::Instance().GetSkillGrade(dwRequireSkillSlotIndex);
                int const iSkillLevel = CPythonPlayer::Instance().GetSkillLevel(dwRequireSkillSlotIndex);
                if (iSkillGrade <= 0)
                {
                    if (iSkillLevel < c_pSkillData->byRequireSkillLevel)
                        return Py_BuildValue("i", FALSE);
                }
            }
        }
    }

    for (auto c_rRequireStatData : c_pSkillData->RequireStatDataVector)
    {
        if (CPythonPlayer::Instance().GetStatus(c_rRequireStatData.byPoint) < c_rRequireStatData.byLevel)
            return Py_BuildValue("i", FALSE);
    }

    if (0 != (c_pSkillData->dwSkillAttribute & CPythonSkill::SKILL_ATTRIBUTE_CANNOT_LEVEL_UP))
        return Py_BuildValue("i", FALSE);

    return Py_BuildValue("i", TRUE);
}

PyObject *skillCheckRequirementSueccess(PyObject *poSelf, PyObject *poArgs)
{
    int iSkillIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iSkillIndex))
        return Py_BadArgument();

    CPythonSkill::SSkillData *c_pSkillData = nullptr;
    if (!CPythonSkill::Instance().GetSkillData(iSkillIndex, &c_pSkillData))
        return Py_BuildException("skill.CheckRequirementSueccess - Failed to find skill by %d", iSkillIndex);

    if (c_pSkillData->isRequirement)
    {
        CPythonSkill::SSkillData *pRequireSkillData = nullptr;
        if (CPythonSkill::Instance().GetSkillDataByName(c_pSkillData->strRequireSkillName.c_str(), &pRequireSkillData))
        {
            DWORD dwRequireSkillSlotIndex = 0;
            if (CPythonPlayer::Instance().FindSkillSlotIndexBySkillIndex(pRequireSkillData->dwSkillIndex,
                                                                         &dwRequireSkillSlotIndex))
            {
                int const iSkillGrade = CPythonPlayer::Instance().GetSkillGrade(dwRequireSkillSlotIndex);
                int const iSkillLevel = CPythonPlayer::Instance().GetSkillLevel(dwRequireSkillSlotIndex);
                if (iSkillGrade <= 0)
                {
                    if (iSkillLevel < c_pSkillData->byRequireSkillLevel)
                        return Py_BuildValue("i", FALSE);
                }
            }
        }
    }

    return Py_BuildValue("i", TRUE);
}

PyObject *skillGetNeedCharacterLevel(PyObject *poSelf, PyObject *poArgs)
{
    int iSkillIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iSkillIndex))
        return Py_BadArgument();

    CPythonSkill::SSkillData *c_pSkillData = nullptr;
    if (!CPythonSkill::Instance().GetSkillData(iSkillIndex, &c_pSkillData))
        return Py_BuildException("skill.GetNeedCharacterLevel - Failed to find skill by %d", iSkillIndex);

    auto itor = c_pSkillData->RequireStatDataVector.begin();
    for (; itor != c_pSkillData->RequireStatDataVector.end(); ++itor)
    {
        const CPythonSkill::TRequireStatData &c_rRequireStatData = *itor;

        if (POINT_LEVEL == c_rRequireStatData.byPoint)
            return Py_BuildValue("i", c_rRequireStatData.byLevel);
    }

    return Py_BuildValue("i", 0);
}

PyObject *skillIsToggleSkill(PyObject *poSelf, PyObject *poArgs)
{
    int iSkillIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iSkillIndex))
        return Py_BadArgument();

    CPythonSkill::SSkillData *c_pSkillData = nullptr;
    if (!CPythonSkill::Instance().GetSkillData(iSkillIndex, &c_pSkillData))
        return Py_BuildException("skill.IsToggleSkill - Failed to find skill by %d", iSkillIndex);

    return Py_BuildValue("i", c_pSkillData->IsToggleSkill());
}

PyObject *skillIsUseHPSkill(PyObject *poSelf, PyObject *poArgs)
{
    int iSkillIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iSkillIndex))
        return Py_BadArgument();

    CPythonSkill::SSkillData *c_pSkillData = nullptr;
    if (!CPythonSkill::Instance().GetSkillData(iSkillIndex, &c_pSkillData))
        return Py_BuildException("skill.IsUseHPSkill - Failed to find skill by %d", iSkillIndex);

    return Py_BuildValue("i", c_pSkillData->IsUseHPSkill());
}

PyObject *skillIsStandingSkill(PyObject *poSelf, PyObject *poArgs)
{
    int iSkillIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iSkillIndex))
        return Py_BadArgument();

    CPythonSkill::SSkillData *c_pSkillData = nullptr;
    if (!CPythonSkill::Instance().GetSkillData(iSkillIndex, &c_pSkillData))
        return Py_BuildException("skill.IsStandingSkill - Failed to find skill by %d", iSkillIndex);

    return Py_BuildValue("i", c_pSkillData->IsStandingSkill());
}

PyObject *skillCanUseSkill(PyObject *poSelf, PyObject *poArgs)
{
    int iSkillIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iSkillIndex))
        return Py_BadArgument();

    CPythonSkill::SSkillData *c_pSkillData = nullptr;
    if (!CPythonSkill::Instance().GetSkillData(iSkillIndex, &c_pSkillData))
        return Py_BuildException("skill.CanUseSkill - Failed to find skill by %d", iSkillIndex);

    return Py_BuildValue("i", c_pSkillData->IsCanUseSkill());
}

PyObject *skillIsLevelUpSkill(PyObject *poSelf, PyObject *poArgs)
{
    int iSkillIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iSkillIndex))
        return Py_BadArgument();

    CPythonSkill::SSkillData *c_pSkillData = nullptr;
    if (!CPythonSkill::Instance().GetSkillData(iSkillIndex, &c_pSkillData))
        return Py_BuildException("skill.IsLevelUpSkill - Failed to find skill by %d", iSkillIndex);

    // 모두 레벨업 가능

    return Py_BuildValue("i", TRUE);
}

PyObject *skillGetIconName(PyObject *poSelf, PyObject *poArgs)
{
    int iSkillIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iSkillIndex))
        return Py_BadArgument();

    CPythonSkill::SSkillData *c_pSkillData = nullptr;
    if (!CPythonSkill::Instance().GetSkillData(iSkillIndex, &c_pSkillData))
        return Py_BuildValue("s", "");

    return Py_BuildValue("s", c_pSkillData->strIconFileName.c_str());
}

PyObject *skillGetIconImage(PyObject *poSelf, PyObject *poArgs)
{
    int iSkillIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iSkillIndex))
        return Py_BadArgument();

    CPythonSkill::SSkillData *c_pSkillData = nullptr;
    if (!CPythonSkill::Instance().GetSkillData(iSkillIndex, &c_pSkillData))
        return Py_BuildValue("l", static_cast<intptr_t>(0)); // 익셉션을 내는 대신 0을 리턴한다.

    return Py_BuildValue("l", (intptr_t)c_pSkillData->pImage);
}

PyObject *skillGetIconInstance(PyObject *poSelf, PyObject *poArgs)
{
    int iSkillIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iSkillIndex))
        return Py_BadArgument();

    CPythonSkill::SSkillData *c_pSkillData = nullptr;
    if (!CPythonSkill::Instance().GetSkillData(iSkillIndex, &c_pSkillData))
        return Py_BuildException("skill.GetIconInstance - Failed to find skill by %d", iSkillIndex);

    CGraphicImageInstance *pImageInstance = CGraphicImageInstance::New();
    pImageInstance->SetImagePointer(c_pSkillData->pImage);

    return Py_BuildValue("l", (intptr_t)pImageInstance);
}

PyObject *skillGetIconImageNew(PyObject *poSelf, PyObject *poArgs)
{
    int iSkillIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iSkillIndex))
        return Py_BadArgument();

    int iGradeIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 1, &iGradeIndex))
        return Py_BadArgument();

    CPythonSkill::SSkillData *c_pSkillData = nullptr;
    if (!CPythonSkill::Instance().GetSkillData(iSkillIndex, &c_pSkillData))
        return Py_BuildValue("l", static_cast<intptr_t>(0)); // 익셉션을 내는 대신 0을 리턴한다.

    iGradeIndex = std::max(iGradeIndex, 0);

    if (iGradeIndex >= CPythonSkill::SKILL_GRADE_COUNT)
        iGradeIndex = CPythonSkill::SKILL_GRADE_COUNT - 1;

    return Py_BuildValue("l", (intptr_t)c_pSkillData->GradeData[iGradeIndex].pImage);
}

PyObject *skillGetIconInstanceNew(PyObject *poSelf, PyObject *poArgs)
{
    int iSkillIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iSkillIndex))
        return Py_BadArgument();

    int iGradeIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 1, &iGradeIndex))
        return Py_BadArgument();

    CPythonSkill::SSkillData *c_pSkillData = nullptr;
    if (!CPythonSkill::Instance().GetSkillData(iSkillIndex, &c_pSkillData))
        return Py_BuildException("skill.GetIconInstanceNew - Failed to find skill by %d", iSkillIndex);

    // CGraphicImage * pImage = c_pSkillData->pImage;

    iGradeIndex = std::max(iGradeIndex, 0);

    if (iGradeIndex >= CPythonSkill::SKILL_GRADE_COUNT)
        iGradeIndex = CPythonSkill::SKILL_GRADE_COUNT - 1;

    CGraphicImageInstance *pImageInstance = CGraphicImageInstance::New();
    pImageInstance->SetImagePointer(c_pSkillData->GradeData[iGradeIndex].pImage);

    return Py_BuildValue("l", (intptr_t)pImageInstance);
}

PyObject *skillDeleteIconInstance(PyObject *poSelf, PyObject *poArgs)
{
    long iHandle = 0;
    if (!PyTuple_GetLong(poArgs, 0, &iHandle))
        return Py_BadArgument();

    CGraphicImageInstance::Delete((CGraphicImageInstance *)iHandle);
    return Py_BuildNone();
}

PyObject *skillGetGradeData(PyObject *poSelf, PyObject *poArgs)
{
    int iSkillIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iSkillIndex))
        return Py_BadArgument();

    int iGradeIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 1, &iGradeIndex))
        return Py_BadArgument();

    CPythonSkill::SSkillData *c_pSkillData = nullptr;
    if (!CPythonSkill::Instance().GetSkillData(iSkillIndex, &c_pSkillData))
        return Py_BuildException("skill.GetGradeData - Failed to find skill by %d", iSkillIndex);

    if (iGradeIndex < 0 || iGradeIndex >= CPythonSkill::SKILL_GRADE_COUNT)
        return Py_BuildException("Strange grade index [%d]", iSkillIndex, iGradeIndex);

    return Py_BuildValue("i", c_pSkillData->GradeData[iGradeIndex].wMotionIndex);
}

PyObject *skillGetNewAffectDataCount(PyObject *poSelf, PyObject *poArgs)
{
    int iSkillIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iSkillIndex))
        return Py_BadArgument();

    CPythonSkill::SSkillData *c_pSkillData = nullptr;
    if (!CPythonSkill::Instance().GetSkillData(iSkillIndex, &c_pSkillData))
        return Py_BuildException("skill.GetNewAffectDataCount - Failed to find skill by %d", iSkillIndex);

    return Py_BuildValue("i", c_pSkillData->AffectDataNewVector.size());
}

PyObject *skillGetNewAffectData(PyObject *poSelf, PyObject *poArgs)
{
    int iSkillIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iSkillIndex))
        return Py_BadArgument();
    int iAffectIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 1, &iAffectIndex))
        return Py_BadArgument();
    float fSkillLevel = NAN;
    if (!PyTuple_GetFloat(poArgs, 2, &fSkillLevel))
        return Py_BadArgument();

    CPythonSkill::SSkillData *pSkillData = nullptr;
    if (!CPythonSkill::Instance().GetSkillData(iSkillIndex, &pSkillData))
        return Py_BuildException("skill.GetNewAffectData - Failed to find skill by %d", iSkillIndex);

    if (iAffectIndex < 0 || iAffectIndex >= pSkillData->AffectDataNewVector.size())
        return Py_BuildException(" skill.GetNewAffectData - Strange AffectIndex %d", iAffectIndex);

    CPythonSkill::TAffectDataNew const &rAffectData = pSkillData->AffectDataNewVector[iAffectIndex];

    CPoly minPoly;
    CPoly maxPoly;
    minPoly.SetRandom(CPoly::RANDOM_TYPE_FORCE_MIN);
    maxPoly.SetRandom(CPoly::RANDOM_TYPE_FORCE_MAX);
    minPoly.SetStr(rAffectData.strPointPoly);
    maxPoly.SetStr(rAffectData.strPointPoly);
    float const fMinValue = pSkillData->ProcessFormula(&minPoly, fSkillLevel, CPythonSkill::VALUE_TYPE_MIN);
    float const fMaxValue = pSkillData->ProcessFormula(&maxPoly, fSkillLevel, CPythonSkill::VALUE_TYPE_MAX);

    return Py_BuildValue("sff", rAffectData.strPointType.c_str(), fMinValue, fMaxValue);
}

PyObject *skillGetDuration(PyObject *poSelf, PyObject *poArgs)
{
    int iSkillIndex = 0;
    if (!PyTuple_GetInteger(poArgs, 0, &iSkillIndex))
        return Py_BadArgument();
    float fSkillLevel = NAN;
    if (!PyTuple_GetFloat(poArgs, 1, &fSkillLevel))
        return Py_BadArgument();

    CPythonSkill::SSkillData *c_pSkillData = nullptr;
    if (!CPythonSkill::Instance().GetSkillData(iSkillIndex, &c_pSkillData))
        return Py_BuildException("skill.GetDuration - Failed to find skill by %d", iSkillIndex);

    return Py_BuildValue("i", c_pSkillData->GetDuration(fSkillLevel));
}

PyObject *skillTEST(PyObject *poSelf, PyObject *poArgs)
{
    CPythonSkill::Instance().TEST();
    return Py_BuildNone();
}

void initskill()
{
    static PyMethodDef s_methods[] = {
        {"SetPathName", skillSetPathName, METH_VARARGS},
        {"RegisterSkill", skillRegisterSkill, METH_VARARGS},
        {"LoadSkillData", skillLoadSkillData, METH_VARARGS},
        {"ClearSkillData", skillClearSkillData, METH_VARARGS},

        /////

        {"GetSkillName", skillGetSkillName, METH_VARARGS},
        {"GetSkillDescription", skillGetSkillDescription, METH_VARARGS},
        {"GetSkillType", skillGetSkillType, METH_VARARGS},
        {"GetSkillConditionDescriptionCount", skillGetSkillConditionDescriptionCount, METH_VARARGS},
        {"GetSkillConditionDescription", skillGetSkillConditionDescription, METH_VARARGS},
        {"GetSkillAffectDescriptionCount", skillGetSkillAffectDescriptionCount, METH_VARARGS},
        {"GetSkillAffectDescription", skillGetSkillAffectDescription, METH_VARARGS},
        {"GetSkillCoolTime", skillGetSkillCoolTime, METH_VARARGS},
        {"GetSkillNeedSP", skillGetSkillNeedSP, METH_VARARGS},
        {"GetSkillContinuationSP", skillGetSkillContinuationSP, METH_VARARGS},
        {"GetSkillMaxLevel", skillGetSkillMaxLevel, METH_VARARGS},
        {"GetSkillLevelUpPoint", skillGetSkillLevelUpPoint, METH_VARARGS},
        {"GetSkillLevelLimit", skillGetSkillLevelLimit, METH_VARARGS},
        {"IsSkillRequirement", skillIsSkillRequirement, METH_VARARGS},
        {"GetSkillRequirementData", skillGetSkillRequirementData, METH_VARARGS},
        {"GetSkillRequireStatCount", skillGetSkillRequireStatCount, METH_VARARGS},
        {"GetSkillRequireStatData", skillGetSkillRequireStatData, METH_VARARGS},
        {"CanLevelUpSkill", skillCanLevelUpSkill, METH_VARARGS},
        {"IsLevelUpSkill", skillIsLevelUpSkill, METH_VARARGS},
        {"CheckRequirementSueccess", skillCheckRequirementSueccess, METH_VARARGS},
        {"GetNeedCharacterLevel", skillGetNeedCharacterLevel, METH_VARARGS},
        {"IsToggleSkill", skillIsToggleSkill, METH_VARARGS},
        {"IsUseHPSkill", skillIsUseHPSkill, METH_VARARGS},
        {"IsStandingSkill", skillIsStandingSkill, METH_VARARGS},
        {"CanUseSkill", skillCanUseSkill, METH_VARARGS},
        {"GetIconName", skillGetIconName, METH_VARARGS},
        {"GetIconImage", skillGetIconImage, METH_VARARGS},
        {"GetIconImageNew", skillGetIconImageNew, METH_VARARGS},
        {"GetIconInstance", skillGetIconInstance, METH_VARARGS},
        {"GetIconInstanceNew", skillGetIconInstanceNew, METH_VARARGS},
        {"DeleteIconInstance", skillDeleteIconInstance, METH_VARARGS},
        {"GetGradeData", skillGetGradeData, METH_VARARGS},

        {"GetNewAffectDataCount", skillGetNewAffectDataCount, METH_VARARGS},
        {"GetNewAffectData", skillGetNewAffectData, METH_VARARGS},
        {"GetDuration", skillGetDuration, METH_VARARGS},

        {"TEST", skillTEST, METH_VARARGS},

        {nullptr, nullptr, NULL},
    };

    PyObject *poModule = Py_InitModule("skill", s_methods);
    PyModule_AddIntConstant(poModule, "SKILL_TYPE_NONE", CPythonSkill::SKILL_TYPE_NONE);
    PyModule_AddIntConstant(poModule, "SKILL_TYPE_ACTIVE", CPythonSkill::SKILL_TYPE_ACTIVE);
    PyModule_AddIntConstant(poModule, "SKILL_TYPE_SUPPORT", CPythonSkill::SKILL_TYPE_SUPPORT);
    PyModule_AddIntConstant(poModule, "SKILL_TYPE_GUILD", CPythonSkill::SKILL_TYPE_GUILD);
    PyModule_AddIntConstant(poModule, "SKILL_TYPE_HORSE", CPythonSkill::SKILL_TYPE_HORSE);
    PyModule_AddIntConstant(poModule, "SKILL_TYPE_MAX_NUM", CPythonSkill::SKILL_TYPE_MAX_NUM);

    PyModule_AddIntConstant(poModule, "SKILL_GRADE_COUNT", CPythonSkill::SKILL_GRADE_COUNT);
    PyModule_AddIntConstant(poModule, "SKILL_GRADE_STEP_COUNT", CPythonSkill::SKILL_GRADE_STEP_COUNT);
    PyModule_AddIntConstant(poModule, "SKILL_GRADEGAP", CPythonSkill::SKILL_GRADEGAP);
    PyModule_AddIntConstant(poModule, "SKILL_EFFECT_COUNT", CPythonSkill::SKILL_EFFECT_COUNT);
}
