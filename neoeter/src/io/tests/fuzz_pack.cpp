#include <Neoeter/IO/EterPackManager.h>
#include <Neoeter/IO/ZIP.h>

#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <filesystem>
#include <fstream>
#include <memory>
#include <string>

namespace fs = std::filesystem;

namespace
{

std::unique_ptr<CEterPackManager> g_manager;
fs::path g_zipPath;

} // namespace

extern "C" int LLVMFuzzerInitialize(int *, char ***)
{
    // ZIP asks the singleton to normalise entry names.
    g_manager = std::make_unique<CEterPackManager>();

    const fs::path dir = fs::temp_directory_path() / "openmt2_fuzz_pack";
    fs::remove_all(dir);
    fs::create_directories(dir);
    g_zipPath = dir / "input.zip";
    return 0;
}

extern "C" int LLVMFuzzerTestOneInput(const uint8_t * data, size_t size)
{
    if (size > (2u << 20))
        return 0;

    {
        std::ofstream out(g_zipPath, std::ios::binary | std::ios::trunc);
        if (!out)
            return 0;
        out.write(reinterpret_cast<const char *>(data),
                  static_cast<std::streamsize>(size));
    }

    try
    {
        ZIP zip(g_zipPath.string());
        const std::vector<std::string> files = zip.listFiles();
        const size_t limit = files.size() < 8 ? files.size() : 8;
        for (size_t i = 0; i < limit; ++i)
        {
            std::shared_ptr<std::vector<char>> bytes;
            zip.getFile(files[i], bytes);
        }
    }
    catch (const std::exception &)
    {
        // libzip rejects most random input; rejection is the expected path.
    }

    std::error_code ec;
    fs::remove(g_zipPath, ec);
    return 0;
}
