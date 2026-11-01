// Reference dump for the Rust map readers (crates/m2-formats/src/map).
//
//   m2diff_map <client dir> <index file> <props|maps|textures|atlas>
//
// Registers packs like m2diff_script, then:
//
//   props:    P <crc>      every property CRC CPropertyManager::Initialize
//                          registers (the Rust test feeds these to its area
//                          filter; properties are another module's format).
//   maps:     for every map directory (a registered */setting.txt or
//             */mapproperty.txt), sorted:
//               M  <dir>
//               MP <ok> <type> <parent>                 CMapBase::LoadProperty
//               S  <ok> <visible> <radius> <hscale> <cx> <cy> <bx> <by> <env>
//               SM <splat alpha 16> <static shadow 16>  (when ok)
//               T  <ok> <file> <count>                  the map's CTextureSet
//               TT <i> <file> <u> <v> <uo> <vo> <splat> <begin> <end> <matrix 16>
//               E  <environment file>                   as CMapOutdoor::Load picks it
//               R  <ok> <count>                         LoadMonsterAreaInfo
//               RA <type> <ox> <oy> <sx> <sy> <l> <t> <r> <b> <vid> <group> <count> <dir>
//             then for every NNNNNN/ directory of the data directory:
//               D  <id>
//               A  <object ok> <ambience ok> <n>        CArea::__Load_LoadObject + __Load_LoadAmbience
//               AO <x> <y> <z> <crc> <yaw> <pitch> <roll> <bias> <portal x8> <range> <pct>
//               AS <crc>...                             the CRCs after CArea's std::ranges::sort
//               TP <ok> <name>                          LoadTerrain's AreaProperty checks
//               TL <water> <height> <attr> <tile> <shadow>
//               TH <height> <normal> <attr> <tile> <water> <nwater> <wheights> <shadow> <patchcount>
//               TC <texture> <count>                    nonzero TileCount entries
//   textures: for every registered textureset/*.txt: T/TT as above.
//   atlas:    for every registered */atlasinfo.txt: F <file>, then
//               I <name> <bx> <by> <sx> <sy> <ex> <ey>
//
// Fields are tab separated, floats are the hex of their bits, TH fields are
// FNV-1a 64 hashes of the arrays' bytes.
//
// What is real C++ and what is replayed: CMapBase::LoadProperty,
// CMapOutdoor::LoadSetting (with CTextureSet::Load), LoadMonsterAreaInfo,
// CArea::__Load_LoadObject/__Load_LoadAmbience (on a CArea, with a real
// CPropertyManager), the CTerrain/CTerrainImpl loaders (LoadWaterMap,
// LoadHeightMap with CalculateNormal, LoadAttrMap, RAW_LoadTileMap's
// CTerrainImpl part + RAW_CountTiles, LoadShadowMap) and
// CMapManager::__LoadMapInfoVector are called as they are. Not callable
// headless, so replayed here line for line: CMapOutdoor::LoadTerrain's
// AreaProperty checks and call sequence (it also builds splat textures and
// patches through the device), the area/terrain path formatting of
// LoadArea/LoadTerrain, and CMapOutdoor::Load's environment file choice
// (Load also builds quadtrees and textures). CTerrain::RAW_LoadTileMap is
// replaced by its two data steps because RAW_GenerateSplat needs a device.

#include <Client/Game/StdAfx.h>

#include <algorithm>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <map>
#include <set>
#include <string>
#include <vector>

#include <unistd.h>

// m_bSettingTerrainVisible is private and has no getter.
#define private public
#include <Client/Game/MapOutdoor.h>
#undef private
#include <Client/Game/Area.h>
#include <Client/Game/AreaTerrain.h>
#include <Client/Game/MapManager.h>
#include <Client/Game/PropertyManager.h>
#include <Neoeter/IO/EterPackManager.h>
#include <Neoeter/Platform/Base/FileLoader.h>
#include <Neoeter/Platform/Base/Timer.h>
#include <Neoeter/Render/ResourceManager.h>
#include <Neoeter/Render/Util.h>
#include <Neoeter/Terrain/TextureSet.h>

