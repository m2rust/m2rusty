#include <Neoeter/Image/StdAfx.h>

#include <algorithm>
#include <cassert>
#include <cstdint>
#include <cstdlib>
#include <cstring>

#if __has_include(<Neoeter/Platform/Base/MappedFile.h>)
#include <Neoeter/Platform/Base/Debug.h>
#include <Neoeter/Platform/Base/MappedFile.h>
#else
#define Tracef(...)                                                            \
  do {                                                                         \
    fprintf(stdout, __VA_ARGS__);                                              \
  } while (0)
#endif

#include <Neoeter/Image/DXTCImage.h>

// Portable 32-bit compatible DDS header structures & flags (124-byte
// DDSURFACEDESC2)
#ifndef DDSD_CAPS
#define DDSD_CAPS 0x00000001l
#define DDSD_HEIGHT 0x00000002l
#define DDSD_WIDTH 0x00000004l
#define DDSD_PITCH 0x00000008l
#define DDSD_PIXELFORMAT 0x00001000l
#define DDSD_MIPMAPCOUNT 0x00020000l
#define DDSD_LINEARSIZE 0x00080000l
#define DDSD_DEPTH 0x00800000l
#endif

#ifndef DDPF_ALPHAPIXELS
#define DDPF_ALPHAPIXELS 0x00000001l
#define DDPF_ALPHA 0x00000002l
#define DDPF_FOURCC 0x00000004l
#define DDPF_RGB 0x00000040l
#define DDPF_ALPHAPREMULT 0x00008000l
#endif

#pragma pack(push, 1)
typedef struct _DDCOLORKEY_COMPAT {
  DWORD dwColorSpaceLowValue;
  DWORD dwColorSpaceHighValue;
} DDCOLORKEY_COMPAT;

typedef struct _DDSCAPS2_COMPAT {
  DWORD dwCaps;
  DWORD dwCaps2;
  DWORD dwCaps3;
  DWORD dwCaps4;
} DDSCAPS2_COMPAT;

typedef struct _DDSURFACEDESC2_COMPAT {
  DWORD dwSize;
  DWORD dwFlags;
  DWORD dwHeight;
  DWORD dwWidth;
  union {
    LONG lPitch;
    DWORD dwLinearSize;
  } DUMMYUNIONNAMEN(1);
  DWORD dwBackBufferCount;
  union {
    DWORD dwMipMapCount;
    DWORD dwRefreshRate;
  } DUMMYUNIONNAMEN(2);
  DWORD dwAlphaBitDepth;
  DWORD dwReserved;
  DWORD lpSurface; // 32-bit pointer field in on-disk DDS header
  union {
    DDCOLORKEY_COMPAT ddckCKDestOverlay;
    DWORD dwEmptyFaceColor;
  } DUMMYUNIONNAMEN(3);
  DDCOLORKEY_COMPAT ddckCKDestBlt;
  DDCOLORKEY_COMPAT ddckCKSrcOverlay;
  DDCOLORKEY_COMPAT ddckCKSrcBlt;
  XDDPIXELFORMAT ddpfPixelFormat;
  DDSCAPS2_COMPAT ddsCaps;
  DWORD dwTextureStage;
} DDSURFACEDESC2;
#pragma pack(pop)

static_assert(sizeof(DDSURFACEDESC2) == 124,
              "DDSURFACEDESC2 must be exactly 124 bytes to match on-disk DDS "
              "header format");

struct DXTColBlock {
  WORD col0;
  WORD col1;

  // no bit fields - use bytes
  BYTE row[4];
};

struct DXTAlphaBlockExplicit {
  WORD row[4];
};

struct DXTAlphaBlock3BitLinear {
  BYTE alpha0;
  BYTE alpha1;

  BYTE stuff[6];
};

// use cast to struct instead of RGBA_MAKE as struct is much
struct Color8888 {
  BYTE b; // Last one is MSB, 1st is LSB.
  BYTE g; // order of the output ARGB or BGRA, etc...
  BYTE r; // change the order of names to change the
  BYTE a;
};

