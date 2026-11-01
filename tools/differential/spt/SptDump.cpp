// Reference dump for the Rust SpeedTree port (crates/m2-formats, spt module).
//
//   m2diff_spt pack <client dir> [index file, default pack/Index.dev]
//   m2diff_spt files <base dir> <spt file>...
//
// Loads every .spt (in the packs, sorted by name, or the given files) through
// CSpeedTreeRT the way CSpeedTreeWrapper::LoadTree does, under a few seeds,
// positions, sizes and cameras, and prints everything GetGeometry,
// GetTextures, GetBoundingBox and GetCollisionObject report. Floats are
// printed as the hex of their bits. The working directory is the client /
// base dir, so LoadBakedGeometry finds caches under treecache/ there.

#include <Neoeter/IO/EterPackManager.h>
#include <Neoeter/Model/SpeedTree/SpeedTreeRTCompat.h>
#include <Neoeter/Platform/Base/FileLoader.h>

#include <algorithm>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <memory>
#include <string>
#include <vector>

namespace
{

struct Variant
{
    const char *name;
    unsigned int seed;
    float position[3];
    float size; // < 0: no SetTreeSize
    float camera[3];
};

// Same table as crates/m2-formats/src/bin/m2-spt-dump.rs.
const Variant kVariants[] = {
    {"engine", 1, {0.0f, 0.0f, 0.0f}, -1.0f, {0.0f, 0.0f, 0.0f}},
    {"seed", 12345, {37.5f, 81.25f, 3.0f}, -1.0f, {500.0f, -300.0f, 200.0f}},
    {"negative", 0xFFFFFFFFu, {-123.4f, -5.5f, 0.0f}, -1.0f, {-123.4f, -5.5f, 100.0f}},
    {"size1000", 2, {0.0f, 0.0f, 0.0f}, 1000.0f, {0.0f, 0.0f, 0.0f}},
    {"sizehalf", 3, {0.0f, 0.0f, 0.0f}, 0.5f, {1.0f, 2.0f, 3.0f}},
};

const float kInstancePosition[3] = {1000.0f, 2000.0f, 30.0f};
const float kInstanceCamera[3] = {10.0f, 20.0f, 1500.0f};
const float kCameraDir[3] = {0.0f, 0.0f, -1.0f};

std::string Hex(const float *values, size_t count)
{
    std::string out;
    char buf[16];
    for (size_t i = 0; i < count; ++i)
    {
        uint32_t bits = 0;
        std::memcpy(&bits, &values[i], sizeof(bits));
        std::snprintf(buf, sizeof(buf), "%s%08x", i ? " " : "", bits);
        out += buf;
    }
    return out;
}

std::string Hex1(float value)
{
    return Hex(&value, 1);
}

void DumpIndexed(const char *label, const CSpeedTreeRT::SGeometry::SIndexed &part, float alpha)
{
    unsigned int const len = part.m_usNumStrips > 0 ? part.m_pStripLengths[0] : 0;
    std::printf("%s\t%u\t%d\t%u\t%s\t%u\n", label, part.m_usVertexCount, part.m_nDiscreteLodLevel, part.m_usNumStrips,
                Hex1(alpha).c_str(), len);
    for (unsigned int i = 0; i < part.m_usVertexCount; ++i)
        std::printf("v\t%u\t%s\t%08lx\t%s\t%s\n", i, Hex(&part.m_pCoords[i * 3], 3).c_str(), part.m_pColors[i],
                    Hex(&part.m_pTexCoords0[i * 2], 2).c_str(), Hex(&part.m_pTexCoords1[i * 2], 2).c_str());
    if (part.m_usNumStrips > 0)
    {
        std::printf("strip\t");
        for (unsigned int i = 0; i < len; ++i)
            std::printf("%s%u", i ? " " : "", part.m_pStrips[0][i]);
        std::printf("\n");
    }
}

void DumpTree(CSpeedTreeRT &tree, const Variant &v)
{
    std::printf("variant\t%s\n", v.name);

    float size = 0.0f;
    float variance = 0.0f;
    tree.GetTreeSize(size, variance);
    float const sizes[2] = {size, variance};
    std::printf("size\t%s\n", Hex(sizes, 2).c_str());

    CSpeedTreeRT::STextures tex;
    tree.GetTextures(tex);
    std::printf("texture\tbranch\t%s\n", tex.m_pBranchTextureFilename ? tex.m_pBranchTextureFilename : "-");
    std::printf("texture\tcomposite\t%s\n", tex.m_pCompositeFilename ? tex.m_pCompositeFilename : "-");
    std::printf("texture\tshadow\t%s\n", tex.m_pSelfShadowFilename ? tex.m_pSelfShadowFilename : "-");
    std::printf("leaf_textures\t%u\n", tex.m_uiLeafTextureCount);
    for (unsigned int i = 0; i < tex.m_uiLeafTextureCount; ++i)
        std::printf("leaf_texture\t%s\n", tex.m_pLeafTextureFilenames[i]);
    std::printf("frond_textures\t%u\n", tex.m_uiFrondTextureCount);
    for (unsigned int i = 0; i < tex.m_uiFrondTextureCount; ++i)
        std::printf("frond_texture\t%s\n", tex.m_pFrondTextureFilenames[i]);

    float bbox[6];
    tree.GetBoundingBox(bbox);
    std::printf("bbox\t%s\n", Hex(bbox, 6).c_str());
    std::printf("material\tbranch\t%s\n", Hex(tree.GetBranchMaterial(), 13).c_str());
    std::printf("material\tfrond\t%s\n", Hex(tree.GetFrondMaterial(), 13).c_str());
    std::printf("material\tleaf\t%s\n", Hex(tree.GetLeafMaterial(), 13).c_str());

    unsigned int const nColl = tree.GetCollisionObjectCount();
    std::printf("collisions\t%u\n", nColl);
    for (unsigned int i = 0; i < nColl; ++i)
    {
        CSpeedTreeRT::ECollisionObjectType type{};
        float pos[3];
        float dims[3];
        tree.GetCollisionObject(i, type, pos, dims);
        std::printf("collision\t%u\t%d\t%s\t%s\n", i, static_cast<int>(type), Hex(pos, 3).c_str(),
                    Hex(dims, 3).c_str());
    }

    CSpeedTreeRT::SGeometry geo;
    tree.GetGeometry(geo);
    DumpIndexed("branches", geo.m_sBranches, geo.m_fBranchAlphaTestValue);
    DumpIndexed("fronds", geo.m_sFronds, geo.m_fFrondAlphaTestValue);

    const auto &l = geo.m_sLeaves0;
    std::printf("leaves0\t%d\t%s\t%d\t%u\n", l.m_bIsActive ? 1 : 0, Hex1(l.m_fAlphaTestValue).c_str(),
                l.m_nDiscreteLodLevel, l.m_usLeafCount);
    for (unsigned int i = 0; i < l.m_usLeafCount; ++i)
        std::printf("leaf\t%u\t%s\t%08lx\t%u\t%u\t%s\t%s\n", i, Hex(&l.m_pCenterCoords[i * 3], 3).c_str(),
                    l.m_pColors[i], l.m_pLeafMapIndices[i], l.m_pLeafClusterIndices[i],
                    Hex(l.m_pLeafMapTexCoords[i], 8).c_str(), Hex(l.m_pLeafMapCoords[i], 16).c_str());
    std::printf("leaves1\t%d\t%u\t%d\n", geo.m_sLeaves1.m_bIsActive ? 1 : 0, geo.m_sLeaves1.m_usLeafCount,
                geo.m_sLeaves1.m_nDiscreteLodLevel);
    std::printf("billboards\t%d\t%d\t%d\n", geo.m_sBillboard0.m_bIsActive ? 1 : 0,
                geo.m_sBillboard1.m_bIsActive ? 1 : 0, geo.m_sHorizontalBillboard.m_bIsActive ? 1 : 0);
    std::printf("triangles\t%u\t%u\t%u\n", tree.GetBranchTriangleCount(), tree.GetLeafTriangleCount(),
                tree.GetFrondTriangleCount());
}

void DumpFile(const std::string &name, const std::vector<char> &data)
{
    std::printf("file\t%s\t%zu\n", name.c_str(), data.size());

    std::unique_ptr<CSpeedTreeRT> engineTree;
    for (const Variant &v : kVariants)
    {
        auto tree = std::make_unique<CSpeedTreeRT>();
        CSpeedTreeRT::SetCamera(v.camera, kCameraDir);
        // CSpeedTreeWrapper::LoadTree's order.
        if (!tree->LoadTree(reinterpret_cast<const unsigned char *>(data.data()),
                            static_cast<unsigned int>(data.size())))
        {
            std::printf("variant\t%s\tload failed\n", v.name);
            continue;
        }
        tree->SetBranchLightingMethod(CSpeedTreeRT::LIGHT_STATIC);
        tree->SetLeafLightingMethod(CSpeedTreeRT::LIGHT_STATIC);
        tree->SetFrondLightingMethod(CSpeedTreeRT::LIGHT_STATIC);
        tree->SetNumLeafRockingGroups(1);
        if (v.size >= 0.0f)
            tree->SetTreeSize(v.size, 0.0f);
        tree->SetTreePosition(v.position[0], v.position[1], v.position[2]);
        if (!tree->Compute(nullptr, v.seed))
        {
            std::printf("variant\t%s\tload failed\n", v.name);
            continue;
        }
        DumpTree(*tree, v);
        if (!engineTree)
            engineTree = std::move(tree);
    }

    if (engineTree)
    {
        std::unique_ptr<CSpeedTreeRT> instance(engineTree->MakeInstance());
        instance->SetTreePosition(kInstancePosition[0], kInstancePosition[1], kInstancePosition[2]);
        CSpeedTreeRT::SetCamera(kInstanceCamera, kCameraDir);
        CSpeedTreeRT::SGeometry geo;
        instance->GetGeometry(geo, SpeedTree_LeafGeometry);
        for (unsigned int i = 0; i < geo.m_sLeaves0.m_usLeafCount; ++i)
            std::printf("instance_leaf\t%u\t%s\n", i, Hex(geo.m_sLeaves0.m_pLeafMapCoords[i], 16).c_str());
    }
}

// Mirrors PackInitialize's registration loop (see tools/differential/pack).
bool RegisterIndex(CEterPackManager &manager, const std::string &indexFile)
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

