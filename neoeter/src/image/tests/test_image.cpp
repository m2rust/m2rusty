#ifdef NDEBUG
#undef NDEBUG
#endif

#include <Neoeter/Image/DXTCImage.h>
#include <Neoeter/Image/Image.h>
#include <Neoeter/Image/TGAImage.h>

#include <cassert>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <vector>

#define TEST_ASSERT(cond)                                                      \
  do {                                                                         \
    if (!(cond)) {                                                             \
      std::fprintf(stderr, "Assertion failed: %s (%s:%d)\n", #cond, __FILE__,  \
                   __LINE__);                                                  \
      std::abort();                                                            \
    }                                                                          \
  } while (0)

namespace fs = std::filesystem;

static std::vector<BYTE> BuildMinimalDXT1DDS(DWORD width, DWORD height) {
  // 4 bytes magic ("DDS ") + 124 bytes header + 8 bytes (one 4x4 DXT1 block)
  std::vector<BYTE> buf(4 + 124 + 8, 0);

  // Magic: "DDS "
  DWORD magic = MAKEFOURCC('D', 'D', 'S', ' ');
  memcpy(&buf[0], &magic, 4);

  // DDSURFACEDESC2 offsets:
  // 0: dwSize = 124
  // 4: dwFlags = DDSD_CAPS | DDSD_HEIGHT | DDSD_WIDTH | DDSD_PIXELFORMAT |
  // DDSD_LINEARSIZE (0x81007) 8: dwHeight 12: dwWidth 16: dwLinearSize = 8 24:
  // dwMipMapCount = 1 72: ddpfPixelFormat.dwSize = 32 76:
  // ddpfPixelFormat.dwFlags = DDPF_FOURCC (0x4) 80: ddpfPixelFormat.dwFourCC =
  // "DXT1"
  DWORD dwSize = 124;
  DWORD dwFlags = 0x00081007;
  DWORD dwLinearSize = 8;
  DWORD dwMipMapCount = 1;
  DWORD ddpfSize = 32;
  DWORD ddpfFlags = 0x4;
  DWORD ddpfFourCC = MAKEFOURCC('D', 'X', 'T', '1');

  BYTE *hdr = &buf[4];
  memcpy(hdr + 0, &dwSize, 4);
  memcpy(hdr + 4, &dwFlags, 4);
  memcpy(hdr + 8, &height, 4);
  memcpy(hdr + 12, &width, 4);
  memcpy(hdr + 16, &dwLinearSize, 4);
  memcpy(hdr + 24, &dwMipMapCount, 4);
  memcpy(hdr + 72, &ddpfSize, 4);
  memcpy(hdr + 76, &ddpfFlags, 4);
  memcpy(hdr + 80, &ddpfFourCC, 4);

  // DXT1 4x4 block:
  // col0 = 0xF800 (pure red in RGB565), col1 = 0x001F (pure blue in RGB565)
  // row[0..3] = 0x00 (all pixels select col_0 -> pure red 0xFFFF0000)
  WORD col0 = 0xF800;
  WORD col1 = 0x001F;
  BYTE *block = &buf[128];
  memcpy(block + 0, &col0, 2);
  memcpy(block + 2, &col1, 2);
  memset(block + 4, 0x00, 4);

  return buf;
}