struct Color565 {
  unsigned nBlue : 5;  // order of names changes
  unsigned nGreen : 6; //  byte order of output to 32 bit
  unsigned nRed : 5;
};

/////////////////////////////////////
// should be in ddraw.h
#ifndef MAKEFOURCC
#define MAKEFOURCC(ch0, ch1, ch2, ch3)                                         \
  ((DWORD)(BYTE)(ch0) | ((DWORD)(BYTE)(ch1) << 8) |                            \
   ((DWORD)(BYTE)(ch2) << 16) | ((DWORD)(BYTE)(ch3) << 24))
#endif // defined(MAKEFOURCC)

CDXTCImage::CDXTCImage() { Initialize(); }

CDXTCImage::~CDXTCImage() {}

void CDXTCImage::Initialize() {
  m_nWidth = 0;
  m_nHeight = 0;

  for (int i = 0; i < MAX_MIPLEVELS; ++i)
    m_pbCompBufferByLevels[i] = NULL;
}

void CDXTCImage::Clear() {
  for (int i = 0; i < MAX_MIPLEVELS; ++i)
    m_bCompVector[i].clear();

  Initialize();
}

bool CDXTCImage::LoadHeaderFromMemory(const BYTE *c_pbMap, int iSize) {
  //////////////////////////////////////
  // start reading the file
  // from Microsoft's mssdk D3DIM example "Compress"
  DWORD dwMagic;

  // Read magic number
  if (iSize < static_cast<int>(sizeof(DWORD)))
    return false;

  memcpy(&dwMagic, c_pbMap, sizeof(DWORD));
  c_pbMap += sizeof(DWORD);
  iSize -= sizeof(DWORD);

  // Check whether the magic header is correct
  if (dwMagic != MAKEFOURCC('D', 'D', 'S', ' '))
    return false;

  DDSURFACEDESC2 ddsd; // read from dds file

  // Read the surface description
  if (iSize < static_cast<int>(sizeof(DDSURFACEDESC2)))
    return false;

  memcpy(&ddsd, c_pbMap, sizeof(DDSURFACEDESC2));
  c_pbMap += sizeof(DDSURFACEDESC2);
  iSize -= sizeof(DDSURFACEDESC2);

  // Does texture have mipmaps?
  m_bMipTexture = (ddsd.dwMipMapCount > 0) ? TRUE : FALSE;

  // Is it DXTC ?
  // I sure hope pixelformat is valid!
  m_xddPixelFormat.dwFlags = ddsd.ddpfPixelFormat.dwFlags;
  m_xddPixelFormat.dwFourCC = ddsd.ddpfPixelFormat.dwFourCC;
  m_xddPixelFormat.dwSize = ddsd.ddpfPixelFormat.dwSize;
  m_xddPixelFormat.dwRGBBitCount = ddsd.ddpfPixelFormat.dwRGBBitCount;
  m_xddPixelFormat.dwRGBAlphaBitMask = ddsd.ddpfPixelFormat.dwRGBAlphaBitMask;
  m_xddPixelFormat.dwRBitMask = ddsd.ddpfPixelFormat.dwRBitMask;
  m_xddPixelFormat.dwGBitMask = ddsd.ddpfPixelFormat.dwGBitMask;
  m_xddPixelFormat.dwBBitMask = ddsd.ddpfPixelFormat.dwBBitMask;

  DecodePixelFormat(m_strFormat, &m_xddPixelFormat);

  if (m_CompFormat != PF_DXT1 && m_CompFormat != PF_DXT3 &&
      m_CompFormat != PF_DXT5) {
    return false;
  }

  if (ddsd.dwMipMapCount > MAX_MIPLEVELS)
    ddsd.dwMipMapCount = MAX_MIPLEVELS;

  m_nWidth = ddsd.dwWidth;
  m_nHeight = ddsd.dwHeight;
  //!@#
  m_dwMipMapCount = std::max<DWORD>(1, ddsd.dwMipMapCount);
  m_dwFlags = ddsd.dwFlags;

  if (ddsd.dwFlags & DDSD_PITCH) {
    m_lPitch = ddsd.lPitch;
    m_pbCompBufferByLevels[0] = c_pbMap;
  } else {
    m_lPitch = ddsd.dwLinearSize;

    if (ddsd.dwFlags & DDSD_MIPMAPCOUNT) {
      for (DWORD dwLinearSize = ddsd.dwLinearSize, i = 0; i < m_dwMipMapCount;
           ++i, dwLinearSize >>= 2) {
        m_pbCompBufferByLevels[i] = c_pbMap;
        c_pbMap += dwLinearSize;
      }
    } else {
      m_pbCompBufferByLevels[0] = c_pbMap;
    }
  }

  return true;
}