namespace
{

FILE *g_out = nullptr;

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

void Tab()
{
    std::fputc('\t', g_out);
}

void PutF(float f)
{
    uint32_t bits = 0;
    std::memcpy(&bits, &f, sizeof(f));
    std::fprintf(g_out, "\t%08x", bits);
}

void PutMatrix(const D3DXMATRIX &m)
{
    const float *f = &m._11;
    for (int i = 0; i < 16; ++i)
        PutF(f[i]);
}

uint64_t Fnv(const void *data, size_t size)
{
    uint64_t h = 0xcbf29ce484222325ULL;
    const auto *p = static_cast<const unsigned char *>(data);
    for (size_t i = 0; i < size; ++i)
        h = (h ^ p[i]) * 0x100000001b3ULL;
    return h;
}

struct DumpMap : CMapOutdoor
{
    using CMapOutdoor::m_matSplatAlpha;
    using CMapOutdoor::m_matStaticShadow;
    using CMapOutdoor::m_settings_envDataName;
    using CMapOutdoor::m_TextureSet;
};

struct DumpArea : CArea
{
    using CArea::__Load_LoadAmbience;
    using CArea::__Load_LoadObject;
    using CArea::m_ObjectDataVector;
    using CArea::ObjectDataComp;
};

struct DumpTerrain : CTerrain
{
    using CTerrain::RAW_CountTiles;
    using CTerrainImpl::m_abyAttrMap;
    using CTerrainImpl::m_abyTileMap;
    using CTerrainImpl::m_abyWaterMap;
    using CTerrainImpl::m_acNormalMap;
    using CTerrainImpl::m_awRawHeightMap;
    using CTerrainImpl::m_awShadowMap;
    using CTerrainImpl::m_byNumWater;
    using CTerrainImpl::m_lWaterHeight;
    using CTerrainImpl::m_TerrainSplatPatch;

