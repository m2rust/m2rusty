// Reference dump for the Rust race, motion and property readers
// (crates/m2-formats/src/race, src/property).
//
//   m2diff_race <client dir> <index file> <mode>
//
// Registers packs like m2diff_pack, then, for every registered file of the
// mode's kind (sorted by name):
//
//   race       .msm through CRaceData::LoadRaceData — see LoadRaceDataFixed
//   race-real  .msm through the real CRaceData::LoadRaceData, as built
//   motion     .msa through CRaceMotionData::LoadMotionData (which also
//              loads the sound script sound/<...>.mss)
//   property   every property/ file through CProperty::ReadFromMemory, then
//              CPropertyManager::Initialize and its CRC map and reserved set
//
// Lines are tab separated; names and tokens are raw bytes; floats are the hex
// of their IEEE bits. See DumpRace / DumpMotion / DumpProperty for the line
// kinds. Files whose text CMemoryTextFileLoader::Bind would read past the end
// of (a lead byte >= 0x80 as the last byte: undefined) print "F <name> skip".

#include <Client/Game/StdAfx.h>

#include <Client/Game/FlyingObjectManager.h>
#include <Client/Game/Property.h>
#include <Client/Game/PropertyManager.h>
#include <Client/Game/RaceData.h>
#include <Client/Game/RaceMotionData.h>
#include <Neoeter/Effect/EffectManager.h>
#include <Neoeter/IO/EterPackManager.h>
#include <Neoeter/Platform/Base/FileLoader.h>
#include <Neoeter/Platform/Base/Filename.h>
#include <Neoeter/Render/ResourceManager.h>
#include <Neoeter/Render/TextFileLoader.h>

#include <algorithm>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <string>
#include <vector>

#include <unistd.h>