bool CDXTCImage::Copy(int miplevel, BYTE *pbDest, long lDestPitch) {
  if (!(m_dwFlags & DDSD_MIPMAPCOUNT))
    if (miplevel)
      return false;

  memcpy(pbDest, m_pbCompBufferByLevels[miplevel], m_lPitch >> (miplevel * 2));
  pbDest += lDestPitch;
  return true;
}

void CDXTCImage::Unextract(BYTE *pbDest, int /*iWidth*/, int /*iHeight*/,
                           int iPitch) {
  if (!m_pbCompBufferByLevels[0])
    return;

  DXTColBlock *pBlock;
  BYTE *pPos = (BYTE *)&m_pbCompBufferByLevels[0][0];
  int xblocks = m_nWidth / 4;
  int yblocks = (m_nHeight / 4) * ((iPitch / m_nWidth) / 2);

  for (int y = 0; y < yblocks; ++y) {
    pBlock = (DXTColBlock *)(pPos + y * xblocks * 8);

    memcpy(pbDest, pBlock, xblocks * 8);
    pbDest += xblocks * 8;
  }
}

void CDXTCImage::Decompress(int miplevel, DWORD *pdwDest) {
  switch (m_CompFormat) {
  case PF_DXT1:
    DecompressDXT1(miplevel, pdwDest);
    break;

  case PF_DXT3:
    DecompressDXT3(miplevel, pdwDest);
    break;

  case PF_DXT5:
    DecompressDXT5(miplevel, pdwDest);
    break;

  case PF_ARGB:
    DecompressARGB(miplevel, pdwDest);
    break;

  case PF_DXT2:
  case PF_DXT4:
  case PF_UNKNOWN:
    break;
  }
}

inline DWORD Color8888ToDWORD(const Color8888 &col) {
  DWORD dw;
  memcpy(&dw, &col, sizeof(DWORD));
  return dw;
}

inline void GetColorBlockColors(DXTColBlock *pBlock, Color8888 *col_0,
                                Color8888 *col_1, Color8888 *col_2,
                                Color8888 *col_3, WORD &wrd) {
  WORD rawCol0, rawCol1;
  memcpy(&rawCol0, &pBlock->col0, sizeof(WORD));
  memcpy(&rawCol1, &pBlock->col1, sizeof(WORD));

  col_0->a = 0xff;
  col_0->r = (BYTE)(((rawCol0 >> 11) & 0x1f) << 3); // shift to full precision
  col_0->g = (BYTE)(((rawCol0 >> 5) & 0x3f) << 2);
  col_0->b = (BYTE)((rawCol0 & 0x1f) << 3);

  col_1->a = 0xff;
  col_1->r = (BYTE)(((rawCol1 >> 11) & 0x1f) << 3); // shift to full precision
  col_1->g = (BYTE)(((rawCol1 >> 5) & 0x3f) << 2);
  col_1->b = (BYTE)((rawCol1 & 0x1f) << 3);

  if (rawCol0 > rawCol1) {
    // Four-color block: derive the other two colors.
    // 00 = color_0, 01 = color_1, 10 = color_2, 11 = color_3
    wrd = (WORD)(((WORD)col_0->r * 2 + (WORD)col_1->r) / 3);
    col_2->r = (BYTE)wrd;

    wrd = (WORD)(((WORD)col_0->g * 2 + (WORD)col_1->g) / 3);
    col_2->g = (BYTE)wrd;

    wrd = (WORD)(((WORD)col_0->b * 2 + (WORD)col_1->b) / 3);
    col_2->b = (BYTE)wrd;
    col_2->a = 0xff;

    wrd = (WORD)(((WORD)col_0->r + (WORD)col_1->r * 2) / 3);
    col_3->r = (BYTE)wrd;

    wrd = (WORD)(((WORD)col_0->g + (WORD)col_1->g * 2) / 3);
    col_3->g = (BYTE)wrd;

    wrd = (WORD)(((WORD)col_0->b + (WORD)col_1->b * 2) / 3);
    col_3->b = (BYTE)wrd;
    col_3->a = 0xff;
  } else {
    // Three-color block: derive the other color.
    // 00 = color_0,  01 = color_1,  10 = color_2,
    // 11 = transparent.
    wrd = (WORD)(((WORD)col_0->r + (WORD)col_1->r) / 2);
    col_2->r = (BYTE)wrd;
    wrd = (WORD)(((WORD)col_0->g + (WORD)col_1->g) / 2);
    col_2->g = (BYTE)wrd;
    wrd = (WORD)(((WORD)col_0->b + (WORD)col_1->b) / 2);
    col_2->b = (BYTE)wrd;
    col_2->a = 0xff;

    col_3->r = 0x00; // random color to indicate alpha
    col_3->g = 0x00;
    col_3->b = 0x00;
    col_3->a = 0x00;
  }
}

