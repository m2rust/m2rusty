// Reference dump for the Rust image decoders (crates/m2-formats/src/image).
//
//   m2diff_image [--full] pack <client dir> <index file>
//   m2diff_image [--full] files <path>...
//
// "pack" registers packs like m2diff_pack and dumps every file whose virtual
// name ends in .dds .tga .jpg .jpeg .png .bmp, sorted by name; "files" dumps
// plain files in argument order. Per file:
//
//   file <name> <size>
//   info <w> <h> <d3dfmt> | info -                Platform_GetImageInfo
//   decode <w> <h> <fnv1a64 of BGRA> | decode -   Platform_DecodeImage
//   tga ...      (.tga only)  CTGAImage::LoadFromMemory
//   dds ...      (.dds only)  CDXTCImage header, Copy and Decompress per level
//
// With --full the decoded pixels follow as hex lines ("px ...").

#include <Neoeter/Image/DXTCImage.h>
#include <Neoeter/Image/TGAImage.h>
#include <Neoeter/IO/EterPackManager.h>
#include <Neoeter/Platform/Base/FileLoader.h>
#include <Neoeter/Platform/Base/ImageLoaderCompat.h>

#include <fcntl.h>
#include <unistd.h>

#include <algorithm>
#include <cctype>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <string>
#include <vector>

