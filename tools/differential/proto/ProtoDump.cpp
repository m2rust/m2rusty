// Reference dump for the Rust table loaders (crates/m2-formats, crypt and
// proto modules).
//
//   m2diff_proto tables <client dir> <index file> <ymir 0|1> <cvt prefix>
//                <item_list> <item_proto> <itemdesc> <mob_proto>
//                <skilldesc> <skilltable> <insult>
//       Registers the packs like PackInitialize, then runs the loaders in
//       the order and with the early returns of LoadLocaleData
//       (client/src/ui/PythonApplication.cpp) on the given virtual file
//       names, and prints every field of every loaded record.
//       <cvt prefix> is the locale path of the lang%d.cvt files (YMIR only).
//   m2diff_proto lz <file> [k0 k1 k2 k3]
//       CLZO::Decompress of a raw CLZObject: size and fnv1a64, or "fail".
//   m2diff_proto lzpack <in> <out> [k0 k1 k2 k3]
//       CLZO::CompressMemory / CompressEncryptedMemory, for synthetic input.
//
// The item, mob and skill managers are the real ones (client_game,
// client_ui). LoadInsultList and LoadConvertTable belong to
// CPythonNetworkStream, whose construction needs the network layer; their
// few lines are copied below instead.
//
// The loaders trace to stdout and stderr, so the dump and this tool's own
// errors go to duplicates of them and both are sent to /dev/null.

#include <Client/UI/StdAfx.h>
#include <Client/Game/ItemManager.h>
#include <Client/UI/Locale.h>
#include <Client/UI/PythonNonPlayer.h>
#include <Client/UI/PythonSkill.h>
#include <Neoeter/IO/EterPackManager.h>
#include <Neoeter/Platform/Base/FileLoader.h>
#include <Neoeter/Platform/Base/lzo.h>
#include <Neoeter/Render/ResourceManager.h>

#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <string>
#include <unistd.h>
#include <vector>

// Defined by UserInterface.cpp, which is not in client_ui (as in the
// client's own tests).
bool __IS_TEST_SERVER_MODE__ = false;