    CTokenVector tokens;
    for (DWORD i = 0; i < loader.GetLineCount() - 1; i++)
    {
        loader.SplitLineByTab(i, &tokens);
        if (tokens.size() != 2)
        {
            std::fprintf(stderr, "line %u: expected 2 tokens, got %zu\n", static_cast<unsigned>(i), tokens.size());
            return false;
        }
        std::string const name = "pack/" + tokens[0];
        manager.RegisterPack(name, tokens[1]);
        std::string const texCache = name + "_texcache";
        if (manager.PackExists(texCache, tokens[1]))
            manager.RegisterPack(texCache, tokens[1]);
    }
    return true;
}

bool EndsWith(const std::string &s, const char *suffix)
{
    size_t const n = std::strlen(suffix);
    return s.size() >= n && s.compare(s.size() - n, n, suffix) == 0;
}

} // namespace

int main(int argc, char **argv)
{
    if (argc < 3)
    {
        std::fprintf(stderr, "usage: %s pack <client dir> [index] | %s files <base dir> <spt>...\n", argv[0],
                     argv[0]);
        return 2;
    }
    std::string const mode = argv[1];
    std::filesystem::current_path(argv[2]);

    if (mode == "pack")
    {
        std::string const indexFile = argc > 3 ? argv[3] : "pack/Index.dev";
        CEterPackManager manager;
        manager.SetSearchMode(true);
        if (!RegisterIndex(manager, indexFile))
            return 1;

        std::vector<std::string> names;
        for (auto const &entry : manager.GetFileMap())
            if (EndsWith(entry.first, ".spt"))
                names.push_back(entry.first);
        std::ranges::sort(names);

        for (auto const &name : names)
        {
            CEterPackManager::TPackDataPtr data;
            if (manager.Get(name, data) && data)
                DumpFile(name, *data);
            else
                std::printf("file\t%s\t-\n", name.c_str());
        }
        return 0;
    }

    if (mode == "files")
    {
        for (int i = 3; i < argc; ++i)
        {
            std::ifstream in(argv[i], std::ios::binary);
            if (!in)
            {
                std::fprintf(stderr, "cannot open %s\n", argv[i]);
                return 1;
            }
            std::vector<char> data((std::istreambuf_iterator<char>(in)), std::istreambuf_iterator<char>());
            DumpFile(argv[i], data);
        }
        return 0;
    }

    std::fprintf(stderr, "unknown mode %s\n", mode.c_str());
    return 2;
}
