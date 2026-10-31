#include <support/TestAssert.h>

#include <cstdint>
#include <cstring>
#include <vector>

#include <Neoeter/Platform/Base/ImageLoaderCompat.h>

// Synthetic 2x2 RGBA PNG (red, green / blue, white). Generated in-repo so no
// game asset is needed and the decoder has a stable fixture (issue #79).
static const unsigned char kPng2x2[] = {
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d,
    0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x02,
    0x08, 0x06, 0x00, 0x00, 0x00, 0x72, 0xb6, 0x0d, 0x24, 0x00, 0x00, 0x00,
    0x12, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0xf8, 0xcf, 0xc0, 0xf0,
    0x1f, 0x0c, 0x81, 0x34, 0x18, 0x00, 0x00, 0x49, 0xc8, 0x09, 0xf7, 0x03,
    0xd9, 0x64, 0xf1, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae,
    0x42, 0x60, 0x82,
};

// Minimal 128-byte DDS header: 64x32, DXT5 fourCC at byte 84.
static std::vector<unsigned char> MakeDdsHeader(uint32_t width, uint32_t height,
                                                const char fourCC[4]) {
    std::vector<unsigned char> dds(128, 0);
    std::memcpy(dds.data(), "DDS ", 4);
    const uint32_t headerSize = 124;
    std::memcpy(dds.data() + 4, &headerSize, 4);
    std::memcpy(dds.data() + 12, &height, 4);
    std::memcpy(dds.data() + 16, &width, 4);
    std::memcpy(dds.data() + 84, fourCC, 4);
    return dds;
}

int main() {
    TEST_SECTION("Platform_GetImageInfo / Platform_DecodeImage");

    // PNG: stb_image reports 2x2 RGBA and D3DFMT_A8R8G8B8.
    {
        uint32_t w = 0, h = 0, format = 0;
        TEST_ASSERT(Platform_GetImageInfo(kPng2x2, sizeof(kPng2x2), &w, &h, &format));
        TEST_ASSERT(w == 2);
        TEST_ASSERT(h == 2);
        TEST_ASSERT(format == 21);

        std::vector<uint8_t> pixels;
        TEST_ASSERT(Platform_DecodeImage(kPng2x2, sizeof(kPng2x2), &w, &h, pixels));
        TEST_ASSERT(w == 2);
        TEST_ASSERT(h == 2);
        TEST_ASSERT(pixels.size() == 2u * 2u * 4u);

        // The compat layer stores BGRA. Pixel 0 is red in the fixture.
        TEST_ASSERT(pixels[0] == 0 && pixels[1] == 0 && pixels[2] == 255 && pixels[3] == 255);
        // Pixel 1 is green.
        TEST_ASSERT(pixels[4] == 0 && pixels[5] == 255 && pixels[6] == 0 && pixels[7] == 255);
    }

    // DDS: header-only parse, no decoding required.
    {
        const std::vector<unsigned char> dds = MakeDdsHeader(64, 32, "DXT5");
        uint32_t w = 0, h = 0, format = 0;
        TEST_ASSERT(Platform_GetImageInfo(dds.data(), dds.size(), &w, &h, &format));
        TEST_ASSERT(w == 64);
        TEST_ASSERT(h == 32);
        TEST_ASSERT(format == 0x35545844u); // D3DFMT_DXT5
    }

    // Garbage must be rejected instead of returning stale dimensions.
    {
        const std::vector<unsigned char> junk(64, 0x7f);
        uint32_t w = 123, h = 456, format = 789;
        TEST_ASSERT(!Platform_GetImageInfo(junk.data(), junk.size(), &w, &h, &format));
        std::vector<uint8_t> pixels;
        TEST_ASSERT(!Platform_DecodeImage(junk.data(), junk.size(), &w, &h, pixels));
    }

    return 0;
}
