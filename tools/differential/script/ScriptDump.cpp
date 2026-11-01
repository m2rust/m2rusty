// Reference dump for the Rust grouped-text reader (crates/m2-formats/src/script).
//
//   m2diff_script <client dir> <index file> <tree|multi> <ext[,ext...]>
//
// Registers packs like m2diff_pack, then for every registered file whose
// extension is in the list (sorted by name):
//
//   tree:  loads it with CTextFileLoader::Load and prints
//            F <name> <ok>
//            G <depth> <group name>                  per node, depth first
//            K <depth> <crc32 of key %08x> <count>   per key, in map (CRC) order
//            T <token> <atoi> <atof as double bits %016llx> <(float)atof bits %08x>
//   multi: loads it with LoadMultipleTextData and prints
//            F <name> <ok>
//            K <key> <count>   per key, in map order
//            T <token>
//
// Fields are tab separated; names and tokens are raw bytes. Files that make
// CMemoryTextFileLoader::Bind read past their end (undefined, so not
// comparable) print only "F <name> skip".

#include <Neoeter/IO/EterPackManager.h>
#include <Neoeter/Platform/Base/FileLoader.h>
#include <Neoeter/Render/TextFileLoader.h>
#include <Neoeter/Render/Util.h>

#include <algorithm>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <sstream>
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

void PutToken(const std::string &token, bool numbers)
{
    std::fputs("T\t", g_out);
    Put(token);
    if (numbers)
    {
        int const i = atoi(token.c_str());
        double const d = atof(token.c_str());
        float const f = static_cast<float>(d);
        uint64_t dbits = 0;
        uint32_t fbits = 0;
        std::memcpy(&dbits, &d, sizeof(d));
        std::memcpy(&fbits, &f, sizeof(f));
        std::fprintf(g_out, "\t%d\t%016llx\t%08x", i, static_cast<unsigned long long>(dbits), fbits);
    }
    std::fputc('\n', g_out);
}

struct TreeLoader : CTextFileLoader
{
    TGroupNode &Root()
    {
        return m_GlobalNode;
    }
};

void DumpNode(CTextFileLoader::TGroupNode &node, int depth)
{
    std::fprintf(g_out, "G\t%d\t", depth);
    Put(node.m_strGroupName);
    std::fputc('\n', g_out);
    for (auto const &[key, tokens] : node.m_kMap_dwKey_kVct_stToken)
    {
        std::fprintf(g_out, "K\t%d\t%08x\t%zu\n", depth, static_cast<unsigned>(key), tokens.size());
        for (auto const &token : tokens)
            PutToken(token, true);
    }
    for (auto *child : node.ChildNodeVector)
        DumpNode(*child, depth + 1);
}

void DumpTree(const std::string &name)
{
    TreeLoader loader;
    bool const ok = loader.Load(name.c_str());
    std::fputs("F\t", g_out);
    Put(name);
    std::fprintf(g_out, "\t%d\n", ok ? 1 : 0);
    DumpNode(loader.Root(), 0);
}

void DumpMulti(const std::string &name)
{
    CTokenVectorMap map;
    bool const ok = LoadMultipleTextData(name.c_str(), map);
    std::fputs("F\t", g_out);
    Put(name);
    std::fprintf(g_out, "\t%d\n", ok ? 1 : 0);
    for (auto const &[key, tokens] : map)
    {
        std::fputs("K\t", g_out);
        Put(key);
        std::fprintf(g_out, "\t%zu\n", tokens.size());
        for (auto const &token : tokens)
            PutToken(token, false);
    }
}

// Whether CMemoryTextFileLoader::Bind reads past the end: a byte >= 0x80
// starts a two-byte character, and when it is the last byte Bind copies the
// byte after the buffer. Those files are skipped (the Rust port clamps).
bool BindOverreads(CEterPackManager &manager, const std::string &name)
{
    CEterPackManager::TPackDataPtr data;
    if (!manager.Get(name, data) || !data)
        return false;
    size_t pos = 0;
    while (pos < data->size())
    {
        char const c = (*data)[pos++];
        if (c < 0)
        {
            if (pos == data->size())
                return true;
            ++pos;
        }
    }
    return false;
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

} // namespace

int main(int argc, char **argv)
{
    if (argc < 5)
    {
        std::fprintf(stderr, "usage: %s <client dir> <index file> <tree|multi> <ext[,ext...]>\n", argv[0]);
        return 2;
    }

    std::fflush(stdout);
    g_out = fdopen(dup(STDOUT_FILENO), "w");
    dup2(STDERR_FILENO, STDOUT_FILENO);

    std::filesystem::current_path(argv[1]);
    std::string const mode = argv[3];

    std::vector<std::string> exts;
    std::stringstream list(argv[4]);
    for (std::string ext; std::getline(list, ext, ',');)
        exts.push_back(ext);

    CEterPackManager manager;
    manager.SetSearchMode(true);
    if (!RegisterIndex(manager, "pack", argv[2]))
        return 1;

    std::vector<std::string> names;
    for (auto const &entry : manager.GetFileMap())
        if (std::ranges::find(exts, Extension(entry.first)) != exts.end())
            names.push_back(entry.first);
    std::ranges::sort(names);

    for (auto const &name : names)
    {
        if (BindOverreads(manager, name))
        {
            std::fputs("F\t", g_out);
            Put(name);
            std::fputs("\tskip\n", g_out);
        }
        else if (mode == "tree")
            DumpTree(name);
        else
            DumpMulti(name);
    }
    std::fclose(g_out);
    return 0;
}
