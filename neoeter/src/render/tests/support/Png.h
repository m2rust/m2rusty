#pragma once

// ============================================================================
// Png.h - minimal PNG IO and image comparison helpers for the render goldens
// ============================================================================
// TA.4 needs two things the rest of the test suite does not:
//  1. a dependency-light PNG writer (zlib is already part of the image), and
//  2. a perceptual comparison so a driver update does not fail CI on a single
//     off-by-one channel while still catching real rendering regressions.
//
// Decoding reuses EterBase's stb_image wrapper (Platform_DecodeImage), which is
// the same decoder the client uses for its textures.

#include <cstddef>
#include <cstdint>
#include <string>
#include <vector>

namespace render
{

struct Image
{
    int width = 0;
    int height = 0;
    std::vector<std::uint8_t> rgba; // top-down, 4 bytes per pixel

    bool Valid() const
    {
        return width > 0 && height > 0 &&
               rgba.size() == static_cast<std::size_t>(width) * height * 4;
    }
};

bool WritePng(const std::string& path, const Image& image, std::string* error);
bool LoadPng(const std::string& path, Image* image, std::string* error);

struct DiffStats
{
    std::size_t totalPixels = 0;
    std::size_t differingPixels = 0;
    int maxDelta = 0;
    double meanAbsError = 0.0;
    double similarity = 0.0; // fraction of pixels within the tolerance
};

// A pixel counts as matching when every channel differs by at most
// `channelTolerance`. Similarity is reported over pixels, not channels, so the
// numbers line up with what a human sees in the diff.
DiffStats CompareImages(const Image& expected, const Image& actual, int channelTolerance);

// FNV-1a over width, height and the raw RGBA bytes. Stable across processes and
// platforms; used to pin the exact pixels in the golden manifest.
std::uint64_t PixelHash(const Image& image);
std::string FormatHash(std::uint64_t hash);

} // namespace render