inline void DecodeColorBlock(DWORD *pImPos, DXTColBlock *pColorBlock, int width,
                             DWORD *col_0, DWORD *col_1, DWORD *col_2,
                             DWORD *col_3) {
  // width is width of image in pixels
  DWORD bits;
  int y, n;

  DWORD c0, c1, c2, c3;
  memcpy(&c0, col_0, sizeof(DWORD));
  memcpy(&c1, col_1, sizeof(DWORD));
  memcpy(&c2, col_2, sizeof(DWORD));
  memcpy(&c3, col_3, sizeof(DWORD));

  // bit masks = 00000011, 00001100, 00110000, 11000000
  const DWORD masks[] = {3, 12, 3 << 4, 3 << 6};
  const int shift[] = {0, 2, 4, 6};

  // r steps through lines in y
  for (y = 0; y < 4;
       ++y, pImPos += width - 4) // no width * 4 as DWORD ptr inc will * 4
  {
    // n steps through pixels
    for (n = 0; n < 4; ++n) {
      bits = pColorBlock->row[y] & masks[n];
      bits >>= shift[n];

      switch (bits) {
      case 0:
        *pImPos = c0;
        pImPos++; // increment to next DWORD
        break;

      case 1:
        *pImPos = c1;
        pImPos++;
        break;

      case 2:
        *pImPos = c2;
        pImPos++;
        break;

      case 3:
        *pImPos = c3;
        pImPos++;
        break;

      default:
        Tracef("Your logic is jacked! bits == 0x%x\n", bits);
        pImPos++;
        break;
      }
    }
  }
}

inline void DecodeAlphaExplicit(DWORD *pImPos,
                                DXTAlphaBlockExplicit *pAlphaBlock, int width,
                                DWORD alphazero) {
  // decodes to 32 bit format only
  int row, pix;

  WORD wrd;

  Color8888 col;
  col.r = col.g = col.b = 0;

  for (row = 0; row < 4; row++, pImPos += width - 4) {
    memcpy(&wrd, &pAlphaBlock->row[row], sizeof(WORD));

    for (pix = 0; pix < 4; ++pix) {
      // zero the alpha bits of image pixel
      *pImPos &= alphazero;

      col.a = (BYTE)(wrd & 0x000f); // get only low 4 bits
      col.a =
          (BYTE)(col.a |
                 (col.a << 4)); // This allows max 4 bit alpha to be 0xff alpha

      *pImPos |=
          Color8888ToDWORD(col); // or the bits into the prev. nulled alpha

      wrd >>= 4; // move next bits to lowest 4

      pImPos++; // move to next pixel in the row
    }
  }
}

static BYTE gBits[4][4];
static WORD gAlphas[8];
static Color8888 gACol[4][4];