namespace
{

FILE *out = nullptr;
FILE *err = nullptr;

// Protected state of the managers, reached through derived classes.
struct ItemManagerAccess : CItemManager
{
    using CItemManager::m_ItemMap;
    using CItemManager::m_vec_ItemRange;
};
struct NonPlayerAccess : CPythonNonPlayer
{
    using CPythonNonPlayer::m_NonPlayerDataMap;
};
struct SkillAccess : CPythonSkill
{
    using CPythonSkill::m_SkillDataMap;
};
// Pointers to CItemData's protected members (named through a derived class,
// typed as members of CItemData).
struct ItemDataAccess : CItemData
{
    static constexpr std::string CItemData::*icon = &ItemDataAccess::m_strIconFileName;
    static constexpr std::string CItemData::*model = &ItemDataAccess::m_strModelFileName;
    static constexpr std::string CItemData::*sub = &ItemDataAccess::m_strSubModelFileName;
    static constexpr std::string CItemData::*drop = &ItemDataAccess::m_strDropModelFileName;
    static constexpr std::string CItemData::*desc = &ItemDataAccess::m_strDescription;
    static constexpr std::string CItemData::*summ = &ItemDataAccess::m_strSummary;
};

uint64_t Fnv1a64(const void *data, size_t size)
{
    uint64_t hash = 0xcbf29ce484222325ull;
    for (size_t i = 0; i < size; i++)
    {
        hash ^= static_cast<const unsigned char *>(data)[i];
        hash *= 0x100000001b3ull;
    }
    return hash;
}

// Printable ASCII as is, everything else (and '\') as \xNN.
void Esc(const char *data, size_t size)
{
    for (size_t i = 0; i < size; i++)
    {
        auto const c = static_cast<unsigned char>(data[i]);
        if (c >= 0x20 && c < 0x7f && c != '\\')
            std::fputc(c, out);
        else
            std::fprintf(out, "\\x%02x", c);
    }
}

void Esc(const std::string &s)
{
    Esc(s.data(), s.size());
}

void Hex(const void *data, size_t size)
{
    for (size_t i = 0; i < size; i++)
        std::fprintf(out, "%02x", static_cast<const unsigned char *>(data)[i]);
}

void Field(const char *name, const std::string &value)
{
    std::fprintf(out, " %s=", name);
    Esc(value);
}

uint32_t FloatBits(float f)
{
    uint32_t bits = 0;
    std::memcpy(&bits, &f, sizeof(bits));
    return bits;
}

bool ReadFile(const char *path, std::vector<char> &bytes)
{
    std::ifstream in(path, std::ios::binary);
    if (!in)
        return false;
    bytes.assign(std::istreambuf_iterator<char>(in), std::istreambuf_iterator<char>());
    return true;
}

// Same as tools/differential/pack (PackInitialize's registration loop).
bool RegisterIndex(CEterPackManager &manager, const std::string &packFolder, const std::string &indexFile)
{
    std::vector<char> bytes;
    if (!ReadFile(indexFile.c_str(), bytes))
    {
        std::fprintf(err, "cannot open %s\n", indexFile.c_str());
        return false;
    }

    CMemoryTextFileLoader loader;
    loader.Bind(static_cast<int>(bytes.size()), bytes.data());

    std::string const folder = packFolder + "/";
    CTokenVector tokens;
    for (DWORD i = 0; i < loader.GetLineCount() - 1; i++)
    {
        loader.SplitLineByTab(i, &tokens);
        if (tokens.size() != 2)
            return false;

        std::string const name = folder + tokens[0];
        manager.RegisterPack(name, tokens[1]);

        std::string const texCache = name + "_texcache";
        if (manager.PackExists(texCache, tokens[1]))
            manager.RegisterPack(texCache, tokens[1]);
    }
    return true;
}

// Copy of CPythonNetworkStream::LoadInsultList, keeping the list.
bool LoadInsultList(const char *fileName, std::vector<std::string> &insults)
{
    CEterPackManager::TPackDataPtr data;
    CMemoryTextFileLoader loader;
    if (!CEterPackManager::Instance().Get(fileName, data))
        return false;

    // Bind copies two bytes for a lead byte >= 0x80 even when it is the last
    // byte of the file (locale/hu/insult.txt ends that way), reading past
    // the buffer. Pad with a NUL so that read is defined; the dump prints
    // insults as C strings, so the NUL drops out, as with the Rust
    // text::lines, which clamps instead.
    std::vector<char> padded(data->begin(), data->end());
    padded.push_back('\0');
    loader.Bind(static_cast<int>(data->size()), padded.data());
    insults.clear();
    for (DWORD i = 0; i < loader.GetLineCount(); ++i)
    {
        const std::string &line = loader.GetLineString(i);
        if (!line.empty()) // CInsultChecker::AppendInsult
            insults.push_back(line);
    }
    return true;
}

struct TextConvertTable
{
    char acUpper[26];
    char acLower[26];
    BYTE aacHan[5000][2];
};

// Copy of CPythonNetworkStream::LoadConvertTable.
bool LoadConvertTable(DWORD dwEmpireID, const char *c_szFileName, TextConvertTable *tables)
{
    if (dwEmpireID < 1 || dwEmpireID >= 4)
        return false;

    std::stringstream data;
    if (!CEterPackManager::Instance().Get(c_szFileName, data))
        return false;

    DWORD const dwEngCount = 26;
    DWORD const dwHanCount = (0xc8 - 0xb0 + 1) * (0xfe - 0xa1 + 1);
    DWORD const dwHanSize = dwHanCount * 2;
    DWORD const dwFileSize = (dwEngCount * 2) + dwHanSize;

    if (data.str().length() < dwFileSize)
        return false;

    TextConvertTable &rkTextConvTable = tables[dwEmpireID - 1];
    data.read(rkTextConvTable.acUpper, dwEngCount);
    data.read(rkTextConvTable.acLower, dwEngCount);
    data.read(reinterpret_cast<char *>(rkTextConvTable.aacHan), dwHanSize);

    return !!data;
}

void Step(const std::string &name, bool ok)
{
    std::fprintf(out, "step ");
    Esc(name);
    std::fprintf(out, " %s\n", ok ? "ok" : "fail");
}

void DumpItems(ItemManagerAccess &items)
{
    for (auto const &[vnum, item] : items.m_ItemMap)
    {
        std::fprintf(out, "item %u", vnum);
        Field("icon", item->*ItemDataAccess::icon);
        Field("model", item->*ItemDataAccess::model);
        Field("sub", item->*ItemDataAccess::sub);
        Field("drop", item->*ItemDataAccess::drop);
        Field("desc", item->*ItemDataAccess::desc);
        Field("summ", item->*ItemDataAccess::summ);
        std::fprintf(out, "\n");

        const CItemData::TItemTable &t = *item->GetTable();
        std::fprintf(out, "itemtable %u %u ", t.dwVnum, t.dwVnumRange);
        Hex(t.szName, sizeof(t.szName));
        std::fprintf(out, " ");
        Hex(t.szLocaleName, sizeof(t.szLocaleName));
        std::fprintf(out, " %u %u %u %u %u %u %u %u %u %u", t.bType, t.bSubType, t.bWeight, t.bSize, t.dwAntiFlags,
                     t.dwFlags, t.dwWearFlags, t.dwImmuneFlag, t.dwIBuyItemPrice, t.dwISellItemPrice);
        for (auto const &l : t.aLimits)
            std::fprintf(out, " %u:%d", l.bType, static_cast<int>(l.lValue));
        for (auto const &a : t.aApplies)
            std::fprintf(out, " %u:%d", a.bType, static_cast<int>(a.lValue));
        for (auto const v : t.alValues)
            std::fprintf(out, " %d", static_cast<int>(v));
        for (auto const s : t.alSockets)
            std::fprintf(out, " %d", static_cast<int>(s));
        std::fprintf(out, " %u %u %u %u %u\n", t.dwRefinedVnum, t.wRefineSet, t.bAlterToMagicItemPct, t.bSpecular,
                     t.bGainSocketPct);
    }
    for (CItemData *item : items.m_vec_ItemRange)
        std::fprintf(out, "range %u\n", item->GetTable()->dwVnum);
}

void DumpMobs(NonPlayerAccess &mobs)
{
    for (auto const &[vnum, m] : mobs.m_NonPlayerDataMap)
    {
        std::fprintf(out, "mob %u ", m->dwVnum);
        Hex(m->szName, sizeof(m->szName));
        std::fprintf(out, " ");
        Hex(m->szLocaleName, sizeof(m->szLocaleName));
        std::fprintf(out, " %u %u %u %u %u %u %u %u %u %u %u %u %u %u %u %u %u %u %u %u %u %d %d %u %u %u", m->bType,
                     m->bRank, m->bBattleType, m->bLevel, m->bSize, m->dwGoldMin, m->dwGoldMax, m->dwExp, m->dwMaxHP,
                     m->bRegenCycle, m->bRegenPercent, m->wDef, m->dwAIFlag, m->dwRaceFlag, m->dwImmuneFlag, m->bStr,
                     m->bDex, m->bCon, m->bInt, m->dwDamageRange[0], m->dwDamageRange[1], m->sAttackSpeed,
                     m->sMovingSpeed, m->bAggresiveHPPct, m->wAggressiveSight, m->wAttackRange);
        for (char const e : m->cEnchants)
            std::fprintf(out, " %d", static_cast<signed char>(e));
        for (char const r : m->cResists)
            std::fprintf(out, " %d", static_cast<signed char>(r));
        std::fprintf(out, " %u %u %u %u %u ", m->dwResurrectionVnum, m->dwDropItemVnum, m->bMountCapacity,
                     m->bOnClickType, m->bEmpire);
        Hex(m->szFolder, sizeof(m->szFolder));
        std::fprintf(out, " %08x %u %u %u %u", FloatBits(m->fDamMultiply), m->dwSummonVnum, m->dwDrainSP,
                     m->dwMonsterColor, m->dwPolymorphItemVnum);
        for (auto const &s : m->Skills)
            std::fprintf(out, " %u:%u", s.dwVnum, s.bLevel);
        std::fprintf(out, " %u %u %u %u %u\n", m->bBerserkPoint, m->bStoneSkinPoint, m->bGodSpeedPoint,
                     m->bDeathBlowPoint, m->bRevivePoint);
    }
}

void DumpSkills(SkillAccess &skills)
{
    for (auto const &[index, s] : skills.m_SkillDataMap)
    {
        std::fprintf(out, "skill %u type=%u index=%u max=%u up=%u limit=%u nomotion=%d motion=%u attr=%u weapon=%u range=%u",
                     index, s.byType, s.dwSkillIndex, s.byMaxLevel, s.byLevelUpPoint, s.byLevelLimit,
                     s.bNoMotion ? 1 : 0, s.wMotionIndex, s.dwSkillAttribute, s.dwNeedWeapon, s.dwTargetRange);
        Field("name", s.strName);
        Field("desc", s.strDescription);
        Field("cool", s.strCoolTimeFormula);
        Field("tc", s.strTargetCountFormula);
        Field("lc", s.strMotionLoopCountFormula);
        Field("sp", s.strNeedSPFormula);
        Field("cont", s.strContinuationSPFormula);
        Field("dur", s.strDuration);
        std::fprintf(out, "\n");
        for (int g = 0; g < CPythonSkill::SKILL_EFFECT_COUNT; ++g)
        {
            std::fprintf(out, "skillgrade %u %d %u", index, g, s.GradeData[g].wMotionIndex);
            Field("name", s.GradeData[g].strName);
            std::fprintf(out, "\n");
        }
        for (auto const &c : s.ConditionDataVector)
        {
            std::fprintf(out, "skillcond %u", index);
            Field("c", c);
            std::fprintf(out, "\n");
        }
        for (auto const &a : s.AffectDataVector)
        {
            std::fprintf(out, "skillaffect %u", index);
            Field("desc", a.strAffectDescription);
            Field("min", a.strAffectMinFormula);
            Field("max", a.strAffectMaxFormula);
            std::fprintf(out, "\n");
        }
    }
}

std::vector<DWORD> Key(int argc, char **argv, int first)
{
    std::vector<DWORD> key;
    for (int i = first; i < first + 4 && i < argc; ++i)
        key.push_back(static_cast<DWORD>(std::strtoul(argv[i], nullptr, 10)));
    return key;
}

int Tables(int argc, char **argv)
{
    if (argc != 13)
    {
        std::fprintf(err, "tables: expected 11 arguments\n");
        return 2;
    }
    std::filesystem::current_path(argv[2]);
    bool const ymir = std::strcmp(argv[4], "1") == 0;
    std::string const cvtPrefix = argv[5];
    const char *itemList = argv[6];
    const char *itemProto = argv[7];
    const char *itemDesc = argv[8];
    const char *mobProto = argv[9];
    const char *skillDesc = argv[10];
    const char *skillTable = argv[11];
    const char *insult = argv[12];

    if (ymir)
        LocaleService_ForceSetLocale("ymir", cvtPrefix.c_str());
    if (LocaleService_IsYMIR() != ymir)
    {
        std::fprintf(err, "cannot select the %s locale\n", ymir ? "YMIR" : "non-YMIR");
        return 1;
    }

    CEterPackManager packs;
    packs.SetSearchMode(true);
    CResourceManager resources;
    CLZO lzo;
    ItemManagerAccess items;
    NonPlayerAccess mobs;
    SkillAccess skills;
    std::vector<std::string> insults;
    static TextConvertTable convert[3] = {};
    bool convertLoaded[3] = {};

    if (!RegisterIndex(packs, "pack", argv[3]))
        return 1;

    // LoadLocaleData.
    [&] {
        Step(itemList, items.LoadItemList(itemList));
        bool ok = items.LoadItemTable(itemProto);
        Step(itemProto, ok);
        if (!ok)
            return;
        Step(itemDesc, items.LoadItemDesc(itemDesc));
        ok = mobs.LoadNonPlayerData(mobProto);
        Step(mobProto, ok);
        if (!ok)
            return;
        ok = skills.RegisterSkillDesc(skillDesc);
        Step(skillDesc, ok);
        if (!ok)
            return;
        ok = skills.RegisterSkillTable(skillTable);
        Step(skillTable, ok);
        if (!ok)
            return;
        Step(insult, LoadInsultList(insult, insults));
        if (LocaleService_IsYMIR())
        {
            for (DWORD empire = 1; empire <= 3; ++empire)
            {
                char name[256];
                std::snprintf(name, sizeof(name), "%s/lang%u.cvt", cvtPrefix.c_str(), empire);
                convertLoaded[empire - 1] = LoadConvertTable(empire, name, convert);
                Step(name, convertLoaded[empire - 1]);
            }
        }
    }();

    DumpItems(items);
    DumpMobs(mobs);
    DumpSkills(skills);
    for (auto const &insultLine : insults)
    {
        std::fprintf(out, "insult ");
        Esc(insultLine.c_str(), std::strlen(insultLine.c_str()));
        std::fprintf(out, "\n");
    }
    for (int i = 0; i < 3; ++i)
    {
        if (!convertLoaded[i])
            continue;
        std::fprintf(out, "cvt %d ", i + 1);
        Hex(convert[i].acUpper, 26);
        std::fprintf(out, " ");
        Hex(convert[i].acLower, 26);
        std::fprintf(out, " %016llx\n", static_cast<unsigned long long>(Fnv1a64(convert[i].aacHan, 2350 * 2)));
    }
    return 0;
}

int Lz(int argc, char **argv)
{
    std::vector<char> bytes;
    if (argc < 3 || !ReadFile(argv[2], bytes))
        return 2;
    std::vector<DWORD> key = Key(argc, argv, 3);
    // Slack so a corrupt header cannot read past the buffer.
    bytes.resize(bytes.size() + 64 * 1024);

    CLZO lzo;
    CLZObject obj;
    if (CLZO::Instance().Decompress(obj, reinterpret_cast<const BYTE *>(bytes.data()),
                                    key.size() == 4 ? key.data() : nullptr))
        std::fprintf(out, "lz ok %u %016llx\n", obj.GetHeader().dwRealSize,
                     static_cast<unsigned long long>(Fnv1a64(obj.GetBuffer(), obj.GetHeader().dwRealSize)));
    else
        std::fprintf(out, "lz fail\n");
    return 0;
}

int LzPack(int argc, char **argv)
{
    std::vector<char> bytes;
    if (argc < 4 || !ReadFile(argv[2], bytes))
        return 2;
    std::vector<DWORD> key = Key(argc, argv, 4);

    CLZO lzo;
    CLZObject obj;
    bool const ok = key.size() == 4
                        ? CLZO::Instance().CompressEncryptedMemory(obj, bytes.data(), bytes.size(), key.data())
                        : CLZO::Instance().CompressMemory(obj, bytes.data(), bytes.size());
    if (!ok)
        return 1;
    std::ofstream o(argv[3], std::ios::binary);
    o.write(reinterpret_cast<const char *>(obj.GetBuffer()), obj.GetSize());
    return o ? 0 : 1;
}

} // namespace

int main(int argc, char **argv)
{
    out = fdopen(dup(STDOUT_FILENO), "w");
    err = fdopen(dup(STDERR_FILENO), "w");
    if (!out || !err || !std::freopen("/dev/null", "w", stdout) || !std::freopen("/dev/null", "w", stderr))
        return 1;

    std::string const mode = argc > 1 ? argv[1] : "";
    int ret = 2;
    if (mode == "tables")
        ret = Tables(argc, argv);
    else if (mode == "lz")
        ret = Lz(argc, argv);
    else if (mode == "lzpack")
        ret = LzPack(argc, argv);
    else
        std::fprintf(err, "usage: %s tables|lz|lzpack ...\n", argv[0]);
    std::fflush(out);
    std::fflush(err);
    // Skip the singletons' destructors (the item pool and the resource
    // manager tear down in an order that is irrelevant here).
    std::_Exit(ret);
}
