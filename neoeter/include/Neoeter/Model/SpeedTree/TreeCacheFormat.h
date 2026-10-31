#pragma once

// On-disk format of the baked SpeedTree geometry cache.
//
// A cache file is named "<fnv1a32-of-the-spt-bytes>.tree" (8 lowercase hex
// digits) and stores the geometry produced by the original SpeedTreeRT SDK for
// one .spt asset. The authoritative writer is tools/speedtree_baker; the reader
// is CSpeedTreeRT::LoadBakedGeometry.
//
// Layout (little-endian):
//   u32 magic
//   u32 spt hash (must match the file name)
//   f32 bbox[6]                  min xyz, max xyz
//   u32 collisionCount
//     per collision: i32 type, f32 pos[3], f32 dims[3]
//   indexed branches             (see below)
//   indexed fronds               (see below)
//   leaf set 0                   (see below)
//   leaf set 1                   (see below)
//   billboard 0                  (see below)
//   billboard 1                  (see below)
//   texture names: 3 x (u32 length, bytes)
//                                 branch, composite, self-shadow
//
// Indexed:
//   u32 vertexCount, u32 stripCount, u16 lodLevel, f32 alphaTest,
//   u32 indexCount, u16 indices[indexCount],
//   f32 coords[vertexCount*3], u32 colors[vertexCount],
//   f32 texCoords0[vertexCount*2], f32 texCoords1[vertexCount*2],
//   f32 normals[vertexCount*3]
//
// Leaf set:
//   u8 active, i32 lodLevel, f32 alphaTest, u32 leafCount,
//   f32 centers[leafCount*3], u32 colors[leafCount],
//   per leaf: f32 corners[16] (4 x xyz+w), f32 texCoords[8] (4 x uv),
//             u8 mapIndex, u8 clusterIndex
//
// Billboard:
//   u8 active, f32 alphaTest, f32 coords[12], f32 texCoords[8]
//
// This header is deliberately freestanding (no CRT) so the 32-bit Windows
// baker can include it while running as a no-CRT PE.

#include <stddef.h>
#include <stdint.h>

namespace TreeCache
{
constexpr uint32_t kMagic = 0x4D543254u; // 'MT2T' in little-endian
// Layout identifier, not a serialized field in v1. Bump it (and the magic, or
// add a version field) if the record order below ever changes.
constexpr uint32_t kVersion = 1;
constexpr uint32_t kAllParts = 15u;
constexpr uint32_t kInvalidColor = 0xFFFFFFFFu;
constexpr size_t kHashHexDigits = 8;

// Bits accepted by SpeedTreeRT::GetGeometry for the part selection mask.
enum PartMask : uint32_t
{
    kPartBranches = 1u << 0,
    kPartFronds = 1u << 1,
    kPartLeaves = 1u << 2,
    kPartBillboards = 1u << 3
};

inline uint32_t Fnv1a32(const unsigned char* pData, size_t nSize)
{
    uint32_t nHash = 2166136261u;
    for (size_t i = 0; i < nSize; ++i)
    {
        nHash ^= pData[i];
        nHash *= 16777619u;
    }
    return nHash;
}

// Writes the 8 hex digits of nHash plus a NUL into name[9].
inline void FormatHash(uint32_t nHash, char (&name)[kHashHexDigits + 1])
{
    static const char s_hex[] = "0123456789abcdef";
    for (int i = static_cast<int>(kHashHexDigits) - 1; i >= 0; --i)
    {
        name[i] = s_hex[nHash & 0xFu];
        nHash >>= 4;
    }
    name[kHashHexDigits] = '\0';
}

} // namespace TreeCache
