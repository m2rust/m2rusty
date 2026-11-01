// Reference dump for the Rust pack reader (crates/m2-formats).
//
//   m2diff_pack <client dir> [index file, default pack/Index]
//
// Registers packs the way client/src/ui/UserInterface.cpp PackInitialize
// does (archive-first), then prints one line per file in the manager's file
// map, sorted by name: name<TAB>size<TAB>fnv1a64 of the contents. Files that
// fail to read print "-" for size and hash.

#include <Neoeter/IO/EterPackManager.h>
#include <Neoeter/Platform/Base/FileLoader.h>

#include <algorithm>
#include <cstdint>
#include <cstdio>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <string>
#include <vector>

namespace
{

uint64_t Fnv1a64(const std::vector<char> &data)
{
    uint64_t hash = 0xcbf29ce484222325ull;
    for (char const c : data)
    {
        hash ^= static_cast<unsigned char>(c);
        hash *= 0x100000001b3ull;
    }
    return hash;
}

// Mirrors PackInitialize's registration loop.
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
        {
            std::fprintf(stderr, "line %u: expected 2 tokens, got %zu\n", static_cast<unsigned>(i), tokens.size());
            return false;
        }

        std::string const name = folder + tokens[0];
        manager.RegisterPack(name, tokens[1]);

        std::string const texCache = name + "_texcache";
        if (manager.PackExists(texCache, tokens[1]))
            manager.RegisterPack(texCache, tokens[1]);
    }
    return true;
}

} // namespace

int main(int argc, char **argv)
{
    if (argc < 2)
    {
        std::fprintf(stderr, "usage: %s <client dir> [index file]\n", argv[0]);
        return 2;
    }

    // Pack names and direct-file lookups are relative to the client dir.
    std::filesystem::current_path(argv[1]);
    std::string const indexFile = argc > 2 ? argv[2] : "pack/Index";

    CEterPackManager manager;
    manager.SetSearchMode(true);
    if (!RegisterIndex(manager, "pack", indexFile))
        return 1;

    std::vector<std::string> names;
    names.reserve(manager.GetFileMap().size());
    for (auto const &entry : manager.GetFileMap())
        names.push_back(entry.first);
    std::ranges::sort(names);

    for (auto const &name : names)
    {
        CEterPackManager::TPackDataPtr data;
        if (manager.Get(name, data) && data)
            std::printf("%s\t%zu\t%016llx\n", name.c_str(), data->size(),
                        static_cast<unsigned long long>(Fnv1a64(*data)));
        else
            std::printf("%s\t-\t-\n", name.c_str());
    }
    return 0;
}