    bool ImplLoadHeightMap(const char *name)
    {
        return CTerrainImpl::LoadHeightMap(name);
    }
    bool ImplLoadTileMap(const char *name)
    {
        return CTerrainImpl::RAW_LoadTileMap(name);
    }
};

struct DumpPropertyManager : CPropertyManager
{
    using CPropertyManager::m_PropertyByCRCMap;
};

struct DumpMapManager : CMapManager
{
    using CMapManager::__LoadMapInfoVector;
    using CMapManager::m_kVct_kMapInfo;
};

void DumpTextureSet(CTextureSet &set, bool ok)
{
    std::fprintf(g_out, "T\t%d\t", ok ? 1 : 0);
    Put(set.GetFileName());
    std::fprintf(g_out, "\t%lu\n", set.GetTextureCount());
    for (unsigned long i = 0; i < set.GetTextureCount(); ++i)
    {
        TTerrainTexture &t = set.GetTexture(i);
        std::fprintf(g_out, "TT\t%lu\t", i);
        Put(t.stFilename);
        PutF(t.UScale);
        PutF(t.VScale);
        PutF(t.UOffset);
        PutF(t.VOffset);
        std::fprintf(g_out, "\t%d\t%u\t%u", t.bSplat ? 1 : 0, t.Begin, t.End);
        PutMatrix(t.m_matTransform);
        std::fputc('\n', g_out);
    }
}

// The CArea part of CMapOutdoor::LoadArea + CArea::Load without
// __Load_BuildObjectInstances (which creates graphic instances).
void DumpAreaData(const std::string &mapDir, WORD x, WORD y)
{
    unsigned long const ulID = (static_cast<unsigned long>(x) * 1000L) + static_cast<unsigned long>(y);
    char szAreaPathName[64 + 1];
    _snprintf(szAreaPathName, sizeof(szAreaPathName), "%s\\%06u\\", mapDir.c_str(), ulID);

    // Leaked: ~CArea needs the effect manager.
    auto *area = new DumpArea;
    std::string const objectFile = szAreaPathName + std::string("AreaData.txt");
    std::string const ambienceFile = szAreaPathName + std::string("AreaAmbienceData.txt");
    bool const objectOk = area->__Load_LoadObject(objectFile.c_str());
    bool const ambienceOk = area->__Load_LoadAmbience(ambienceFile.c_str());

    auto &v = area->m_ObjectDataVector;
    std::fprintf(g_out, "A\t%d\t%d\t%zu\n", objectOk ? 1 : 0, ambienceOk ? 1 : 0, v.size());
    for (auto const &o : v)
    {
        std::fputs("AO", g_out);
        PutF(o.Position.x);
        PutF(o.Position.y);
        PutF(o.Position.z);
        std::fprintf(g_out, "\t%u", o.dwCRC);
        PutF(o.m_fYaw);
        PutF(o.m_fPitch);
        PutF(o.m_fRoll);
        PutF(o.m_fHeightBias);
        for (BYTE const id : o.abyPortalID)
            std::fprintf(g_out, "\t%u", id);
        std::fprintf(g_out, "\t%u", o.dwRange);
        PutF(o.fMaxVolumeAreaPercentage);
        std::fputc('\n', g_out);
    }

    std::ranges::sort(v, DumpArea::ObjectDataComp());
    std::fputs("AS", g_out);
    for (auto const &o : v)
        std::fprintf(g_out, "\t%u", o.dwCRC);
    std::fputc('\n', g_out);
}

// CMapOutdoor::LoadTerrain, minus shadow/minimap textures, splats and
// patches.
void DumpTerrainData(DumpMap &map, const std::string &mapDir, WORD x, WORD y)
{
    unsigned long const ulID = (static_cast<unsigned long>(x) * 1000L) + static_cast<unsigned long>(y);
    char filename[256];
    sprintf(filename, "%s\\%06u\\AreaProperty.txt", mapDir.c_str(), ulID);

    CTokenVectorMap stTokenVectorMap;
    bool ok = LoadMultipleTextData(filename, stTokenVectorMap) && stTokenVectorMap.contains("scripttype") &&
              stTokenVectorMap.contains("areaname");
    std::string areaName;
    if (ok)
    {
        // [0] of an empty vector is undefined; the data always has one.
        auto const &type = stTokenVectorMap["scripttype"];
        auto const &name = stTokenVectorMap["areaname"];
        ok = !type.empty() && type[0] == "AreaProperty";
        if (!name.empty())
            areaName = name[0];
    }
    std::fprintf(g_out, "TP\t%d\t", ok ? 1 : 0);
    if (ok)
        Put(areaName);
    std::fputc('\n', g_out);
    if (!ok)
        return;

    auto *t = new DumpTerrain;
    t->Clear();
    t->SetMapOutDoor(&map);
    t->SetCoordinate(x, y);
    t->CopySettingFromGlobalSetting();

    char szRawHeightFieldname[64 + 1];
    char szWaterMapName[64 + 1];
    char szAttrMapName[64 + 1];
    char szShadowMapName[64 + 1];
    char szSplatName[64 + 1];
    _snprintf(szRawHeightFieldname, sizeof(szRawHeightFieldname), "%s\\%06u\\height.raw", mapDir.c_str(), ulID);
    _snprintf(szSplatName, sizeof(szSplatName), "%s\\%06u\\tile.raw", mapDir.c_str(), ulID);
    _snprintf(szAttrMapName, sizeof(szAttrMapName), "%s\\%06u\\attr.atr", mapDir.c_str(), ulID);
    _snprintf(szWaterMapName, sizeof(szWaterMapName), "%s\\%06u\\water.wtr", mapDir.c_str(), ulID);
    _snprintf(szShadowMapName, sizeof(szShadowMapName), "%s\\%06u\\shadowmap.raw", mapDir.c_str(), ulID);

    bool const water = t->LoadWaterMap(szWaterMapName);
    // CTerrain::LoadHeightMap always returns true; report the inner result.
    bool const height = t->ImplLoadHeightMap(szRawHeightFieldname);
    t->LoadHeightMap(szRawHeightFieldname);
    bool const attr = t->LoadAttrMap(szAttrMapName);
    // CTerrain::RAW_LoadTileMap = this + RAW_AllocateSplats (memset, count,
    // RAW_GenerateSplat through the device).
    bool const tile = t->ImplLoadTileMap(szSplatName);
    std::memset(&t->m_TerrainSplatPatch, 0, sizeof(t->m_TerrainSplatPatch)); // RAW_DeallocateSplats
    t->RAW_CountTiles();
    bool const shadow = t->LoadShadowMap(szShadowMapName);
    t->SetName(areaName);

    std::fprintf(g_out, "TL\t%d\t%d\t%d\t%d\t%d\n", water, height, attr, tile, shadow);
    std::fprintf(g_out, "TH\t%016llx\t%016llx\t%016llx\t%016llx\t%016llx\t%u\t%016llx\t%016llx\t%016llx\n",
                 static_cast<unsigned long long>(Fnv(t->m_awRawHeightMap, sizeof(t->m_awRawHeightMap))),
                 static_cast<unsigned long long>(Fnv(t->m_acNormalMap, sizeof(t->m_acNormalMap))),
                 static_cast<unsigned long long>(Fnv(t->m_abyAttrMap, sizeof(t->m_abyAttrMap))),
                 static_cast<unsigned long long>(Fnv(t->m_abyTileMap, sizeof(t->m_abyTileMap))),
                 static_cast<unsigned long long>(Fnv(t->m_abyWaterMap, sizeof(t->m_abyWaterMap))), t->m_byNumWater,
                 static_cast<unsigned long long>(Fnv(t->m_lWaterHeight, sizeof(t->m_lWaterHeight))),
                 static_cast<unsigned long long>(Fnv(t->m_awShadowMap, sizeof(t->m_awShadowMap))),
                 static_cast<unsigned long long>(
                     Fnv(t->m_TerrainSplatPatch.PatchTileCount, sizeof(t->m_TerrainSplatPatch.PatchTileCount))));
    for (int i = 0; i < MAXTERRAINTEXTURES; ++i)
        if (t->m_TerrainSplatPatch.TileCount[i])
            std::fprintf(g_out, "TC\t%d\t%u\n", i, t->m_TerrainSplatPatch.TileCount[i]);
    delete t;
}

void DumpMapDir(CEterPackManager &manager, const std::string &name, const std::set<std::string> &files)
{
    std::fputs("M\t", g_out);
    Put(name);
    std::fputc('\n', g_out);

    // Leaked: ~CMapOutdoor tears down subsystems this tool never created.
    auto *map = new DumpMap;
    map->SetName(name);
    bool const propertyOk = map->LoadProperty();
    std::fprintf(g_out, "MP\t%d\t%d\t", propertyOk ? 1 : 0, static_cast<int>(map->GetType()));
    Put(map->GetParentMapName());
    std::fputc('\n', g_out);

    std::string const dir = map->GetMapDataDirectory();
    std::string const settingFile = dir + "\\Setting.txt";
    bool const settingOk = map->LoadSetting(settingFile.c_str());
    short cx = 0;
    short cy = 0;
    map->GetTerrainCount(&cx, &cy);
    DWORD bx = 0;
    DWORD by = 0;
    map->GetBaseXY(&bx, &by);
    std::fprintf(g_out, "S\t%d\t%d\t%ld", settingOk ? 1 : 0, map->m_bSettingTerrainVisible ? 1 : 0,
                 map->GetViewRadius());
    PutF(map->GetHeightScale());
    std::fprintf(g_out, "\t%d\t%d\t%u\t%u\t", cx, cy, bx, by);
    Put(map->m_settings_envDataName);
    std::fputc('\n', g_out);
    if (settingOk)
    {
        std::fputs("SM", g_out);
        PutMatrix(map->m_matSplatAlpha);
        PutMatrix(map->m_matStaticShadow);
        std::fputc('\n', g_out);
    }
    DumpTextureSet(map->m_TextureSet, true);

    // CMapOutdoor::Load's LOCAL_ENVIRONMENT_DATA block.
    {
        std::string const local = dir + "\\" + map->m_settings_envDataName;
        std::string env;
        if (manager.isExist(local.c_str()))
            env = local;
        else
        {
            std::string const root = "d:/ymir work/environment/";
            env = root + map->m_settings_envDataName;
            if (root == env)
                env = root + name.substr(name.size() - 2, 2) + ".msenv";
        }
        std::fputs("E\t", g_out);
        Put(env);
        std::fputc('\n', g_out);
    }

    bool const regenOk = map->LoadMonsterAreaInfo();
    std::fprintf(g_out, "R\t%d\t%u\n", regenOk ? 1 : 0, map->GetMonsterAreaInfoCount());
    for (DWORD i = 0; i < map->GetMonsterAreaInfoCount(); ++i)
    {
        CMonsterAreaInfo *info = nullptr;
        map->GetMonsterAreaInfoFromVectorIndex(i, &info);
        long ox = 0;
        long oy = 0;
        long sx = 0;
        long sy = 0;
        info->GetOrigin(&ox, &oy);
        info->GetSize(&sx, &sy);
        std::fprintf(g_out, "RA\t%d\t%ld\t%ld\t%ld\t%ld\t%ld\t%ld\t%ld\t%ld\t%u\t%u\t%u\t%d\n",
                     static_cast<int>(info->GetMonsterAreaInfoType()), ox, oy, sx, sy, info->GetLeft(),
                     info->GetTop(), info->GetRight(), info->GetBottom(), info->GetMonsterVID(),
                     info->GetMonsterGroupID(), info->GetMonsterCount(), static_cast<int>(info->GetMonsterDir()));
    }

    // Every NNNNNN directory under the data directory.
    std::string prefix = dir + "/";
    std::ranges::transform(prefix, prefix.begin(), [](unsigned char c) { return c == '\\' ? '/' : std::tolower(c); });
    std::set<unsigned long> ids;
    for (auto it = files.lower_bound(prefix); it != files.end() && it->starts_with(prefix); ++it)
    {
        std::string const rest = it->substr(prefix.size());
        if (rest.size() > 7 && rest[6] == '/' &&
            std::all_of(rest.begin(), rest.begin() + 6, [](char c) { return c >= '0' && c <= '9'; }))
            ids.insert(std::stoul(rest.substr(0, 6)));
    }
    for (unsigned long const id : ids)
    {
        auto const x = static_cast<WORD>(id / 1000);
        auto const y = static_cast<WORD>(id % 1000);
        std::fprintf(g_out, "D\t%06lu\n", id);
        DumpAreaData(dir, x, y);
        DumpTerrainData(*map, dir, x, y);
    }
}

std::string BaseName(const std::string &name)
{
    auto const slash = name.find_last_of('/');
    return slash == std::string::npos ? name : name.substr(slash + 1);
}

} // namespace

