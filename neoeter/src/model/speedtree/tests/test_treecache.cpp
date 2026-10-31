#include <support/TestAssert.h>
#include <support/TreeCacheFixture.h>

#include <Neoeter/Model/SpeedTree/SpeedTreeRTCompat.h>

#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <string>
#include <vector>

namespace fs = std::filesystem;

namespace
{

std::vector<unsigned char> MakeSpt(const char * barkName)
{
    std::string spt = "__IdvSpt_02_";
    spt += barkName;
    spt.push_back('\0');
    spt += "SyntheticLeaf.tga";
    spt.push_back('\0');
    return std::vector<unsigned char>(spt.begin(), spt.end());
}

void SetTreeCacheEnv(const std::string & path)
{
#ifdef _WIN32
    _putenv_s("MT2_TREECACHE", path.c_str());
#else
    setenv("MT2_TREECACHE", path.c_str(), 1);
#endif
}

void ClearTreeCacheEnv()
{
#ifdef _WIN32
    _putenv_s("MT2_TREECACHE", "");
#else
    unsetenv("MT2_TREECACHE");
#endif
}

std::string CachePath(const fs::path & dir, uint32_t hash)
{
    char name[TreeCache::kHashHexDigits + 1];
    TreeCache::FormatHash(hash, name);
    return (dir / (std::string(name) + ".tree")).string();
}

bool WriteFile(const std::string & path, const std::vector<unsigned char> & data)
{
    std::ofstream out(path, std::ios::binary);
    if (!out)
        return false;
    if (!data.empty())
        out.write(reinterpret_cast<const char *>(data.data()),
                  static_cast<std::streamsize>(data.size()));
    return static_cast<bool>(out);
}

fs::path MakeSandbox()
{
    fs::path root = fs::temp_directory_path() / "openmt2_treecache";
    fs::remove_all(root);
    fs::create_directories(root);
    return root;
}

void test_valid_cache_round_trip()
{
    TEST_SECTION("treecache round-trip");

    const fs::path root = MakeSandbox();
    const std::vector<unsigned char> spt = MakeSpt("SyntheticBark.dds");
    const uint32_t hash = TreeCache::Fnv1a32(spt.data(), spt.size());
    const std::string path = CachePath(root, hash);
    TEST_ASSERT(WriteFile(path, TreeCacheFixture::MakeBranchesOnly(hash, 123.5f)));

    SetTreeCacheEnv(root.string());

    CSpeedTreeRT tree;
    TEST_ASSERT(tree.LoadTree(spt.data(), static_cast<unsigned int>(spt.size())));

    float bounds[6] = {0};
    tree.GetBoundingBox(bounds);
    TEST_ASSERT(bounds[0] == -10.0f && bounds[1] == -20.0f && bounds[2] == -30.0f);
    TEST_ASSERT(bounds[3] == 10.0f && bounds[4] == 20.0f && bounds[5] == 123.5f);

    TEST_ASSERT(tree.GetCollisionObjectCount() == 1);
    CSpeedTreeRT::ECollisionObjectType type;
    float position[3] = {0};
    float dimensions[3] = {0};
    tree.GetCollisionObject(0, type, position, dimensions);
    TEST_ASSERT(type == CSpeedTreeRT::CO_CYLINDER);
    TEST_ASSERT(position[0] == 1.0f && position[1] == 2.0f && position[2] == 3.0f);
    TEST_ASSERT(dimensions[0] == 0.5f && dimensions[1] == 61.75f &&
                dimensions[2] == 0.5f);

    CSpeedTreeRT::SGeometry geometry;
    tree.GetGeometry(geometry);
    TEST_ASSERT(geometry.m_sBranches.m_usVertexCount == 4);
    TEST_ASSERT(geometry.m_sBranches.m_usNumStrips == 1);
    TEST_ASSERT(geometry.m_sBranches.m_pStripLengths[0] == 4);
    TEST_ASSERT(geometry.m_sBranches.m_pStrips[0][0] == 0);
    TEST_ASSERT(geometry.m_sBranches.m_pStrips[0][3] == 3);
    TEST_ASSERT(geometry.m_sBranches.m_pCoords[3] == 1.0f); // vertex 1 x
    TEST_ASSERT(geometry.m_sBranches.m_pCoords[7] == 1.0f); // vertex 2 y
    TEST_ASSERT(geometry.m_fBranchAlphaTestValue == 0.25f);
    TEST_ASSERT(geometry.m_sFronds.m_usVertexCount == 0);
    TEST_ASSERT(!geometry.m_sLeaves0.m_bIsActive);
    TEST_ASSERT(!geometry.m_sLeaves1.m_bIsActive);

    ClearTreeCacheEnv();
    fs::remove_all(root);
    TEST_PASS("header, collision, branch strip and leaf flags");
}

void test_rejected_cache_falls_back()
{
    TEST_SECTION("treecache rejection");

    const fs::path root = MakeSandbox();
    const std::vector<unsigned char> spt = MakeSpt("RejectBark.dds");
    const uint32_t hash = TreeCache::Fnv1a32(spt.data(), spt.size());
    const std::string path = CachePath(root, hash);

    std::vector<unsigned char> valid = TreeCacheFixture::MakeBranchesOnly(hash, 123.5f);

    auto expect_fallback = [&](const std::vector<unsigned char> & cache, const char * label)
    {
        TEST_ASSERT(WriteFile(path, cache));
        CSpeedTreeRT tree;
        TEST_ASSERT(tree.LoadTree(spt.data(), static_cast<unsigned int>(spt.size())));
        float bounds[6] = {0};
        tree.GetBoundingBox(bounds);
        if (bounds[3] == 10.0f && bounds[5] == 123.5f)
        {
            std::printf("    %s: cache was accepted (bounds %.1f %.1f %.1f %.1f %.1f %.1f)\n",
                        label, bounds[0], bounds[1], bounds[2], bounds[3], bounds[4], bounds[5]);
            TEST_ASSERT(false);
        }
        TEST_PASS(label);
    };

    SetTreeCacheEnv(root.string());

    std::vector<unsigned char> badMagic = valid;
    badMagic[0] ^= 0xFF;
    expect_fallback(badMagic, "bad magic");

    std::vector<unsigned char> badHash = valid;
    badHash[4] ^= 0xFF;
    expect_fallback(badHash, "wrong hash");

    for (size_t size : {size_t(4), size_t(20), size_t(44), size_t(100),
                        valid.size() / 2})
    {
        std::vector<unsigned char> truncated(valid.begin(), valid.begin() + size);
        expect_fallback(truncated, "truncated cache");
    }

    expect_fallback(TreeCacheFixture::MakeHugeLeafCount(hash), "huge leaf count");

    ClearTreeCacheEnv();
    fs::remove_all(root);
}

} // namespace

int main()
{
    test_valid_cache_round_trip();
    test_rejected_cache_falls_back();

    std::printf("[test_treecache] all checks passed\n");
    return 0;
}
