#include <support/TestAssert.h>

#include <Neoeter/Image/DXTCImage.h>

#include <cstring>
#include <vector>

namespace
{

constexpr DWORD kFourCC(char a, char b, char c, char d)
{
    return static_cast<DWORD>(static_cast<BYTE>(a)) |
           (static_cast<DWORD>(static_cast<BYTE>(b)) << 8) |
           (static_cast<DWORD>(static_cast<BYTE>(c)) << 16) |
           (static_cast<DWORD>(static_cast<BYTE>(d)) << 24);
}

constexpr DWORD kDdsdCaps = 0x00000001u;
constexpr DWORD kDdsdHeight = 0x00000002u;
constexpr DWORD kDdsdWidth = 0x00000004u;
constexpr DWORD kDdsdPixelFormat = 0x00001000u;
constexpr DWORD kDdsdMipMapCount = 0x00020000u;
constexpr DWORD kDdsdLinearSize = 0x00080000u;
constexpr DWORD kDdpfFourCC = 0x00000004u;

void put32(std::vector<BYTE> & buf, size_t offset, DWORD value)
{
    std::memcpy(&buf[offset], &value, sizeof(value));
}

void put16(std::vector<BYTE> & buf, size_t offset, WORD value)
{
    std::memcpy(&buf[offset], &value, sizeof(value));
}

// Builds a syntactically valid DDS container around the supplied compressed
// payload. Only the fields CDXTCImage actually reads are populated.
std::vector<BYTE> makeDds(DWORD fourCc, UINT width, UINT height, UINT mipCount,
                          DWORD linearSize, const std::vector<BYTE> & payload)
{
    std::vector<BYTE> buf(128 + payload.size(), 0);
    put32(buf, 0, kFourCC('D', 'D', 'S', ' '));

    const size_t hdr = 4;
    DWORD flags = kDdsdCaps | kDdsdHeight | kDdsdWidth | kDdsdPixelFormat |
                  kDdsdLinearSize;
    if (mipCount > 1)
        flags |= kDdsdMipMapCount;

    put32(buf, hdr + 0, 124);       // dwSize
    put32(buf, hdr + 4, flags);     // dwFlags
    put32(buf, hdr + 8, height);    // dwHeight
    put32(buf, hdr + 12, width);    // dwWidth
    put32(buf, hdr + 16, linearSize);
    put32(buf, hdr + 24, mipCount); // dwMipMapCount
    put32(buf, hdr + 72, 32);       // ddpfPixelFormat.dwSize
    put32(buf, hdr + 76, kDdpfFourCC);
    put32(buf, hdr + 80, fourCc);

    if (!payload.empty())
        std::memcpy(&buf[128], payload.data(), payload.size());

    return buf;
}

std::vector<BYTE> colorBlock(WORD col0, WORD col1, const BYTE rows[4])
{
    std::vector<BYTE> block(8);
    put16(block, 0, col0);
    put16(block, 2, col1);
    std::memcpy(&block[4], rows, 4);
    return block;
}

std::vector<BYTE> alphaBlock5(BYTE alpha0, BYTE alpha1, const int indices[16])
{
    std::vector<BYTE> block(8);
    block[0] = alpha0;
    block[1] = alpha1;

    // DXT5 packs 16 three-bit indices little-endian across stuff[6].
    unsigned long long bits = 0;
    for (int i = 0; i < 16; ++i)
        bits |= static_cast<unsigned long long>(indices[i] & 0x7) << (3 * i);
    for (int i = 0; i < 6; ++i)
        block[2 + i] = static_cast<BYTE>(bits >> (8 * i));

    return block;
}

std::vector<BYTE> concat(const std::vector<BYTE> & a, const std::vector<BYTE> & b)
{
    std::vector<BYTE> out(a);
    out.insert(out.end(), b.begin(), b.end());
    return out;
}

// Loads the fixture, hands the compressed payload to the image and decompresses
// one mip level. Mirrors the calling convention used by the texture loader.
std::vector<DWORD> decompress(const std::vector<BYTE> & dds, int mipLevel)
{
    CDXTCImage image;
    TEST_ASSERT(image.LoadHeaderFromMemory(dds.data(), static_cast<int>(dds.size())));

    const size_t payload = dds.size() - 128;
    image.m_bCompVector[mipLevel].assign(dds.begin() + 128, dds.begin() + 128 + payload);

    const int width = image.m_nWidth >> mipLevel;
    const int height = image.m_nHeight >> mipLevel;
    std::vector<DWORD> pixels(static_cast<size_t>(width) * height, 0);
    image.Decompress(mipLevel, pixels.data());
    return pixels;
}

DWORD rgba(BYTE a, BYTE r, BYTE g, BYTE b)
{
    return (static_cast<DWORD>(a) << 24) | (static_cast<DWORD>(r) << 16) |
           (static_cast<DWORD>(g) << 8) | static_cast<DWORD>(b);
}

void test_dxt1_four_color_block()
{
    TEST_SECTION("DXT1 four-colour block");

    const BYTE rows[4] = {0xE4, 0x1B, 0xE4, 0x1B};
    const std::vector<BYTE> block = colorBlock(0xF800, 0x001F, rows); // red > blue
    const std::vector<BYTE> dds = makeDds(kFourCC('D', 'X', 'T', '1'), 4, 4, 1, 8, block);

    const std::vector<DWORD> pixels = decompress(dds, 0);
    TEST_ASSERT(pixels.size() == 16);

    const DWORD c0 = rgba(0xFF, 0xF8, 0x00, 0x00);
    const DWORD c1 = rgba(0xFF, 0x00, 0x00, 0xF8);
    const DWORD c2 = rgba(0xFF, 165, 0x00, 82);  // (2*c0 + c1) / 3
    const DWORD c3 = rgba(0xFF, 82, 0x00, 165);  // (c0 + 2*c1) / 3

    TEST_ASSERT(pixels[0] == c0);
    TEST_ASSERT(pixels[1] == c1);
    TEST_ASSERT(pixels[2] == c2);
    TEST_ASSERT(pixels[3] == c3);

    TEST_ASSERT(pixels[4] == c3);
    TEST_ASSERT(pixels[5] == c2);
    TEST_ASSERT(pixels[6] == c1);
    TEST_ASSERT(pixels[7] == c0);

    TEST_PASS("2-bit selectors and 2/3 interpolation are exact");
}

void test_dxt1_three_color_transparent_block()
{
    TEST_SECTION("DXT1 three-colour block");

    const BYTE rows[4] = {0xE4, 0xE4, 0xE4, 0xE4};
    const std::vector<BYTE> block = colorBlock(0x001F, 0xF800, rows); // blue <= red
    const std::vector<BYTE> dds = makeDds(kFourCC('D', 'X', 'T', '1'), 4, 4, 1, 8, block);

    const std::vector<DWORD> pixels = decompress(dds, 0);
    TEST_ASSERT(pixels.size() == 16);

    TEST_ASSERT(pixels[0] == rgba(0xFF, 0x00, 0x00, 0xF8));
    TEST_ASSERT(pixels[1] == rgba(0xFF, 0xF8, 0x00, 0x00));
    TEST_ASSERT(pixels[2] == rgba(0xFF, 124, 0x00, 124)); // (c0 + c1) / 2
    TEST_ASSERT(pixels[3] == 0x00000000);                 // selector 3 is transparent

    TEST_PASS("median colour and transparent selector match the DXT1 spec");
}

void test_dxt3_explicit_alpha()
{
    TEST_SECTION("DXT3 explicit alpha");

    std::vector<BYTE> alpha(8, 0);
    put16(alpha, 0, 0x3210); // row 0: nibbles 0,1,2,3
    put16(alpha, 2, 0x7654); // row 1: nibbles 4,5,6,7
    put16(alpha, 4, 0xBA98); // row 2: nibbles 8,9,A,B
    put16(alpha, 6, 0xFEDC); // row 3: nibbles C,D,E,F

    const BYTE rows[4] = {0, 0, 0, 0};
    const std::vector<BYTE> block = concat(alpha, colorBlock(0xF800, 0xF800, rows));
    TEST_ASSERT(block.size() == 16);

    const std::vector<BYTE> dds = makeDds(kFourCC('D', 'X', 'T', '3'), 4, 4, 1, 16, block);
    const std::vector<DWORD> pixels = decompress(dds, 0);
    TEST_ASSERT(pixels.size() == 16);

    for (int i = 0; i < 16; ++i)
    {
        const BYTE nibble = static_cast<BYTE>(i);
        const BYTE alpha8 = static_cast<BYTE>((nibble << 4) | nibble);
        const DWORD expected = rgba(alpha8, 0xF8, 0x00, 0x00);
        if (pixels[i] != expected)
        {
            std::printf("    pixel %d: got 0x%08X expected 0x%08X\n", i,
                        static_cast<unsigned>(pixels[i]),
                        static_cast<unsigned>(expected));
            TEST_ASSERT(false);
        }
    }

    TEST_PASS("4-bit alpha expands to 8-bit by nibble replication");
}

void test_dxt5_eight_alpha_block()
{
    TEST_SECTION("DXT5 eight-alpha block");

    int indices[16];
    for (int i = 0; i < 16; ++i)
        indices[i] = i % 8;

    const std::vector<BYTE> alpha = alphaBlock5(200, 100, indices);
    const BYTE rows[4] = {0, 0, 0, 0};
    const std::vector<BYTE> block = concat(alpha, colorBlock(0xF800, 0xF800, rows));

    const std::vector<BYTE> dds = makeDds(kFourCC('D', 'X', 'T', '5'), 4, 4, 1, 16, block);
    const std::vector<DWORD> pixels = decompress(dds, 0);
    TEST_ASSERT(pixels.size() == 16);

    const BYTE expected[8] = {200, 100, 185, 171, 157, 142, 128, 114};
    for (int i = 0; i < 16; ++i)
    {
        const DWORD want = rgba(expected[i % 8], 0xF8, 0x00, 0x00);
        if (pixels[i] != want)
        {
            std::printf("    pixel %d: got 0x%08X expected 0x%08X\n", i,
                        static_cast<unsigned>(pixels[i]),
                        static_cast<unsigned>(want));
            TEST_ASSERT(false);
        }
    }

    TEST_PASS("6 interpolated alphas between two explicit endpoints");
}

void test_dxt5_six_alpha_block()
{
    TEST_SECTION("DXT5 six-alpha block");

    int indices[16];
    for (int i = 0; i < 16; ++i)
        indices[i] = i % 8;

    const std::vector<BYTE> alpha = alphaBlock5(100, 200, indices); // a0 <= a1
    const BYTE rows[4] = {0, 0, 0, 0};
    const std::vector<BYTE> block = concat(alpha, colorBlock(0xF800, 0xF800, rows));

    const std::vector<BYTE> dds = makeDds(kFourCC('D', 'X', 'T', '5'), 4, 4, 1, 16, block);
    const std::vector<DWORD> pixels = decompress(dds, 0);
    TEST_ASSERT(pixels.size() == 16);

    const BYTE expected[8] = {100, 200, 120, 140, 160, 180, 0, 255};
    for (int i = 0; i < 16; ++i)
    {
        const DWORD want = rgba(expected[i % 8], 0xF8, 0x00, 0x00);
        if (pixels[i] != want)
        {
            std::printf("    pixel %d: got 0x%08X expected 0x%08X\n", i,
                        static_cast<unsigned>(pixels[i]),
                        static_cast<unsigned>(want));
            TEST_ASSERT(false);
        }
    }

    TEST_PASS("6-alpha mode pins codes 6 and 7 to 0 and 255");
}

void test_dxt1_multiblock_image()
{
    TEST_SECTION("DXT1 block grid");

    // 8x8 red image: four identical blocks, all selectors point at colour 0.
    const BYTE rows[4] = {0, 0, 0, 0};
    const std::vector<BYTE> red = colorBlock(0xF800, 0xF800, rows);
    std::vector<BYTE> payload;
    for (int i = 0; i < 4; ++i)
        payload = concat(payload, red);

    const std::vector<BYTE> dds = makeDds(kFourCC('D', 'X', 'T', '1'), 8, 8, 1, 32, payload);
    const std::vector<DWORD> pixels = decompress(dds, 0);
    TEST_ASSERT(pixels.size() == 64);

    for (int y = 0; y < 8; ++y)
        for (int x = 0; x < 8; ++x)
            TEST_ASSERT(pixels[y * 8 + x] == rgba(0xFF, 0xF8, 0x00, 0x00));

    TEST_PASS("blocks are addressed row-major across the full surface");
}

void test_mip_chain_offsets()
{
    TEST_SECTION("mip chain");

    const BYTE rows[4] = {0, 0, 0, 0};
    std::vector<BYTE> payload;
    for (int i = 0; i < 4; ++i) // 8x8: four red blocks
        payload = concat(payload, colorBlock(0xF800, 0xF800, rows));
    const std::vector<BYTE> level1 = colorBlock(0x001F, 0x001F, rows);
    payload = concat(payload, level1); // 4x4: one blue block (8 bytes)

    const std::vector<BYTE> dds = makeDds(kFourCC('D', 'X', 'T', '1'), 8, 8, 2, 32, payload);

    CDXTCImage image;
    TEST_ASSERT(image.LoadHeaderFromMemory(dds.data(), static_cast<int>(dds.size())));
    TEST_ASSERT(image.m_dwMipMapCount == 2);
    TEST_ASSERT(image.m_bMipTexture);
    TEST_ASSERT(image.m_pbCompBufferByLevels[1] == dds.data() + 128 + 32);

    image.m_bCompVector[1].assign(dds.begin() + 128 + 32, dds.end());
    std::vector<DWORD> pixels(4 * 4, 0);
    image.Decompress(1, pixels.data());
    for (int i = 0; i < 16; ++i)
        TEST_ASSERT(pixels[i] == rgba(0xFF, 0x00, 0x00, 0xF8));

    BYTE copied[16] = {0};
    TEST_ASSERT(image.Copy(1, copied, 8));
    TEST_ASSERT(std::memcmp(copied, dds.data() + 128 + 32, 8) == 0);

    TEST_PASS("level 1 pointer and Copy() skip exactly the level 0 payload");
}

void test_header_validation()
{
    TEST_SECTION("DDS header validation");

    CDXTCImage image;
    TEST_ASSERT(!image.LoadHeaderFromMemory(nullptr, 0));

    const std::vector<BYTE> tiny(3, 0);
    TEST_ASSERT(!image.LoadHeaderFromMemory(tiny.data(), static_cast<int>(tiny.size())));

    std::vector<BYTE> badMagic = makeDds(kFourCC('D', 'X', 'T', '1'), 4, 4, 1, 8,
                                         std::vector<BYTE>(8, 0));
    badMagic[0] = 'X';
    TEST_ASSERT(!image.LoadHeaderFromMemory(badMagic.data(), static_cast<int>(badMagic.size())));

    // 4-byte magic + an incomplete 124-byte surface descriptor.
    std::vector<BYTE> truncated = makeDds(kFourCC('D', 'X', 'T', '1'), 4, 4, 1, 8, {});
    truncated.resize(4 + 123);
    TEST_ASSERT(!image.LoadHeaderFromMemory(truncated.data(), static_cast<int>(truncated.size())));

    for (DWORD fourCc : {kFourCC('D', 'X', 'T', '2'), kFourCC('D', 'X', 'T', '4'),
                         kFourCC('A', 'B', 'C', 'D')})
    {
        const std::vector<BYTE> dds = makeDds(fourCc, 4, 4, 1, 8, std::vector<BYTE>(8, 0));
        TEST_ASSERT(!image.LoadHeaderFromMemory(dds.data(), static_cast<int>(dds.size())));
    }

    const std::vector<BYTE> valid = makeDds(kFourCC('D', 'X', 'T', '5'), 4, 4, 1, 16,
                                            std::vector<BYTE>(16, 0));
    TEST_ASSERT(image.LoadHeaderFromMemory(valid.data(), static_cast<int>(valid.size())));
    TEST_ASSERT(image.m_CompFormat == PF_DXT5);
    TEST_ASSERT(image.m_dwMipMapCount == 1);
    TEST_ASSERT(image.m_pbCompBufferByLevels[0] == valid.data() + 128);
    TEST_ASSERT(std::strcmp(image.m_strFormat, "DXT5") == 0);

    TEST_PASS("rejects truncation, bad magic and unsupported fourCCs");
}

} // namespace

int main()
{
    test_dxt1_four_color_block();
    test_dxt1_three_color_transparent_block();
    test_dxt3_explicit_alpha();
    test_dxt5_eight_alpha_block();
    test_dxt5_six_alpha_block();
    test_dxt1_multiblock_image();
    test_mip_chain_offsets();
    test_header_validation();

    std::printf("[test_dxt_invariants] all checks passed\n");
    return 0;
}
