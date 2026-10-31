#include <Neoeter/Model/SpeedTree/SpeedTreeRTCompat.h>
#include <Neoeter/Model/SpeedTree/TreeCacheFormat.h>

#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <string>
#include <vector>

namespace fs = std::filesystem;

namespace
{

fs::path g_cacheDir;
std::string g_cachePath;
std::vector<unsigned char> g_spt;

} // namespace

extern "C" int LLVMFuzzerInitialize(int *, char ***)
{
    g_cacheDir = fs::temp_directory_path() / "openmt2_fuzz_treecache";
    fs::remove_all(g_cacheDir);
    fs::create_directories(g_cacheDir);

    const std::string spt = "__IdvSpt_02_FuzzBark.tga";
    g_spt.assign(spt.begin(), spt.end());
    const uint32_t hash = TreeCache::Fnv1a32(g_spt.data(), g_spt.size());

    char name[TreeCache::kHashHexDigits + 1];
    TreeCache::FormatHash(hash, name);
    g_cachePath = (g_cacheDir / (std::string(name) + ".tree")).string();

#ifdef _WIN32
    _putenv_s("MT2_TREECACHE", g_cacheDir.string().c_str());
#else
    setenv("MT2_TREECACHE", g_cacheDir.string().c_str(), 1);
#endif
    return 0;
}

extern "C" int LLVMFuzzerTestOneInput(const uint8_t * data, size_t size)
{
    if (size > (1u << 20))
        return 0;

    {
        std::ofstream out(g_cachePath, std::ios::binary | std::ios::trunc);
        if (!out)
            return 0;
        out.write(reinterpret_cast<const char *>(data),
                  static_cast<std::streamsize>(size));
    }

    CSpeedTreeRT tree;
    tree.LoadTree(g_spt.data(), static_cast<unsigned int>(g_spt.size()));

    std::error_code ec;
    fs::remove(g_cachePath, ec);
    return 0;
}