inline void DecodeAlpha3BitLinear(DWORD *pImPos,
                                  DXTAlphaBlock3BitLinear *pAlphaBlock,
                                  int width, DWORD alphazero) {
  gAlphas[0] = pAlphaBlock->alpha0;
  gAlphas[1] = pAlphaBlock->alpha1;

  // 8-alpha or 6-alpha block?
  if (gAlphas[0] > gAlphas[1]) {
    // 8-alpha block:  derive the other 6 alphas.
    // 000 = alpha_0, 001 = alpha_1, others are interpolated
    gAlphas[2] = (WORD)((6 * gAlphas[0] + gAlphas[1]) / 7);     // Bit code 010
    gAlphas[3] = (WORD)((5 * gAlphas[0] + 2 * gAlphas[1]) / 7); // Bit code 011
    gAlphas[4] = (WORD)((4 * gAlphas[0] + 3 * gAlphas[1]) / 7); // Bit code 100
    gAlphas[5] = (WORD)((3 * gAlphas[0] + 4 * gAlphas[1]) / 7); // Bit code 101
    gAlphas[6] = (WORD)((2 * gAlphas[0] + 5 * gAlphas[1]) / 7); // Bit code 110
    gAlphas[7] = (WORD)((gAlphas[0] + 6 * gAlphas[1]) / 7);     // Bit code 111
  } else {
    // 6-alpha block:  derive the other alphas.
    // 000 = alpha_0, 001 = alpha_1, others are interpolated
    gAlphas[2] = (WORD)((4 * gAlphas[0] + gAlphas[1]) / 5);     // Bit code 010
    gAlphas[3] = (WORD)((3 * gAlphas[0] + 2 * gAlphas[1]) / 5); // Bit code 011
    gAlphas[4] = (WORD)((2 * gAlphas[0] + 3 * gAlphas[1]) / 5); // Bit code 100
    gAlphas[5] = (WORD)((gAlphas[0] + 4 * gAlphas[1]) / 5);     // Bit code 101
    gAlphas[6] = 0;                                             // Bit code 110
    gAlphas[7] = 255;                                           // Bit code 111
  }

  // Decode 3-bit fields into array of 16 BYTES with same value
  const DWORD mask = 0x00000007; // bits = 00 00 01 11
  DWORD bits = 0;
  memcpy(&bits, &(pAlphaBlock->stuff[0]), 3);

  gBits[0][0] = (BYTE)(bits & mask);
  bits >>= 3;
  gBits[0][1] = (BYTE)(bits & mask);
  bits >>= 3;
  gBits[0][2] = (BYTE)(bits & mask);
  bits >>= 3;
  gBits[0][3] = (BYTE)(bits & mask);
  bits >>= 3;
  gBits[1][0] = (BYTE)(bits & mask);
  bits >>= 3;
  gBits[1][1] = (BYTE)(bits & mask);
  bits >>= 3;
  gBits[1][2] = (BYTE)(bits & mask);
  bits >>= 3;
  gBits[1][3] = (BYTE)(bits & mask);

  // now for last two rows:
  bits = 0;
  memcpy(&bits, &(pAlphaBlock->stuff[3]), 3); // last 3 bytes

  gBits[2][0] = (BYTE)(bits & mask);
  bits >>= 3;
  gBits[2][1] = (BYTE)(bits & mask);
  bits >>= 3;
  gBits[2][2] = (BYTE)(bits & mask);
  bits >>= 3;
  gBits[2][3] = (BYTE)(bits & mask);
  bits >>= 3;
  gBits[3][0] = (BYTE)(bits & mask);
  bits >>= 3;
  gBits[3][1] = (BYTE)(bits & mask);
  bits >>= 3;
  gBits[3][2] = (BYTE)(bits & mask);
  bits >>= 3;
  gBits[3][3] = (BYTE)(bits & mask);

  // decode the codes into alpha values
  int row, pix;

  for (row = 0; row < 4; ++row) {
    for (pix = 0; pix < 4; ++pix) {
      gACol[row][pix].a = (BYTE)gAlphas[gBits[row][pix]];

      assert(gACol[row][pix].r == 0);
      assert(gACol[row][pix].g == 0);
      assert(gACol[row][pix].b == 0);
    }
  }

  // Write out alpha values to the image bits
  for (row = 0; row < 4; ++row, pImPos += width - 4) {
    for (pix = 0; pix < 4; ++pix) {
      // zero the alpha bits of image pixel
      *pImPos &= alphazero;
      *pImPos |= Color8888ToDWORD(
          gACol[row][pix]); // or the bits into the prev. nulled alpha
      pImPos++;
    }
  }
}

