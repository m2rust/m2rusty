// Reference dump for the Rust locale codecs (crates/m2-formats/src/locale).
//
//   m2diff_locale tables <out dir>
//       Writes the glibc iconv tables the Rust port uses (cp<N>.bin, see
//       crates/m2-formats/src/locale/iconv.rs for the format). Reads iconv
//       directly: on Linux StringCodec.cpp is a loop around iconv, so iconv's
//       tables are the C++ code's tables.
//   m2diff_locale codec <code page>
//       Exhaustive MultiByteToWideChar / WideCharToMultiByte dump: every
//       single byte, every byte pair, combining triples, UTF-8 sequence
//       classes, every BMP code point and a sample of the rest.
//   m2diff_locale vietnamese
//       Ymir_* with CP 1258 (EL_String_*_Vietnamese).
//   m2diff_locale arabic
//       Arabic_* predicates for the BMP and Arabic_MakeShape over short
//       sequences.
//   m2diff_locale japanese
//       ShiftJIS_* over all bytes and short strings.
//   m2diff_locale corpus <code page> <file>...
//       Every line of each file through Ymir_* in that code page, then a
//       per-file hash of the whole file in every supported code page.
//
// Wide strings print as comma-separated hex code units, byte strings as hex,
// "-" when empty.

// StdAfx.h first: Arabic.h relies on it for size_t.
#include <Neoeter/Platform/Locale/StdAfx.h>

#include <Neoeter/Platform/Locale/Arabic.h>
#include <Neoeter/Platform/Locale/Japanese.h>
#include <Neoeter/Platform/Locale/StringCodec.h>

#include <iconv.h>

#include <cerrno>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <fstream>
#include <iterator>
#include <string>
#include <vector>