int main() {
  printf("[test_image] Running EterImageLib unit tests...\n");

  // 1. Test CImage basic surface operations
  {
    CImage img;
    TEST_ASSERT(img.IsEmpty());
    img.Create(8, 8);
    TEST_ASSERT(!img.IsEmpty());
    TEST_ASSERT(img.GetWidth() == 8);
    TEST_ASSERT(img.GetHeight() == 8);

    img.Clear(0xFF112233);
    TEST_ASSERT(img.GetBasePointer()[0] == 0xFF112233);

    // PutImage & FlipTopToBottom
    CImage sub;
    sub.Create(2, 2);
    sub.Clear(0xFFAABBCC);
    img.PutImage(0, 0, &sub);
    TEST_ASSERT(img.GetLinePointer(0)[0] == 0xFFAABBCC);
    TEST_ASSERT(img.GetLinePointer(7)[0] == 0xFF112233);

    img.FlipTopToBottom();
    TEST_ASSERT(img.GetLinePointer(7)[0] == 0xFFAABBCC);
    TEST_ASSERT(img.GetLinePointer(0)[0] == 0xFF112233);
    printf("  [PASS] CImage creation, Clear, PutImage, and FlipTopToBottom\n");
  }

  // 2. Test CTGAImage Uncompressed & RLE Save/Load round-trip
  {
    fs::path tmpFile = fs::temp_directory_path() / "test_eterimage_rle.tga";

    CTGAImage tgaOut;
    tgaOut.Create(4, 4);
    tgaOut.SetAlphaChannel(true);
    tgaOut.SetCompressed(true); // RLE compressed TGA (type 10)
    // Set top-left origin bit (0x20) so Save/Load preserves row order directly
    tgaOut.GetHeader().desc |= IMAGEDESC_TOPLEFT;

    DWORD *pixels = tgaOut.GetBasePointer();
    for (int i = 0; i < 8; ++i)
      pixels[i] = 0xFFFF0000; // Run of 8 identical red pixels
    for (int i = 8; i < 16; ++i)
      pixels[i] = 0xFF00FF00 + i; // 8 distinct pixels

    const bool saved = tgaOut.SaveToDiskFile(tmpFile.string().c_str());
    TEST_ASSERT(saved);

    std::ifstream f(tmpFile, std::ios::binary);
    std::vector<BYTE> fileBytes((std::istreambuf_iterator<char>(f)),
                                std::istreambuf_iterator<char>());
    f.close();
    TEST_ASSERT(!fileBytes.empty());

    CTGAImage tgaIn;
    const bool loaded = tgaIn.LoadFromMemory(static_cast<int>(fileBytes.size()),
                                             fileBytes.data());
    TEST_ASSERT(loaded);
    TEST_ASSERT(tgaIn.GetWidth() == 4);
    TEST_ASSERT(tgaIn.GetHeight() == 4);

    DWORD *loadedPixels = tgaIn.GetBasePointer();
    for (int i = 0; i < 8; ++i)
      TEST_ASSERT(loadedPixels[i] == 0xFFFF0000);
    for (int i = 8; i < 16; ++i)
      TEST_ASSERT(loadedPixels[i] == static_cast<DWORD>(0xFF00FF00 + i));

    fs::remove(tmpFile);
    printf("  [PASS] CTGAImage RLE compression & decompression round-trip\n");
  }

  // 3. Test CDXTCImage 64-bit DDS header parsing & DXT1 block decompression
  {
    std::vector<BYTE> ddsBytes = BuildMinimalDXT1DDS(4, 4);

    CDXTCImage dxt;
    const bool hdrLoaded = dxt.LoadHeaderFromMemory(
        ddsBytes.data(), static_cast<int>(ddsBytes.size()));
    TEST_ASSERT(hdrLoaded);
    TEST_ASSERT(dxt.m_CompFormat == PF_DXT1);
    TEST_ASSERT(dxt.m_nWidth == 4);
    TEST_ASSERT(dxt.m_nHeight == 4);

    // Populate m_bCompVector[0] and decompress on 64-bit address space
    dxt.m_bCompVector[0].assign(dxt.m_pbCompBufferByLevels[0],
                                dxt.m_pbCompBufferByLevels[0] + 8);
    std::vector<DWORD> decompressed(4 * 4, 0);
    dxt.Decompress(0, decompressed.data());

    // RGB565 0xF800 -> R = (0x1F << 3) = 0xF8, G = 0, B = 0, A = 0xFF ->
    // 0xFFF80000
    for (int i = 0; i < 16; ++i) {
      TEST_ASSERT(decompressed[i] == 0xFFF80000);
    }
    printf(
        "  [PASS] CDXTCImage 64-bit DDS header & DXT1 block decompression\n");
  }

  printf("[test_image] All tests passed successfully!\n");
  return 0;
}