void CDXTCImage::DecompressDXT1(int miplevel, DWORD *pdwDest) {
  int xblocks, yblocks;
  UINT nWidth = m_nWidth >> miplevel;
  UINT nHeight = m_nHeight >> miplevel;

  xblocks = nWidth / 4;
  yblocks = nHeight / 4;

  int x, y;
  DWORD *pBase = (DWORD *)pdwDest;
  WORD *pPos = (WORD *)&m_bCompVector[miplevel][0]; // pos in compressed data
  DWORD *pImPos;

  DXTColBlock *pBlock;

  Color8888 col_0, col_1, col_2, col_3;
  WORD wrd;

  for (y = 0; y < yblocks; ++y) {
    // 8 bytes per block
    pBlock = (DXTColBlock *)((uintptr_t)pPos + y * xblocks * 8);

    for (x = 0; x < xblocks; ++x, ++pBlock) {
      GetColorBlockColors(pBlock, &col_0, &col_1, &col_2, &col_3, wrd);

      pImPos = (DWORD *)((uintptr_t)pBase + x * 16 + (y * 4) * nWidth * 4);
      DecodeColorBlock(pImPos, pBlock, nWidth, (DWORD *)&col_0, (DWORD *)&col_1,
                       (DWORD *)&col_2, (DWORD *)&col_3);
    }
  }
}

void CDXTCImage::DecompressDXT3(int miplevel, DWORD *pdwDest) {
  int xblocks, yblocks;
  UINT nWidth = m_nWidth >> miplevel;
  UINT nHeight = m_nHeight >> miplevel;

  xblocks = nWidth / 4;
  yblocks = nHeight / 4;

  int x, y;
  DWORD *pBase = (DWORD *)pdwDest;
  WORD *pPos = (WORD *)&m_bCompVector[miplevel][0]; // pos in compressed data
  DWORD *pImPos;                                    // pos in decompressed data

  DXTColBlock *pBlock;
  DXTAlphaBlockExplicit *pAlphaBlock;

  Color8888 col_0, col_1, col_2, col_3;
  WORD wrd;

  col_0.a = 0;
  col_0.r = col_0.g = col_0.b = 0xff;

  DWORD alphazero = Color8888ToDWORD(col_0);

  for (y = 0; y < yblocks; ++y) {
    // 16 bytes per block (8 alpha + 8 color)
    pBlock = (DXTColBlock *)((uintptr_t)(pPos + y * xblocks * 8));

    for (x = 0; x < xblocks; ++x, ++pBlock) {
      // Get alpha block
      pAlphaBlock = (DXTAlphaBlockExplicit *)pBlock;

      // Get color block & colors
      pBlock++;
      GetColorBlockColors(pBlock, &col_0, &col_1, &col_2, &col_3, wrd);

      // Decode the color block into the bitmap bits
      pImPos = (DWORD *)((uintptr_t)pBase + x * 16 + (y * 4) * nWidth * 4);

      DecodeColorBlock(pImPos, pBlock, nWidth, (DWORD *)&col_0, (DWORD *)&col_1,
                       (DWORD *)&col_2, (DWORD *)&col_3);

      // Overwrite the previous alpha bits with the alpha block info
      DecodeAlphaExplicit(pImPos, pAlphaBlock, nWidth, alphazero);
    }
  }
}