namespace
{

bool g_full = false;

uint64_t Fnv1a64(const void *data, size_t size)
{
    const auto *p = static_cast<const unsigned char *>(data);
    uint64_t hash = 0xcbf29ce484222325ull;
    for (size_t i = 0; i < size; ++i)
    {
        hash ^= p[i];
        hash *= 0x100000001b3ull;
    }
    return hash;
}

void PutHex(const void *data, size_t size)
{
    const auto *p = static_cast<const unsigned char *>(data);
    std::printf("px ");
    for (size_t i = 0; i < size; ++i)
        std::printf("%02x", p[i]);
    std::printf("\n");
}

bool EndsWith(const std::string &s, const char *suffix)
{
    size_t const n = std::strlen(suffix);
    if (s.size() < n)
        return false;
    for (size_t i = 0; i < n; ++i)
        if (std::tolower(static_cast<unsigned char>(s[s.size() - n + i])) != suffix[i])
            return false;
    return true;
}

void DumpTga(const std::vector<unsigned char> &bytes)
{
    if (bytes.size() < 18)
    {
        std::printf("tga fail\n");
        return;
    }
    TGA_HEADER h;
    std::memcpy(&h, bytes.data(), 18);
    bool const supported = (h.imgType == 3) ||
                           (h.imgType == 2 && (h.colorBits == 16 || h.colorBits == 24 || h.colorBits == 32)) ||
                           (h.imgType == 10 && (h.colorBits == 24 || h.colorBits == 32));
    if (!supported)
    {
        std::printf("tga unsupported\n"); // C++ leaves the pixels uninitialised
        return;
    }
    // The C++ loader never checks the input length. Pad with zeros so a
    // truncated file reads zeros instead of crashing (Rust fails such files).
    uint64_t const need = static_cast<uint64_t>(h.width) * h.height * 4;
    if (need > (64u << 20))
    {
        std::printf("tga big\n");
        return;
    }
    std::vector<unsigned char> padded(bytes);
    padded.resize(bytes.size() + need + 1024, 0);

    CTGAImage img;
    // The loader prints "RLE overflow" on stdout; keep it out of the dump.
    std::fflush(stdout);
    int const saved = dup(1);
    int const devnull = open("/dev/null", O_WRONLY);
    dup2(devnull, 1);
    bool const loaded = img.LoadFromMemory(static_cast<int>(bytes.size()), padded.data());
    std::fflush(stdout);
    dup2(saved, 1);
    close(saved);
    close(devnull);
    if (!loaded)
    {
        std::printf("tga fail\n");
        return;
    }
    size_t const n = static_cast<size_t>(img.GetWidth()) * img.GetHeight() * 4;
    std::printf("tga %d %d %d %d %d %016llx\n", img.GetWidth(), img.GetHeight(), h.imgType, h.colorBits, h.desc,
                static_cast<unsigned long long>(Fnv1a64(img.GetBasePointer(), n)));
    if (g_full)
        PutHex(img.GetBasePointer(), n);
}

void DumpDds(const std::vector<unsigned char> &bytes)
{
    CDXTCImage dxt;
    if (!dxt.LoadHeaderFromMemory(bytes.data(), static_cast<int>(bytes.size())))
    {
        std::printf("dds fail\n");
        return;
    }
    std::printf("dds %s %d %d %u %d %08x %ld %d\n", dxt.m_strFormat, dxt.m_nWidth, dxt.m_nHeight,
                static_cast<unsigned>(dxt.m_dwMipMapCount), dxt.m_bMipTexture ? 1 : 0,
                static_cast<unsigned>(dxt.m_dwFlags), dxt.m_lPitch, static_cast<int>(dxt.m_CompFormat));

    for (DWORD i = 0; i < dxt.m_dwMipMapCount; ++i)
    {
        const BYTE *p = dxt.m_pbCompBufferByLevels[i];
        std::printf("level %u", static_cast<unsigned>(i));
        if (!p)
        {
            std::printf(" off=- copy=- pix=-\n");
            continue;
        }
        // Pointers may point past the end of a truncated file; stay with offsets.
        int64_t const start = p - bytes.data();
        std::printf(" off=%lld", static_cast<long long>(start));

        if (!(dxt.m_dwFlags & 0x00020000) && i != 0)
            std::printf(" copy=none");
        else
        {
            int64_t const len = dxt.m_lPitch >> (i * 2);
            if (len < 0 || start + len > static_cast<int64_t>(bytes.size()))
                std::printf(" copy=oob");
            else
            {
                std::vector<BYTE> dst(static_cast<size_t>(len) + 1);
                dxt.Copy(static_cast<int>(i), dst.data(), 0);
                std::printf(" copy=%016llx", static_cast<unsigned long long>(Fnv1a64(dst.data(), len)));
            }
        }

        UINT const w = dxt.m_nWidth >> i;
        UINT const h = dxt.m_nHeight >> i;
        if (dxt.m_nWidth < 0 || dxt.m_nHeight < 0 || static_cast<uint64_t>(w) * h > (1u << 26))
            std::printf(" pix=big\n");
        else
        {
            size_t const need = static_cast<size_t>(w / 4) * (h / 4) * (dxt.m_CompFormat == PF_DXT1 ? 8 : 16);
            if (start + static_cast<int64_t>(need) > static_cast<int64_t>(bytes.size()))
                std::printf(" pix=oob\n");
            else
            {
                dxt.m_bCompVector[i].assign(p, p + need);
                std::vector<DWORD> out(static_cast<size_t>(w) * h, 0);
                dxt.Decompress(static_cast<int>(i), out.data());
                std::printf(" pix=%016llx\n", static_cast<unsigned long long>(Fnv1a64(out.data(), out.size() * 4)));
                if (g_full)
                    PutHex(out.data(), out.size() * 4);
            }
        }
    }
}

void Dump(const std::string &name, const std::vector<unsigned char> &bytes)
{
    std::printf("file %s %zu\n", name.c_str(), bytes.size());

    uint32_t w = 0, h = 0, fmt = 0;
    if (Platform_GetImageInfo(bytes.data(), bytes.size(), &w, &h, &fmt))
        std::printf("info %u %u %u\n", w, h, fmt);
    else
        std::printf("info -\n");

    std::vector<uint8_t> pixels;
    if (Platform_DecodeImage(bytes.data(), bytes.size(), &w, &h, pixels))
    {
        std::printf("decode %u %u %016llx\n", w, h,
                    static_cast<unsigned long long>(Fnv1a64(pixels.data(), pixels.size())));
        if (g_full)
            PutHex(pixels.data(), pixels.size());
    }
    else
        std::printf("decode -\n");

    if (EndsWith(name, ".tga"))
        DumpTga(bytes);
    if (EndsWith(name, ".dds"))
        DumpDds(bytes);
}

bool WantedExtension(const std::string &name)
{
    for (const char *ext : {".dds", ".tga", ".jpg", ".jpeg", ".png", ".bmp"})
        if (EndsWith(name, ext))
            return true;
    return false;
}

// Same registration loop as tools/differential/pack/PackDump.cpp.
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

int DumpPacks(const char *clientDir, const char *indexFile)
{
    std::filesystem::current_path(clientDir);
    CEterPackManager manager;
    manager.SetSearchMode(true);
    if (!RegisterIndex(manager, "pack", indexFile))
        return 1;

    std::vector<std::string> names;
    for (auto const &entry : manager.GetFileMap())
        if (WantedExtension(entry.first))
            names.push_back(entry.first);
    std::ranges::sort(names);

    for (auto const &name : names)
    {
        CEterPackManager::TPackDataPtr data;
        if (manager.Get(name, data) && data)
            Dump(name, std::vector<unsigned char>(data->begin(), data->end()));
        else
            std::printf("file %s\nunreadable\n", name.c_str());
    }
    return 0;
}

int DumpFiles(int count, char **paths)
{
    for (int i = 0; i < count; ++i)
    {
        std::ifstream in(paths[i], std::ios::binary);
        if (!in)
        {
            std::printf("file %s\nunreadable\n", paths[i]);
            continue;
        }
        std::vector<unsigned char> bytes((std::istreambuf_iterator<char>(in)), std::istreambuf_iterator<char>());
        Dump(paths[i], bytes);
    }
    return 0;
}

} // namespace

int main(int argc, char **argv)
{
    int arg = 1;
    if (arg < argc && std::strcmp(argv[arg], "--full") == 0)
    {
        g_full = true;
        ++arg;
    }
    if (arg + 2 < argc && std::strcmp(argv[arg], "pack") == 0)
        return DumpPacks(argv[arg + 1], argv[arg + 2]);
    if (arg < argc && std::strcmp(argv[arg], "files") == 0)
        return DumpFiles(argc - arg - 1, argv + arg + 1);

    std::fprintf(stderr, "usage: %s [--full] pack <client dir> <index file>\n       %s [--full] files <path>...\n",
                 argv[0], argv[0]);
    return 2;
}