namespace
{

// The dump. TraceError writes to stdout, so main moves stdout to stderr and
// keeps the original descriptor for this.
FILE *g_out = nullptr;

// Mirrors PackInitialize's registration loop (as m2diff_pack does).
bool RegisterIndex(CEterPackManager &manager, const std::string &packFolder, const std::string &indexFile)
{
    std::ifstream in(indexFile, std::ios::binary);
    if (!in)
    {
        std::fprintf(stderr, "cannot open %s\n", indexFile.c_str());
        return false;
    }
    std::vector<char> bytes((std::istreambuf_iterator<char>(in)), std::istreambuf_iterator<char>());

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

void Put(const std::string &s)
{
    std::fwrite(s.data(), 1, s.size(), g_out);
}

// "\t<s>"
void Field(const std::string &s)
{
    std::fputc('\t', g_out);
    Put(s);
}

void Field(float f)
{
    uint32_t bits = 0;
    std::memcpy(&bits, &f, sizeof(f));
    std::fprintf(g_out, "\t%08x", bits);
}

void Field(const D3DXVECTOR3 &v)
{
    Field(v.x);
    Field(v.y);
    Field(v.z);
}

void FieldInt(long long v)
{
    std::fprintf(g_out, "\t%lld", v);
}

void FieldHex(uint32_t v)
{
    std::fprintf(g_out, "\t%08x", v);
}

void Line(const char *kind)
{
    std::fputs(kind, g_out);
}

void End()
{
    std::fputc('\n', g_out);
}

// Whether CMemoryTextFileLoader::Bind reads past the end of `size` bytes.
bool Overreads(const char *data, size_t size)
{
    size_t pos = 0;
    while (pos < size)
    {
        char const c = data[pos++];
        if (c < 0)
        {
            if (pos == size)
                return true;
            ++pos;
        }
    }
    return false;
}

bool FileOverreads(CEterPackManager &manager, const std::string &name)
{
    CEterPackManager::TPackDataPtr data;
    if (!manager.Get(name, data) || !data)
        return false;
    return Overreads(data->data(), data->size());
}

std::string Extension(const std::string &name)
{
    auto const dot = name.find_last_of('.');
    if (dot == std::string::npos)
        return {};
    std::string ext = name.substr(dot + 1);
    for (char &c : ext)
        if (c >= 'A' && c <= 'Z')
            c = static_cast<char>(c - 'A' + 'a');
    return ext;
}

/////////////////////////////////////////////////////////////////////////////
// Race data (.msm)

void DumpSpheres(const CSphereCollisionInstanceVector &spheres)
{
    for (auto const &sphere : spheres)
    {
        Line("SPH");
        Field(sphere.GetAttribute().fRadius);
        Field(sphere.GetAttribute().v3Position);
        End();
    }
}

void DumpCollision(const NRaceData::TCollisionData &c)
{
    Line("COL");
    FieldInt(c.iCollisionType);
    FieldInt(static_cast<long long>(c.SphereDataVector.size()));
    End();
    DumpSpheres(c.SphereDataVector);
}

void DumpAttaching(const NRaceData::TAttachingDataVector &v)
{
    for (size_t i = 0; i < v.size(); ++i)
    {
        auto const &a = v[i];
        Line("A");
        FieldInt(static_cast<long long>(i));
        FieldInt(a.dwType);
        FieldInt(a.isAttaching);
        FieldInt(a.dwAttachingModelIndex);
        Field(a.strAttachingBoneName);
        End();
        if (a.pCollisionData)
            DumpCollision(*a.pCollisionData);
        if (a.pEffectData)
        {
            Line("AEFF");
            Field(a.pEffectData->strFileName);
            Field(a.pEffectData->v3Position);
            Field(a.pEffectData->v3Rotation);
            End();
        }
        if (a.pObjectData)
        {
            Line("AOBJ");
            Field(a.pObjectData->strFileName);
            End();
        }
    }
}

struct RaceAccess : CRaceData
{
    // CRaceData::LoadRaceData (client/src/game/RaceDataFile.cpp) verbatim,
    // except that dwShapeDataCount and dwHairDataCount are not `const`. In
    // the C++ as built they are `DWORD const ... = 0` (written through
    // GetTokenDoubleWord's `const DWORD *`), and a const integer initialised
    // with a constant is folded by the compiler, so both loops run zero
    // times and no shape or hair is ever loaded. The Rust port keeps the
    // original behaviour; mode race-real dumps the function as built.
    BOOL LoadRaceDataFixed(const char *c_szFileName)
    {
        CTextFileLoader TextFileLoader;
        if (!TextFileLoader.Load(c_szFileName))
            return FALSE;

        TextFileLoader.SetTop();

        TextFileLoader.GetTokenString("basemodelfilename", &m_strBaseModelFileName);
        TextFileLoader.GetTokenString("treefilename", &m_strTreeFileName);
        TextFileLoader.GetTokenString("attributefilename", &m_strAttributeFileName);
        TextFileLoader.GetTokenString("smokebonename", &m_strSmokeBoneName);
        TextFileLoader.GetTokenString("motionlistfilename", &m_strMotionListFileName);

        if (!m_strTreeFileName.empty())
        {
            CFileNameHelper::StringPath(m_strTreeFileName);
        }

        CTokenVector *pSmokeTokenVector = nullptr;
        if (TextFileLoader.GetTokenVector("smokefilename", &pSmokeTokenVector))
        {
            if (pSmokeTokenVector->size() % 2 != 0)
            {
                TraceError("SmokeFileName ArgCount[%d]%2==0", pSmokeTokenVector->size());
                return FALSE;
            }

            UINT const uLineCount = pSmokeTokenVector->size() / 2;

            for (UINT uLine = 0; uLine < uLineCount; ++uLine)
            {
                int const eSmoke = atoi(pSmokeTokenVector->at((uLine * 2) + 0).c_str());
                if (eSmoke < 0 || eSmoke >= SMOKE_NUM)
                {
                    TraceError("SmokeFileName SmokeNum[%d] OUT OF RANGE", eSmoke);
                    return FALSE;
                }

                const std::string &c_rstrEffectFileName = pSmokeTokenVector->at((uLine * 2) + 1);

                DWORD &rdwCRCEft = m_adwSmokeEffectID[eSmoke];
                if (!CEffectManager::Instance().RegisterEffect2(c_rstrEffectFileName.c_str(), &rdwCRCEft))
                {
                    TraceError("CRaceData::RegisterEffect2(%s) ERROR", c_rstrEffectFileName.c_str());
                    rdwCRCEft = 0;
                    return false;
                }
            }
        }

        if (TextFileLoader.SetChildNode("shapedata"))
        {
            std::string strPathName;
            DWORD dwShapeDataCount = 0;
            if (TextFileLoader.GetTokenString("pathname", &strPathName) &&
                TextFileLoader.GetTokenDoubleWord("shapedatacount", &dwShapeDataCount))
            {
                for (DWORD i = 0; i < dwShapeDataCount; ++i)
                {
                    if (!TextFileLoader.SetChildNode("shapedata", i))
                    {
                        continue;
                    }

                    TextFileLoader.GetTokenString("specialpath", &strPathName);

                    DWORD dwShapeIndex = 0;
                    if (!TextFileLoader.GetTokenDoubleWord("shapeindex", &dwShapeIndex))
                    {
                        continue;
                    }

                    std::string strModel;
                    if (TextFileLoader.GetTokenString("model", &strModel))
                    {
                        SetShapeModel(dwShapeIndex, (strPathName + strModel).c_str());
                    }
                    else
                    {
                        if (!TextFileLoader.GetTokenString("local_model", &strModel))
                            continue;

                        SetShapeModel(dwShapeIndex, strModel.c_str());
                    }

                    std::string strSourceSkin;
                    std::string strTargetSkin;

                    if (TextFileLoader.GetTokenString("local_sourceskin", &strSourceSkin) &&
                        TextFileLoader.GetTokenString("local_targetskin", &strTargetSkin))
                    {
                        AppendShapeSkin(dwShapeIndex, 0, strSourceSkin.c_str(), strTargetSkin.c_str());
                    }

                    if (TextFileLoader.GetTokenString("sourceskin", &strSourceSkin) &&
                        TextFileLoader.GetTokenString("targetskin", &strTargetSkin))
                    {
                        AppendShapeSkin(dwShapeIndex, 0, (strPathName + strSourceSkin).c_str(),
                                        (strPathName + strTargetSkin).c_str());
                    }

                    if (TextFileLoader.GetTokenString("sourceskin2", &strSourceSkin) &&
                        TextFileLoader.GetTokenString("targetskin2", &strTargetSkin))
                    {
                        AppendShapeSkin(dwShapeIndex, 0, (strPathName + strSourceSkin).c_str(),
                                        (strPathName + strTargetSkin).c_str());
                    }

                    TextFileLoader.SetParentNode();
                }
            }

            TextFileLoader.SetParentNode();
        }

        if (TextFileLoader.SetChildNode("hairdata"))
        {
            std::string strPathName;
            DWORD dwHairDataCount = 0;
            if (TextFileLoader.GetTokenString("pathname", &strPathName) &&
                TextFileLoader.GetTokenDoubleWord("hairdatacount", &dwHairDataCount))
            {
                for (DWORD i = 0; i < dwHairDataCount; ++i)
                {
                    if (!TextFileLoader.SetChildNode("hairdata", i))
                    {
                        continue;
                    }

                    TextFileLoader.GetTokenString("specialpath", &strPathName);

                    DWORD dwShapeIndex = 0;
                    if (!TextFileLoader.GetTokenDoubleWord("hairindex", &dwShapeIndex))
                    {
                        continue;
                    }

                    std::string strModel;
                    std::string strSourceSkin;
                    std::string strTargetSkin;
                    if (TextFileLoader.GetTokenString("model", &strModel) &&
                        TextFileLoader.GetTokenString("sourceskin", &strSourceSkin) &&
                        TextFileLoader.GetTokenString("targetskin", &strTargetSkin))
                    {
                        SetHairSkin(dwShapeIndex, 0, (strPathName + strModel).c_str(),
                                    (strPathName + strSourceSkin).c_str(), (strPathName + strTargetSkin).c_str());
                    }

                    TextFileLoader.SetParentNode();
                }
            }

            TextFileLoader.SetParentNode();
        }

        if (TextFileLoader.SetChildNode("attachingdata"))
        {
            if (!NRaceData::LoadAttachingData(TextFileLoader, &m_AttachingDataVector))
                return FALSE;

            TextFileLoader.SetParentNode();
        }

        return TRUE;
    }

    void Dump()
    {
        Line("S\tbase");
        Field(m_strBaseModelFileName);
        End();
        Line("S\ttree");
        Field(m_strTreeFileName);
        End();
        Line("S\tattribute");
        Field(m_strAttributeFileName);
        End();
        Line("S\tsmokebone");
        Field(m_strSmokeBoneName);
        End();
        Line("S\tmotionlist");
        Field(m_strMotionListFileName);
        End();
        Line("SMOKE");
        for (DWORD id : m_adwSmokeEffectID)
            FieldHex(id);
        End();
        for (auto const &[index, shape] : m_kMap_dwShapeKey_kShape)
        {
            Line("SHAPE");
            FieldInt(index);
            Field(shape.m_stModelFileName);
            End();
            DumpSkins(shape.m_kVct_kSkin);
        }
        for (auto const &[index, hair] : m_kMap_dwHairKey_kHair)
        {
            Line("HAIR");
            FieldInt(index);
            Field(hair.m_stModelFileName);
            End();
            DumpSkins(hair.m_kVct_kSkin);
        }
        DumpAttaching(m_AttachingDataVector);
    }

    static void DumpSkins(const std::vector<SSkin> &skins)
    {
        for (auto const &skin : skins)
        {
            Line("SKIN");
            FieldInt(skin.m_ePart);
            Field(skin.m_stSrcFileName);
            Field(skin.m_stDstFileName);
            End();
        }
    }
};

void DumpRace(const std::string &name, bool real)
{
    RaceAccess race;
    bool const ok = real ? race.LoadRaceData(name.c_str()) : race.LoadRaceDataFixed(name.c_str());
    std::fputs("F\t", g_out);
    Put(name);
    std::fprintf(g_out, "\t%d\n", ok ? 1 : 0);
    race.Dump();
}

/////////////////////////////////////////////////////////////////////////////
// Motion data (.msa)

using NMotionEvent::SMotionEventData;

void DumpEvent(size_t index, const SMotionEventData *e)
{
    Line("E");
    FieldInt(static_cast<long long>(index));
    FieldInt(e->iType);
    FieldInt(e->dwFrame);
    Field(e->fStartingTime);
    Field(e->fDurationTime);
    End();

    if (auto const *p = dynamic_cast<const CRaceMotionData::TMotionEffectEventData *>(e))
    {
        Line("EEFF");
        FieldInt(p->isIndependent);
        FieldInt(p->isAttaching);
        FieldInt(p->isFollowing);
        Field(p->strAttachingBoneName);
        Field(p->strEffectFileName);
        Field(p->v3EffectPosition);
        FieldHex(p->dwEffectIndex);
        End();
    }
    else if (auto const *p = dynamic_cast<const CRaceMotionData::TMotionEffectToTargetEventData *>(e))
    {
        Line("ETGT");
        Field(p->strEffectFileName);
        Field(p->v3EffectPosition);
        FieldInt(p->isFollowing);
        FieldInt(p->isFishingEffect);
        FieldHex(p->dwEffectIndex);
        End();
    }
    else if (auto const *p = dynamic_cast<const CRaceMotionData::TMotionFlyEventData *>(e))
    {
        Line("EFLY");
        FieldInt(p->isAttaching);
        Field(p->strAttachingBoneName);
        Field(p->strFlyFileName);
        Field(p->v3FlyPosition);
        FieldHex(p->dwFlyIndex);
        End();
    }
    else if (auto const *p = dynamic_cast<const CRaceMotionData::TScreenWavingEventData *>(e))
    {
        Line("EWAV");
        FieldInt(p->iPower);
        FieldInt(p->iAffectingRange);
        End();
    }
    else if (auto const *p = dynamic_cast<const CRaceMotionData::TMotionAttackingEventData *>(e))
    {
        auto const &a = p->AttackData;
        Line("EATK");
        FieldInt(p->isEnableHitProcess);
        FieldInt(a.iAttackType);
        FieldInt(a.iHittingType);
        Field(a.fInvisibleTime);
        Field(a.fExternalForce);
        Field(a.fStiffenTime);
        FieldInt(a.iHitLimitCount);
        End();
        DumpCollision(p->CollisionData);
    }
    else if (auto const *p = dynamic_cast<const CRaceMotionData::TMotionSoundEventData *>(e))
    {
        Line("ESND");
        Field(p->strSoundFileName);
        End();
    }
}

struct MotionAccess : CRaceMotionData
{
    void Dump()
    {
        Line("M");
        Field(m_strMotionFileName);
        Field(m_fMotionDuration);
        End();
        Line("ACC");
        FieldInt(m_isAccumulationMotion);
        Field(m_accumulationPosition);
        End();
        Line("COMBO");
        FieldInt(m_isComboMotion);
        Field(m_ComboInputData.fInputStartTime);
        Field(m_ComboInputData.fNextComboTime);
        Field(m_ComboInputData.fInputEndTime);
        End();
        Line("LOOP");
        FieldInt(m_isLoopMotion);
        FieldInt(m_iLoopCount);
        FieldInt(m_bCancelEnableSkill);
        Field(m_fLoopStartTime);
        Field(m_fLoopEndTime);
        End();
        Line("SPLASH");
        FieldInt(HasSplashMotionEvent());
        End();
        Line("ATK");
        FieldInt(m_isAttackingMotion);
        if (m_isAttackingMotion)
        {
            auto const &a = m_MotionAttackData;
            FieldInt(a.iAttackType);
            FieldInt(a.iHittingType);
            Field(a.fInvisibleTime);
            Field(a.fExternalForce);
            Field(a.fStiffenTime);
            FieldInt(a.iHitLimitCount);
            FieldInt(a.iMotionType);
            FieldInt(static_cast<long long>(a.HitDataContainer.size()));
        }
        End();
        if (m_isAttackingMotion)
        {
            for (auto const &hit : m_MotionAttackData.HitDataContainer)
            {
                Line("HIT");
                Field(hit.fAttackStartTime);
                Field(hit.fAttackEndTime);
                Field(hit.fWeaponLength);
                Field(hit.strBoneName);
                FieldInt(static_cast<long long>(hit.mapHitPosition.size()));
                End();
                for (auto const &[time, pos] : hit.mapHitPosition)
                {
                    Line("HP");
                    Field(time);
                    Field(pos.v3LastPosition);
                    Field(pos.v3Position);
                    End();
                }
            }
        }
        for (size_t i = 0; i < m_MotionEventDataVector.size(); ++i)
            DumpEvent(i, m_MotionEventDataVector[i]);
        Line("SND");
        Field(m_strSoundScriptDataFileName);
        FieldInt(static_cast<long long>(m_SoundInstanceVector.size()));
        End();
        for (auto const &s : m_SoundInstanceVector)
        {
            Line("SI");
            FieldInt(s.dwFrame);
            Field(s.strSoundFileName);
            End();
        }
    }
};

// The sound script LoadMotionData reads for `name`.
std::string SoundScriptName(const std::string &name)
{
    std::string s = CFileNameHelper::NoExtension(name) + ".mss";
    if (s.length() <= 13)
        return {};
    return "sound/" + s.substr(13);
}

void DumpMotion(const std::string &name)
{
    MotionAccess motion;
    bool const ok = motion.LoadMotionData(name.c_str());
    std::fputs("F\t", g_out);
    Put(name);
    std::fprintf(g_out, "\t%d\n", ok ? 1 : 0);
    // A failed motion is deleted by its caller (CRaceData::RegisterMotionData).
    if (ok)
        motion.Dump();
}

/////////////////////////////////////////////////////////////////////////////
// Properties

struct PropertyAccess : CProperty
{
    using CProperty::CProperty;
    void Dump()
    {
        Line("P");
        Field(m_stFileName);
        Field(mc_pFileName);
        Field(m_stCRC);
        FieldHex(m_dwCRC);
        FieldInt(static_cast<long long>(m_stTokenMap.size()));
        End();
        for (auto const &[key, tokens] : m_stTokenMap)
        {
            Line("K");
            Field(key);
            FieldInt(static_cast<long long>(tokens.size()));
            End();
            for (auto const &token : tokens)
            {
                Line("T");
                Field(token);
                End();
            }
        }
    }
};

struct ManagerAccess : CPropertyManager
{
    void Dump()
    {
        for (auto const &[crc, property] : m_PropertyByCRCMap)
        {
            Line("CRC");
            FieldHex(crc);
            Field(property->GetFileName());
            End();
        }
        for (DWORD crc : m_ReservedCRCSet)
        {
            Line("RESERVED");
            FieldHex(crc);
            End();
        }
    }
};

void DumpPropertyFile(CEterPackManager &manager, const std::string &name)
{
    CEterPackManager::TPackDataPtr data;
    if (!manager.Get(name, data) || !data)
        return;
    std::fputs("F\t", g_out);
    Put(name);
    // ReadFromMemory reads the 4-byte FourCC and the byte after it without
    // checking the size; shorter files are not comparable.
    if (data->size() < 5)
    {
        std::fputs("\tshort\n", g_out);
        return;
    }
    // Only YPRT files reach Bind; the header bytes are all ASCII.
    if (std::memcmp(data->data(), "YPRT", 4) == 0 && Overreads(data->data(), data->size()))
    {
        std::fputs("\tskip\n", g_out);
        return;
    }
    PropertyAccess property(name.c_str());
    bool const ok = property.ReadFromMemory(data->data(), static_cast<int>(data->size()), name.c_str());
    std::fprintf(g_out, "\t%d\n", ok ? 1 : 0);
    if (ok)
        property.Dump();
}

} // namespace

int main(int argc, char **argv)
{
    if (argc < 4)
    {
        std::fprintf(stderr, "usage: %s <client dir> <index file> <race|race-real|motion|property>\n", argv[0]);
        return 2;
    }

    std::fflush(stdout);
    g_out = fdopen(dup(STDOUT_FILENO), "w");
    dup2(STDERR_FILENO, STDOUT_FILENO);

    std::filesystem::current_path(argv[1]);
    std::string const mode = argv[3];

    // Singletons the loaders reach: effects (smoke, attaching and motion
    // event effects), flying data (fly events), resources, properties.
    static CEterPackManager manager;
    manager.SetSearchMode(true);
    if (!RegisterIndex(manager, "pack", argv[2]))
        return 1;
    static CResourceManager resources;
    static CEffectManager effects;
    static CFlyingManager flying;
    static ManagerAccess properties;

    std::vector<std::string> names;
    for (auto const &entry : manager.GetFileMap())
        names.push_back(entry.first);
    std::ranges::sort(names);

    for (auto const &name : names)
    {
        if (mode == "race" || mode == "race-real")
        {
            if (Extension(name) != "msm")
                continue;
            if (FileOverreads(manager, name))
            {
                std::fputs("F\t", g_out);
                Put(name);
                std::fputs("\tskip\n", g_out);
                continue;
            }
            DumpRace(name, mode == "race-real");
        }
        else if (mode == "motion")
        {
            if (Extension(name) != "msa")
                continue;
            std::string const sound = SoundScriptName(name);
            if (FileOverreads(manager, name) || (!sound.empty() && FileOverreads(manager, sound)))
            {
                std::fputs("F\t", g_out);
                Put(name);
                std::fputs("\tskip\n", g_out);
                continue;
            }
            DumpMotion(name);
        }
        else if (mode == "property")
        {
            if (name.starts_with("property/"))
                DumpPropertyFile(manager, name);
        }
    }

    if (mode == "property")
    {
        // Initialize walks the std::unordered_map file map; when two files
        // share a CRC the later one wins, so the order is part of the result.
        for (auto const &entry : manager.GetFileMap())
        {
            if (entry.first.starts_with("property/"))
            {
                Line("ORDER");
                Field(entry.first);
                End();
            }
        }
        properties.Initialize();
        properties.Dump();
    }

    std::fflush(g_out);
    // The engine's pools and singletons are not torn down in a defined order.
    _exit(0);
}