void CDXTCImage::DecompressDXT5(int level, DWORD *pdwDest) {
  int xblocks, yblocks;
  UINT nWidth = m_nWidth >> level;
  UINT nHeight = m_nHeight >> level;

  xblocks = nWidth / 4;
  yblocks = nHeight / 4;

  int x, y;

  DWORD *pBase = (DWORD *)pdwDest;
  WORD *pPos = (WORD *)&m_bCompVector[level][0]; // pos in compressed data
  DWORD *pImPos;                                 // pos in decompressed data

  DXTColBlock *pBlock;
  DXTAlphaBlock3BitLinear *pAlphaBlock;

  Color8888 col_0, col_1, col_2, col_3;
  WORD wrd;

  col_0.a = 0;
  col_0.r = col_0.g = col_0.b = 0xff;
  DWORD alphazero = Color8888ToDWORD(col_0);

  for (y = 0; y < yblocks; ++y) {
    // 16 bytes per block (8 alpha + 8 color)
    pBlock = (DXTColBlock *)((uintptr_t)(pPos + y * xblocks * 8));

    for (x = 0; x < xblocks; ++x, ++pBlock) {
      // Get alpha block
      pAlphaBlock = (DXTAlphaBlock3BitLinear *)pBlock;

      // Get color block & colors
      pBlock++;

      GetColorBlockColors(pBlock, &col_0, &col_1, &col_2, &col_3, wrd);

      // Decode the color block into the bitmap bits
      pImPos = (DWORD *)((uintptr_t)pBase + x * 16 + (y * 4) * nWidth * 4);

      DecodeColorBlock(pImPos, pBlock, nWidth, (DWORD *)&col_0, (DWORD *)&col_1,
                       (DWORD *)&col_2, (DWORD *)&col_3);

      // Overwrite the previous alpha bits with the alpha block info
      DecodeAlpha3BitLinear(pImPos, pAlphaBlock, nWidth, alphazero);
    }
  }
}

void CDXTCImage::DecompressARGB(int level, DWORD *pdwDest) {
  UINT lPitch = m_lPitch >> (level * 2);
  memcpy(pdwDest, &m_bCompVector[level][0], lPitch);
}

static WORD GetNumberOfBits(DWORD dwMask) {
  WORD wBits;
  for (wBits = 0; dwMask; wBits++)
    dwMask = (dwMask & (dwMask - 1));

  return wBits;
}

VOID CDXTCImage::DecodePixelFormat(CHAR *strPixelFormat,
                                   XDDPIXELFORMAT *pxddpf) {
  switch (pxddpf->dwFourCC) {
  case 0: {
    // This dds texture isn't compressed so write out ARGB format
    WORD a = GetNumberOfBits(pxddpf->dwRGBAlphaBitMask);
    WORD r = GetNumberOfBits(pxddpf->dwRBitMask);
    WORD g = GetNumberOfBits(pxddpf->dwGBitMask);
    WORD b = GetNumberOfBits(pxddpf->dwBBitMask);

    _snprintf(strPixelFormat, 31, "ARGB-%d%d%d%d%s", a, r, g, b,
              pxddpf->dwBBitMask & DDPF_ALPHAPREMULT ? "-premul" : "");
    m_CompFormat = PF_ARGB;
  } break;

  case MAKEFOURCC('D', 'X', 'T', '1'):
    strncpy(strPixelFormat, "DXT1", 31);
    m_CompFormat = PF_DXT1;
    break;

  case MAKEFOURCC('D', 'X', 'T', '2'):
    strncpy(strPixelFormat, "DXT2", 31);
    m_CompFormat = PF_DXT2;
    break;

  case MAKEFOURCC('D', 'X', 'T', '3'):
    strncpy(strPixelFormat, "DXT3", 31);
    m_CompFormat = PF_DXT3;
    break;

  case MAKEFOURCC('D', 'X', 'T', '4'):
    strncpy(strPixelFormat, "DXT4", 31);
    m_CompFormat = PF_DXT4;
    break;

  case MAKEFOURCC('D', 'X', 'T', '5'):
    strncpy(strPixelFormat, "DXT5", 31);
    m_CompFormat = PF_DXT5;
    break;

  default:
    strcpy(strPixelFormat, "Format Unknown");
    m_CompFormat = PF_UNKNOWN;
    break;
  }
}