int main(int argc, char **argv)
{
    if (argc < 4)
    {
        std::fprintf(stderr, "usage: %s <client dir> <index file> <props|maps|textures|atlas>\n", argv[0]);
        return 2;
    }

    // TraceError writes to stdout; keep it for the dump only.
    std::fflush(stdout);
    g_out = fdopen(dup(STDOUT_FILENO), "w");
    dup2(STDERR_FILENO, STDOUT_FILENO);

    std::filesystem::current_path(argv[1]);
    std::string const mode = argv[3];

    CEterPackManager manager;
    manager.SetSearchMode(true);
    if (!RegisterIndex(manager, "pack", argv[2]))
        return 1;

    // Singletons the loaders reach for. No image types are registered, so
    // texture lookups come back empty instead of touching a device.
    CTimer timer;
    CResourceManager resources;
    DumpPropertyManager properties;
    properties.Initialize();

    std::set<std::string> files;
    for (auto const &entry : manager.GetFileMap())
        files.insert(entry.first);

    if (mode == "props")
    {
        for (auto const &[crc, property] : properties.m_PropertyByCRCMap)
            std::fprintf(g_out, "P\t%u\n", crc);
    }
    else if (mode == "maps")
    {
        std::set<std::string> dirs;
        for (auto const &name : files)
        {
            std::string const base = BaseName(name);
            if ((base == "setting.txt" || base == "mapproperty.txt") && name.size() > base.size())
                dirs.insert(name.substr(0, name.size() - base.size() - 1));
        }
        for (auto const &dir : dirs)
            DumpMapDir(manager, dir, files);
    }
    else if (mode == "textures")
    {
        for (auto const &name : files)
        {
            if (!name.starts_with("textureset/") || !name.ends_with(".txt"))
                continue;
            CTextureSet set;
            bool const ok = set.Load(name.c_str(), 1.0f / static_cast<float>(CTerrainImpl::PATCH_XSIZE *
                                                                              CTerrainImpl::CELLSCALE));
            DumpTextureSet(set, ok);
        }
    }
    else if (mode == "atlas")
    {
        for (auto const &name : files)
        {
            if (BaseName(name) != "atlasinfo.txt")
                continue;
            std::fputs("F\t", g_out);
            Put(name);
            std::fputc('\n', g_out);
            DumpMapManager mapManager;
            mapManager.SetAtlasInfoFileName(name.c_str());
            mapManager.m_kVct_kMapInfo.clear();
            mapManager.__LoadMapInfoVector();
            for (auto const &info : mapManager.m_kVct_kMapInfo)
            {
                std::fputs("I\t", g_out);
                Put(info.m_strName);
                std::fprintf(g_out, "\t%u\t%u\t%u\t%u\t%u\t%u\n", info.m_dwBaseX, info.m_dwBaseY, info.m_dwSizeX,
                             info.m_dwSizeY, info.m_dwEndX, info.m_dwEndY);
            }
        }
    }
    std::fclose(g_out);
    // Skip destructors of the singletons and leaked maps.
    std::_Exit(0);
}