namespace
{

const UINT kCodePages[] = {65001, 949, 932, 936, 950, 1250, 1251, 1252, 1253, 1254, 1255, 1256, 1257, 1258, 874};

// Same names as GetIconvEncoding in StringCodec.cpp (static there).
const char *IconvName(UINT codePage)
{
    switch (codePage)
    {
    case 949:
        return "CP949";
    case 932:
        return "CP932";
    case 936:
        return "CP936";
    case 950:
        return "CP950";
    case 874:
        return "CP874";
    case 1250:
        return "CP1250";
    case 1251:
        return "CP1251";
    case 1252:
        return "CP1252";
    case 1253:
        return "CP1253";
    case 1254:
        return "CP1254";
    case 1255:
        return "CP1255";
    case 1256:
        return "CP1256";
    case 1257:
        return "CP1257";
    case 1258:
        return "CP1258";
    default:
        return "UTF-8";
    }
}

void PrintWide(const wchar_t *w, int n)
{
    if (n <= 0)
    {
        std::fputs("-", stdout);
        return;
    }
    for (int i = 0; i < n; i++)
        std::printf(i ? ",%x" : "%x", static_cast<unsigned>(w[i]));
}

void PrintBytes(const void *data, int n)
{
    if (n <= 0)
    {
        std::fputs("-", stdout);
        return;
    }
    const auto *b = static_cast<const unsigned char *>(data);
    for (int i = 0; i < n; i++)
        std::printf("%02x", b[i]);
}

using DecodeFn = int (*)(UINT, DWORD, LPCSTR, int, LPWSTR, int);
using EncodeFn = int (*)(UINT, DWORD, LPCWSTR, int, LPSTR, int, LPCSTR, LPBOOL);

// "<tag> <input> <output, 64 slots> n=<size query> c1=<output, 1 slot>"
void DumpDecode(char tag, DecodeFn decode, UINT codePage, const std::vector<unsigned char> &in)
{
    const auto *src = reinterpret_cast<LPCSTR>(in.data());
    int const n = static_cast<int>(in.size());
    wchar_t out[64];
    int const written = decode(codePage, 0, src, n, out, 64);
    int const count = decode(codePage, 0, src, n, nullptr, 0);
    wchar_t one[1];
    int const written1 = decode(codePage, 0, src, n, one, 1);

    std::printf("%c ", tag);
    PrintBytes(in.data(), n);
    std::putchar(' ');
    PrintWide(out, written);
    std::printf(" n=%d c1=", count);
    PrintWide(one, written1);
    std::putchar('\n');
}

void DumpEncode(char tag, EncodeFn encode, UINT codePage, const std::vector<wchar_t> &in)
{
    int const n = static_cast<int>(in.size());
    char out[64];
    int const written = encode(codePage, 0, in.data(), n, out, 64, nullptr, nullptr);
    int const count = encode(codePage, 0, in.data(), n, nullptr, 0, nullptr, nullptr);
    char one[1];
    int const written1 = encode(codePage, 0, in.data(), n, one, 1, nullptr, nullptr);

    std::printf("%c ", tag);
    PrintWide(in.data(), n);
    std::putchar(' ');
    PrintBytes(out, written);
    std::printf(" n=%d c1=", count);
    PrintBytes(one, written1);
    std::putchar('\n');
}

// Code points every encoder is tried on: the BMP, a sample of the other
// planes, all of the tag block (which glibc drops), and values past Unicode.
std::vector<unsigned> EncodeSample()
{
    std::vector<unsigned> cps;
    for (unsigned c = 0; c < 0x10000; c++)
        cps.push_back(c);
    for (unsigned c = 0x10000; c < 0x110000; c += 0x101)
        cps.push_back(c);
    for (unsigned c = 0xdff00; c < 0xe0100; c++)
        cps.push_back(c);
    for (unsigned c : {0x10ffffu, 0x110000u, 0x1fffffu, 0x200000u, 0x3ffffffu, 0x4000000u, 0x7fffffffu, 0x80000000u,
                       0xffffff41u, 0xffffffffu})
        cps.push_back(c);
    return cps;
}

int Codec(UINT codePage)
{
    // Single bytes, alone and followed by a space (which flushes the
    // character glibc's CP1255/CP1258 decoders hold back for combining).
    for (int b = 0; b < 256; b++)
    {
        DumpDecode('D', MultiByteToWideChar, codePage, {static_cast<unsigned char>(b)});
        DumpDecode('D', MultiByteToWideChar, codePage, {static_cast<unsigned char>(b), 0x20});
    }

    // Every byte pair, alone and followed by a space.
    for (int l = 0; l < 256; l++)
        for (int t = 0; t < 256; t++)
        {
            auto const lead = static_cast<unsigned char>(l);
            auto const trail = static_cast<unsigned char>(t);
            // glibc's UHC rejects A2 E8 after consuming both bytes; the C++
            // loop then skips one more byte, past the end of the input, and
            // converts whatever memory follows. Only test it with a suffix.
            if (!(codePage == 949 && l == 0xa2 && t == 0xe8))
                DumpDecode('D', MultiByteToWideChar, codePage, {lead, trail});
            DumpDecode('D', MultiByteToWideChar, codePage, {lead, trail, 0x20});
        }
    DumpDecode('D', MultiByteToWideChar, codePage, {0xa2, 0xe8, 0x41, 0x42});

    // Combining sequences: any byte, then up to two combining marks (or a
    // space, a letter, an undefined byte).
    std::vector<unsigned char> marks;
    if (codePage == 1255)
        for (int b = 0xc0; b <= 0xd2; b++)
            marks.push_back(static_cast<unsigned char>(b));
    if (codePage == 1258)
        marks = {0xcc, 0xd2, 0xde, 0xec, 0xf2, 0xca};
    if (!marks.empty())
    {
        for (unsigned char const extra : {0x20, 0x41, 0x81})
            marks.push_back(extra);
        for (int x = 0; x < 256; x++)
            for (unsigned char const y : marks)
                for (unsigned char const z : marks)
                {
                    DumpDecode('D', MultiByteToWideChar, codePage, {static_cast<unsigned char>(x), y, z});
                    DumpDecode('D', MultiByteToWideChar, codePage, {static_cast<unsigned char>(x), y, z, 0x20});
                }
    }

    // UTF-8 sequence classes: 3- to 6-byte forms, overlong, surrogates,
    // truncated and broken continuations.
    if (codePage == 65001)
    {
        const unsigned char tails[] = {0x00, 0x41, 0x7f, 0x80, 0x8f, 0x90, 0x9f, 0xa0, 0xbf, 0xc0, 0xff};
        for (int a = 0xc0; a < 256; a++)
            for (int b = 0x80; b < 0xc0; b++)
                for (unsigned char const c : tails)
                    DumpDecode('D', MultiByteToWideChar, codePage,
                               {static_cast<unsigned char>(a), static_cast<unsigned char>(b), c});
        for (int a = 0xf0; a < 256; a++)
            for (int b = 0x80; b < 0xc0; b++)
                for (unsigned char const c : {0x41, 0x80, 0xbf})
                    for (unsigned char const d : {0x41, 0x80, 0xbf})
                    {
                        DumpDecode('D', MultiByteToWideChar, codePage,
                                   {static_cast<unsigned char>(a), static_cast<unsigned char>(b), c, d});
                        DumpDecode('D', MultiByteToWideChar, codePage,
                                   {static_cast<unsigned char>(a), static_cast<unsigned char>(b), c, d, 0x80, 0x80});
                        DumpDecode('D', MultiByteToWideChar, codePage,
                                   {static_cast<unsigned char>(a), static_cast<unsigned char>(b), c, d, 0xbf, 0x41});
                    }
    }

    for (unsigned const c : EncodeSample())
        DumpEncode('E', WideCharToMultiByte, codePage, {static_cast<wchar_t>(c)});
    // Errors and dropped tags in the middle of a string.
    const std::vector<std::vector<unsigned>> sequences = {
        {0x41, 0xe0001, 0x42},  {0x41, 0xd800, 0x42},   {0x41, 0xffffffff, 0x42}, {0xac00, 0x41},
        {0x41, 0x110000, 0x42}, {0xfb2c, 0x41, 0x1ea0}, {0x30a2, 0x4e00, 0x41}};
    for (auto const &sequence : sequences)
    {
        std::vector<wchar_t> wide;
        for (unsigned const c : sequence)
            wide.push_back(static_cast<wchar_t>(c));
        DumpEncode('E', WideCharToMultiByte, codePage, wide);
    }
    return 0;
}

int Vietnamese()
{
    for (int b = 0; b < 256; b++)
        DumpDecode('V', Ymir_MultiByteToWideChar, CP_1258, {static_cast<unsigned char>(b)});
    for (int x = 0; x < 256; x++)
        for (int y = 0; y < 256; y++)
            DumpDecode('V', Ymir_MultiByteToWideChar, CP_1258,
                       {static_cast<unsigned char>(x), static_cast<unsigned char>(y)});
    const unsigned char tones[] = {0xcc, 0xd2, 0xde, 0xec, 0xf2};
    for (int x = 0; x < 256; x++)
        for (unsigned char const t : tones)
            for (unsigned char const y : {0xcc, 0xd2, 0xde, 0xec, 0xf2, 0x41, 0x20, 0xc3})
                DumpDecode('V', Ymir_MultiByteToWideChar, CP_1258, {static_cast<unsigned char>(x), t, y});

    for (unsigned const c : EncodeSample())
        DumpEncode('W', Ymir_WideCharToMultiByte, CP_1258, {static_cast<wchar_t>(c)});
    DumpEncode('W', Ymir_WideCharToMultiByte, CP_1258,
               {static_cast<wchar_t>(0x1ea0), static_cast<wchar_t>(0x41), static_cast<wchar_t>(0x4e00),
                static_cast<wchar_t>(0x1ec7)});
    return 0;
}

uint32_t g_seed = 0x2545f491;
uint32_t NextRandom()
{
    g_seed = g_seed * 1664525u + 1013904223u;
    return g_seed >> 8;
}

void DumpShape(const std::vector<wchar_t> &src)
{
    std::vector<wchar_t> dst(src.size() + 1, 0);
    auto const n = static_cast<int>(Arabic_MakeShape(src.data(), src.size(), dst.data(), src.size()));
    std::fputs("S ", stdout);
    PrintWide(src.data(), static_cast<int>(src.size()));
    std::putchar(' ');
    PrintWide(dst.data(), n);
    std::fputs(" h=", stdout);
    for (int i = -1; i < n; i++)
        std::putchar(Arabic_HasPresentation(dst.data(), i) ? '1' : '0');
    std::putchar('\n');
}

int Arabic()
{
    for (unsigned c = 0; c < 0x10000; c++)
    {
        auto const w = static_cast<wchar_t>(c);
        std::printf("P %x %d%d%d %x\n", c, Arabic_IsInSpace(w), Arabic_IsInSymbol(w), Arabic_IsInPresentation(w),
                    static_cast<unsigned>(Arabic_ConvSymbol(w)));
    }

    std::vector<wchar_t> alphabet;
    for (wchar_t c = 0x0621; c <= 0x064a; c++)
        alphabet.push_back(c);
    for (unsigned const c : {0x064bu, 0x0651u, 0x0670u, 0x06d6u, 0x06edu, 0x06ddu, 0x20u, 0x41u, 0x28u, 0xfe8du, 0x08u})
        alphabet.push_back(static_cast<wchar_t>(c));

    DumpShape({});
    for (wchar_t const a : alphabet)
    {
        DumpShape({a});
        for (wchar_t const b : alphabet)
        {
            DumpShape({a, b});
            for (wchar_t const c : alphabet)
                DumpShape({a, b, c});
        }
    }
    for (int i = 0; i < 20000; i++)
    {
        std::vector<wchar_t> src(4 + NextRandom() % 21);
        for (auto &c : src)
            c = alphabet[NextRandom() % alphabet.size()];
        DumpShape(src);
    }
    return 0;
}

void DumpCompare(const std::string &a, const std::string &b, size_t length)
{
    std::fputs("C ", stdout);
    PrintBytes(a.data(), static_cast<int>(a.size()));
    std::putchar(' ');
    PrintBytes(b.data(), static_cast<int>(b.size()));
    std::printf(" %zu %d\n", length, ShiftJIS_StringCompareCI(a.c_str(), b.c_str(), length));
}

int Japanese()
{
    for (int b = 0; b < 256; b++)
        std::printf("L %02x %d%d\n", b, ShiftJIS_IsLeadByte(static_cast<char>(b)) ? 1 : 0,
                    ShiftJIS_IsTrailByte(static_cast<char>(b)) ? 1 : 0);

    const char alphabet[] = {'a', 'A', 'z', 'Z', '[', '\x81', '\x40', '\x9f', '\xe0', '\xfc', '\x80', '\x7e'};
    std::vector<std::string> strings = {""};
    for (char const a : alphabet)
    {
        strings.emplace_back(1, a);
        for (char const b : alphabet)
            strings.push_back(std::string{a, b});
    }
    for (auto const &a : strings)
        for (auto const &b : strings)
            for (size_t length = 0; length < 4; length++)
                DumpCompare(a, b, length);
    for (int i = 0; i < 20000; i++)
    {
        std::string a(NextRandom() % 9, ' ');
        std::string b(NextRandom() % 9, ' ');
        for (auto &c : a)
            c = alphabet[NextRandom() % sizeof(alphabet)];
        for (auto &c : b)
            c = alphabet[NextRandom() % sizeof(alphabet)];
        DumpCompare(a, b, NextRandom() % 11);
    }
    return 0;
}

uint64_t Fnv1a64(const void *data, size_t size)
{
    const auto *b = static_cast<const unsigned char *>(data);
    uint64_t hash = 0xcbf29ce484222325ull;
    for (size_t i = 0; i < size; i++)
    {
        hash ^= b[i];
        hash *= 0x100000001b3ull;
    }
    return hash;
}

// Hash of wide characters as little-endian 32-bit units.
uint64_t HashWide(const std::vector<wchar_t> &w, int n)
{
    std::vector<unsigned char> bytes;
    for (int i = 0; i < n; i++)
        for (int k = 0; k < 4; k++)
            bytes.push_back(static_cast<unsigned char>(static_cast<uint32_t>(w[i]) >> (8 * k)));
    return Fnv1a64(bytes.data(), bytes.size());
}

// "H <file> <code page> <size query> <written> <hash> <size query> <written> <hash>"
// for the whole file decoded, then encoded back.
void HashFile(const char *path, const std::vector<char> &data, UINT codePage, DecodeFn decode, EncodeFn encode,
              const char *label)
{
    int const n = static_cast<int>(data.size());
    int const count = decode(codePage, 0, data.data(), n, nullptr, 0);
    std::vector<wchar_t> wide(data.size() + 1);
    int const written = decode(codePage, 0, data.data(), n, wide.data(), n);
    int const bytesCount = encode(codePage, 0, wide.data(), written, nullptr, 0, nullptr, nullptr);
    std::vector<char> back(static_cast<size_t>(written) * 6 + 1);
    int const bytesWritten =
        encode(codePage, 0, wide.data(), written, back.data(), static_cast<int>(back.size()), nullptr, nullptr);
    std::printf("H %s %s %d %d %016llx %d %d %016llx\n", path, label, count, written,
                static_cast<unsigned long long>(HashWide(wide, written)), bytesCount, bytesWritten,
                static_cast<unsigned long long>(Fnv1a64(back.data(), static_cast<size_t>(bytesWritten))));
}

int Corpus(UINT codePage, int argc, char **argv)
{
    for (int f = 0; f < argc; f++)
    {
        std::ifstream in(argv[f], std::ios::binary);
        if (!in)
        {
            std::fprintf(stderr, "cannot open %s\n", argv[f]);
            return 1;
        }
        std::vector<char> data((std::istreambuf_iterator<char>(in)), std::istreambuf_iterator<char>());

        // "L <file> <line> <decoded> n=<size query> t=<written into half the
        // room> <encoded back, = when equal to the line> n=<size query>"
        size_t start = 0;
        int lineNo = 0;
        while (start < data.size())
        {
            size_t end = start;
            while (end < data.size() && data[end] != '\n')
                end++;
            const char *line = data.data() + start;
            int const len = static_cast<int>(end - start);

            std::vector<wchar_t> wide(static_cast<size_t>(len) + 1);
            int const written = Ymir_MultiByteToWideChar(codePage, 0, line, len, wide.data(), len);
            int const count = Ymir_MultiByteToWideChar(codePage, 0, line, len, nullptr, 0);
            int const half = Ymir_MultiByteToWideChar(codePage, 0, line, len, wide.data() + len / 2 + 1, len / 2);
            // Recompute: the half-room call wrote over the tail of the buffer.
            Ymir_MultiByteToWideChar(codePage, 0, line, len, wide.data(), len);

            std::vector<char> back(static_cast<size_t>(written) * 6 + 1);
            int const bytes = Ymir_WideCharToMultiByte(codePage, 0, wide.data(), written, back.data(),
                                                       static_cast<int>(back.size()), nullptr, nullptr);
            int const bytesCount =
                Ymir_WideCharToMultiByte(codePage, 0, wide.data(), written, nullptr, 0, nullptr, nullptr);

            std::printf("L %s %d ", argv[f], lineNo);
            PrintWide(wide.data(), written);
            std::printf(" n=%d t=%d ", count, half);
            if (bytes == len && std::memcmp(back.data(), line, static_cast<size_t>(len)) == 0)
                std::putchar('=');
            else
                PrintBytes(back.data(), bytes);
            std::printf(" n=%d\n", bytesCount);

            start = end + 1;
            lineNo++;
        }

        for (UINT const cp : kCodePages)
        {
            char label[16];
            std::snprintf(label, sizeof(label), "%u", cp);
            HashFile(argv[f], data, cp, MultiByteToWideChar, WideCharToMultiByte, label);
        }
        HashFile(argv[f], data, CP_1258, Ymir_MultiByteToWideChar, Ymir_WideCharToMultiByte, "ymir1258");
    }
    return 0;
}

// --- Table generation (glibc iconv) -----------------------------------------

struct Step
{
    bool ok;
    int err;
    size_t consumed;
    std::vector<uint32_t> out;
};

// One iconv() call on a fresh descriptor.
Step DecodeOnce(const char *charset, const std::vector<unsigned char> &in)
{
    iconv_t const cd = iconv_open("WCHAR_T", charset);
    if (cd == reinterpret_cast<iconv_t>(-1))
    {
        std::perror(charset);
        std::exit(1);
    }
    wchar_t out[16];
    char *inBuf = reinterpret_cast<char *>(const_cast<unsigned char *>(in.data()));
    size_t inLeft = in.size();
    char *outBuf = reinterpret_cast<char *>(out);
    size_t outLeft = sizeof(out);
    errno = 0;
    size_t const r = iconv(cd, &inBuf, &inLeft, &outBuf, &outLeft);
    Step step{r != static_cast<size_t>(-1), errno, in.size() - inLeft, {}};
    for (auto *w = out; w < reinterpret_cast<wchar_t *>(outBuf); w++)
        step.out.push_back(static_cast<uint32_t>(*w));
    iconv_close(cd);
    return step;
}

// Repeated iconv() calls on one descriptor, skipping a byte after each
// error, as MultiByteToWideChar does.
std::vector<uint32_t> DecodeLoop(const char *charset, const std::vector<unsigned char> &in)
{
    iconv_t const cd = iconv_open("WCHAR_T", charset);
    wchar_t out[16];
    char *inBuf = reinterpret_cast<char *>(const_cast<unsigned char *>(in.data()));
    size_t inLeft = in.size();
    char *outBuf = reinterpret_cast<char *>(out);
    size_t outLeft = sizeof(out);
    while (inLeft > 0)
        if (iconv(cd, &inBuf, &inLeft, &outBuf, &outLeft) == static_cast<size_t>(-1))
        {
            if (errno == E2BIG || inLeft == 0)
                break;
            inBuf++;
            inLeft--;
        }
    iconv_close(cd);
    std::vector<uint32_t> result;
    for (auto *w = out; w < reinterpret_cast<wchar_t *>(outBuf); w++)
        result.push_back(static_cast<uint32_t>(*w));
    return result;
}

Step EncodeOnce(const char *charset, uint32_t c)
{
    iconv_t const cd = iconv_open(charset, "WCHAR_T");
    wchar_t in[1] = {static_cast<wchar_t>(c)};
    unsigned char out[16];
    char *inBuf = reinterpret_cast<char *>(in);
    size_t inLeft = sizeof(in);
    char *outBuf = reinterpret_cast<char *>(out);
    size_t outLeft = sizeof(out);
    errno = 0;
    size_t const r = iconv(cd, &inBuf, &inLeft, &outBuf, &outLeft);
    Step step{r != static_cast<size_t>(-1), errno, sizeof(in) - inLeft, {}};
    for (auto *b = out; b < reinterpret_cast<unsigned char *>(outBuf); b++)
        step.out.push_back(*b);
    iconv_close(cd);
    return step;
}

[[noreturn]] void Unexpected(const char *charset, const char *what, const std::vector<unsigned char> &in,
                             const Step &step)
{
    std::fprintf(stderr, "%s: unexpected %s for ", charset, what);
    for (unsigned char const b : in)
        std::fprintf(stderr, "%02x", b);
    std::fprintf(stderr, ": ok=%d errno=%d consumed=%zu out=%zu\n", step.ok, step.err, step.consumed, step.out.size());
    std::exit(1);
}

constexpr uint32_t kInvalid = 0xffffffff;
constexpr uint32_t kLead = 0xfffffffe;
constexpr uint32_t kHeld = 0x80000000;  // flag: held back for combining
constexpr uint16_t kPairSkip2 = 0xffff; // rejected after consuming both bytes

void PutU32(std::vector<unsigned char> &out, uint32_t v)
{
    for (int k = 0; k < 4; k++)
        out.push_back(static_cast<unsigned char>(v >> (8 * k)));
}

struct Composition
{
    uint32_t last, mark, composed, held;
    std::vector<unsigned char> prefix; // bytes that reach `composed`
};

int WriteTable(UINT codePage, const std::string &outDir)
{
    const char *charset = IconvName(codePage);

    uint32_t single[256];
    bool hasLead = false;
    for (int b = 0; b < 256; b++)
    {
        std::vector<unsigned char> const in = {static_cast<unsigned char>(b)};
        Step const s = DecodeOnce(charset, in);
        if (s.ok && s.consumed == 1 && s.out.size() == 1)
            single[b] = s.out[0];
        else if (s.ok && s.consumed == 1 && s.out.empty())
        {
            Step const flushed = DecodeOnce(charset, {static_cast<unsigned char>(b), 0x20});
            if (!flushed.ok || flushed.out.size() != 2 || flushed.out[1] != 0x20)
                Unexpected(charset, "held byte", in, flushed);
            single[b] = flushed.out[0] | kHeld;
        }
        else if (!s.ok && s.err == EINVAL && s.consumed == 0)
        {
            single[b] = kLead;
            hasLead = true;
        }
        else if (!s.ok && s.err == EILSEQ && s.consumed == 0)
            single[b] = kInvalid;
        else
            Unexpected(charset, "single byte", in, s);
        if (single[b] != kInvalid && single[b] != kLead && (single[b] & ~kHeld) > 0xffff)
            Unexpected(charset, "non-BMP single byte", in, s);
    }

    std::vector<uint16_t> pairs;
    if (hasLead)
    {
        pairs.assign(128 * 256, 0);
        for (int l = 0; l < 256; l++)
        {
            if (single[l] != kLead)
                continue;
            if (l < 0x80)
                Unexpected(charset, "ASCII lead byte", {static_cast<unsigned char>(l)}, {});
            for (int t = 0; t < 256; t++)
            {
                std::vector<unsigned char> const in = {static_cast<unsigned char>(l), static_cast<unsigned char>(t)};
                Step const s = DecodeOnce(charset, in);
                uint16_t value = 0;
                if (s.ok && s.consumed == 2 && s.out.size() == 1 && s.out[0] > 0 && s.out[0] < 0xffff)
                    value = static_cast<uint16_t>(s.out[0]);
                else if (!s.ok && s.err == EILSEQ && s.consumed == 0)
                    value = 0;
                else if (!s.ok && s.err == EILSEQ && s.consumed == 2 && s.out.empty())
                    value = kPairSkip2;
                else
                    Unexpected(charset, "pair", in, s);
                pairs[(l - 0x80) * 256 + t] = value;
            }
        }
    }

    // Compositions of a held character with the next byte's character,
    // found breadth-first from every held single byte.
    std::vector<Composition> comps;
    std::vector<std::pair<std::vector<unsigned char>, uint32_t>> work; // prefix, held character
    for (int b = 0; b < 256; b++)
        if (single[b] != kInvalid && single[b] != kLead && (single[b] & kHeld))
            work.push_back({{static_cast<unsigned char>(b)}, single[b] & ~kHeld});
    for (size_t w = 0; w < work.size(); w++)
    {
        auto const [prefix, last] = work[w];
        for (int y = 0; y < 256; y++)
        {
            std::vector<unsigned char> in = prefix;
            in.push_back(static_cast<unsigned char>(y));
            std::vector<unsigned char> flushedIn = in;
            flushedIn.push_back(0x20);
            std::vector<uint32_t> const flushed = DecodeLoop(charset, flushedIn);

            std::vector<uint32_t> plain = {last};
            if (single[y] != kInvalid)
                plain.push_back(single[y] & ~kHeld);
            plain.push_back(0x20);
            if (flushed == plain)
                continue;
            if (flushed.size() != 2 || flushed[1] != 0x20 || single[y] == kInvalid || (single[y] & kHeld))
                Unexpected(charset, "composition", flushedIn, {});

            std::vector<uint32_t> const alone = DecodeLoop(charset, in);
            uint32_t const held = alone.empty() ? 1 : 0;
            if (!held && (alone.size() != 1 || alone[0] != flushed[0]))
                Unexpected(charset, "composition end", in, {});
            comps.push_back({last, single[y], flushed[0], held, in});
            if (held)
            {
                bool seen = false;
                for (auto const &item : work)
                    seen = seen || item.second == flushed[0];
                if (!seen)
                    work.push_back({in, flushed[0]});
            }
        }
    }

    // The encoder, as exceptions to the inverse of the decoder (first single
    // byte, else first pair, in byte order).
    std::vector<std::vector<unsigned char>> inverse(0x10000);
    for (int b = 0; b < 256; b++)
        if (single[b] != kInvalid && single[b] != kLead && inverse[single[b] & ~kHeld].empty())
            inverse[single[b] & ~kHeld] = {static_cast<unsigned char>(b)};
    for (size_t i = 0; i < pairs.size(); i++)
        if (pairs[i] != 0 && pairs[i] != kPairSkip2 && inverse[pairs[i]].empty())
            inverse[pairs[i]] = {static_cast<unsigned char>(0x80 + i / 256), static_cast<unsigned char>(i % 256)};

    std::vector<unsigned char> exceptions;
    uint32_t exceptionCount = 0;
    for (uint32_t c = 0; c < 0x110000; c++)
    {
        Step const s = EncodeOnce(charset, c);
        std::vector<unsigned char> bytes;
        if (s.ok && s.consumed == 4)
            bytes.assign(s.out.begin(), s.out.end());
        else if (s.ok || s.err != EILSEQ || s.consumed != 0)
            Unexpected(charset, "encoding", {}, s);
        bool const tag = (c >> 7) == (0xe0000 >> 7);
        if (tag)
        {
            if (!s.ok || !bytes.empty())
                Unexpected(charset, "tag encoding", {}, s);
            continue;
        }
        if (s.ok && bytes.empty())
            Unexpected(charset, "empty encoding", {}, s);
        std::vector<unsigned char> const expected = c < 0x10000 ? inverse[c] : std::vector<unsigned char>{};
        if (bytes == expected)
            continue;
        if (bytes.size() > 3)
            Unexpected(charset, "long encoding", {}, s);
        PutU32(exceptions, c);
        exceptions.push_back(static_cast<unsigned char>(bytes.size()));
        for (size_t k = 0; k < 3; k++)
            exceptions.push_back(k < bytes.size() ? bytes[k] : 0);
        exceptionCount++;
    }

    std::vector<unsigned char> file = {'M', '2', 'C', 'P'};
    PutU32(file, 1);
    for (uint32_t const v : single)
        PutU32(file, v);
    PutU32(file, static_cast<uint32_t>(pairs.size()));
    for (uint16_t const v : pairs)
    {
        file.push_back(static_cast<unsigned char>(v));
        file.push_back(static_cast<unsigned char>(v >> 8));
    }
    PutU32(file, static_cast<uint32_t>(comps.size()));
    for (auto const &c : comps)
    {
        PutU32(file, c.last);
        PutU32(file, c.mark);
        PutU32(file, c.composed);
        PutU32(file, c.held);
    }
    PutU32(file, exceptionCount);
    file.insert(file.end(), exceptions.begin(), exceptions.end());

    std::string const path = outDir + "/cp" + std::to_string(codePage) + ".bin";
    FILE *out = std::fopen(path.c_str(), "wb");
    if (!out || std::fwrite(file.data(), 1, file.size(), out) != file.size() || std::fclose(out) != 0)
    {
        std::perror(path.c_str());
        return 1;
    }
    std::printf("%s: %zu pairs, %zu compositions, %u encoder exceptions\n", path.c_str(), pairs.size(), comps.size(),
                exceptionCount);
    return 0;
}

int Tables(const std::string &outDir)
{
    for (UINT const cp : kCodePages)
        if (cp != 65001 && WriteTable(cp, outDir) != 0)
            return 1;
    return 0;
}

} // namespace

int main(int argc, char **argv)
{
    std::string const command = argc > 1 ? argv[1] : "";
    if (command == "tables" && argc == 3)
        return Tables(argv[2]);
    if (command == "codec" && argc == 3)
        return Codec(static_cast<UINT>(std::stoul(argv[2])));
    if (command == "vietnamese" && argc == 2)
        return Vietnamese();
    if (command == "arabic" && argc == 2)
        return Arabic();
    if (command == "japanese" && argc == 2)
        return Japanese();
    if (command == "corpus" && argc >= 3)
        return Corpus(static_cast<UINT>(std::stoul(argv[2])), argc - 3, argv + 3);

    std::fprintf(stderr,
                 "usage: %s tables <out dir> | codec <code page> | vietnamese | arabic | japanese |"
                 " corpus <code page> <file>...\n",
                 argv[0]);
    return 2;
}
