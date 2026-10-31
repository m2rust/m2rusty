#include "support/Png.h"

#include <Neoeter/Platform/Base/ImageLoaderCompat.h>

#include <zlib.h>

#include <algorithm>
#include <cstdio>
#include <cstring>

namespace render
{
namespace
{

void PutBigEndian32(std::vector<std::uint8_t>& out, std::uint32_t value)
{
    out.push_back(static_cast<std::uint8_t>((value >> 24) & 0xFF));
    out.push_back(static_cast<std::uint8_t>((value >> 16) & 0xFF));
    out.push_back(static_cast<std::uint8_t>((value >> 8) & 0xFF));
    out.push_back(static_cast<std::uint8_t>(value & 0xFF));
}

void AppendChunk(std::vector<std::uint8_t>& out, const char type[4],
                 const std::uint8_t* data, std::size_t size)
{
    PutBigEndian32(out, static_cast<std::uint32_t>(size));

    const std::size_t typeOffset = out.size();
    out.insert(out.end(), type, type + 4);
    if (size > 0)
        out.insert(out.end(), data, data + size);

    uLong crc = crc32(0L, Z_NULL, 0);
    crc = crc32(crc, out.data() + typeOffset, static_cast<uInt>(size + 4));
    PutBigEndian32(out, static_cast<std::uint32_t>(crc));
}

bool WriteAll(const std::string& path, const std::vector<std::uint8_t>& bytes, std::string* error)
{
    std::FILE* file = std::fopen(path.c_str(), "wb");
    if (!file)
    {
        if (error)
            *error = "cannot open '" + path + "' for writing";
        return false;
    }

    const bool ok = std::fwrite(bytes.data(), 1, bytes.size(), file) == bytes.size();
    const bool closed = std::fclose(file) == 0;
    if (!ok || !closed)
    {
        if (error)
            *error = "short write while saving '" + path + "'";
        return false;
    }
    return true;
}

} // namespace

bool WritePng(const std::string& path, const Image& image, std::string* error)
{
    if (!image.Valid())
    {
        if (error)
            *error = "refusing to write an empty image";
        return false;
    }

    // Raw scanlines with filter type 0. The scenes are small and already
    // deterministic, so the simplest filter keeps the encoder tiny.
    std::vector<std::uint8_t> raw;
    raw.reserve((image.width * 4 + 1) * image.height);
    for (int y = 0; y < image.height; ++y)
    {
        raw.push_back(0);
        const std::uint8_t* row = image.rgba.data() + static_cast<std::size_t>(y) * image.width * 4;
        raw.insert(raw.end(), row, row + image.width * 4);
    }

    uLongf compressedSize = compressBound(static_cast<uLong>(raw.size()));
    std::vector<std::uint8_t> compressed(compressedSize);
    const int zret = compress2(compressed.data(), &compressedSize, raw.data(),
                               static_cast<uLong>(raw.size()), Z_BEST_COMPRESSION);
    if (zret != Z_OK)
    {
        if (error)
            *error = "zlib compression failed";
        return false;
    }
    compressed.resize(compressedSize);

    std::vector<std::uint8_t> png;
    static const std::uint8_t signature[8] = { 0x89, 'P', 'N', 'G', 0x0D, 0x0A, 0x1A, 0x0A };
    png.insert(png.end(), signature, signature + sizeof(signature));

    std::uint8_t ihdr[13];
    ihdr[0] = static_cast<std::uint8_t>((image.width >> 24) & 0xFF);
    ihdr[1] = static_cast<std::uint8_t>((image.width >> 16) & 0xFF);
    ihdr[2] = static_cast<std::uint8_t>((image.width >> 8) & 0xFF);
    ihdr[3] = static_cast<std::uint8_t>(image.width & 0xFF);
    ihdr[4] = static_cast<std::uint8_t>((image.height >> 24) & 0xFF);
    ihdr[5] = static_cast<std::uint8_t>((image.height >> 16) & 0xFF);
    ihdr[6] = static_cast<std::uint8_t>((image.height >> 8) & 0xFF);
    ihdr[7] = static_cast<std::uint8_t>(image.height & 0xFF);
    ihdr[8] = 8;  // bit depth
    ihdr[9] = 6;  // colour type: RGBA
    ihdr[10] = 0; // deflate
    ihdr[11] = 0; // no filtering
    ihdr[12] = 0; // no interlace

    AppendChunk(png, "IHDR", ihdr, sizeof(ihdr));
    AppendChunk(png, "IDAT", compressed.data(), compressed.size());
    AppendChunk(png, "IEND", nullptr, 0);

    return WriteAll(path, png, error);
}

bool LoadPng(const std::string& path, Image* image, std::string* error)
{
    if (!image)
        return false;

    std::FILE* file = std::fopen(path.c_str(), "rb");
    if (!file)
    {
        if (error)
            *error = "cannot open '" + path + "'";
        return false;
    }

    std::vector<std::uint8_t> bytes;
    std::uint8_t buffer[16 * 1024];
    std::size_t read = 0;
    while ((read = std::fread(buffer, 1, sizeof(buffer), file)) > 0)
        bytes.insert(bytes.end(), buffer, buffer + read);
    std::fclose(file);

    std::uint32_t width = 0;
    std::uint32_t height = 0;
    std::vector<std::uint8_t> pixels;
    if (!Platform_DecodeImage(bytes.data(), bytes.size(), &width, &height, pixels))
    {
        if (error)
            *error = "'" + path + "' is not a decodable image";
        return false;
    }

    image->width = static_cast<int>(width);
    image->height = static_cast<int>(height);
    image->rgba = std::move(pixels);

    // Platform_DecodeImage returns BGRA (the client's D3D texture order), so
    // swap red and blue back to the RGBA layout used by the render target.
    for (std::size_t i = 0; i + 3 < image->rgba.size(); i += 4)
        std::swap(image->rgba[i], image->rgba[i + 2]);

    if (!image->Valid())
    {
        if (error)
            *error = "'" + path + "' decoded to an unexpected size";
        return false;
    }
    return true;
}

DiffStats CompareImages(const Image& expected, const Image& actual, int channelTolerance)
{
    DiffStats stats;
    if (!expected.Valid() || !actual.Valid() ||
        expected.width != actual.width || expected.height != actual.height)
        return stats;

    stats.totalPixels = static_cast<std::size_t>(expected.width) * expected.height;
    std::uint64_t totalError = 0;

    for (std::size_t i = 0; i < stats.totalPixels; ++i)
    {
        int pixelDelta = 0;
        for (int channel = 0; channel < 4; ++channel)
        {
            const int a = expected.rgba[i * 4 + channel];
            const int b = actual.rgba[i * 4 + channel];
            const int delta = a > b ? a - b : b - a;
            totalError += static_cast<std::uint64_t>(delta);
            if (delta > pixelDelta)
                pixelDelta = delta;
        }

        if (pixelDelta > stats.maxDelta)
            stats.maxDelta = pixelDelta;
        if (pixelDelta > channelTolerance)
            ++stats.differingPixels;
    }

    stats.similarity = 1.0 - static_cast<double>(stats.differingPixels) /
                                 static_cast<double>(stats.totalPixels);
    stats.meanAbsError = static_cast<double>(totalError) /
                         static_cast<double>(stats.totalPixels * 4);
    return stats;
}

std::uint64_t PixelHash(const Image& image)
{
    std::uint64_t hash = 1469598103934665603ULL; // FNV offset basis
    auto mix = [&hash](std::uint8_t byte) {
        hash ^= byte;
        hash *= 1099511628211ULL;
    };

    for (int shift = 24; shift >= 0; shift -= 8)
        mix(static_cast<std::uint8_t>((static_cast<std::uint32_t>(image.width) >> shift) & 0xFF));
    for (int shift = 24; shift >= 0; shift -= 8)
        mix(static_cast<std::uint8_t>((static_cast<std::uint32_t>(image.height) >> shift) & 0xFF));
    for (std::uint8_t byte : image.rgba)
        mix(byte);
    return hash;
}

std::string FormatHash(std::uint64_t hash)
{
    char buffer[17];
    std::snprintf(buffer, sizeof(buffer), "%016llx", static_cast<unsigned long long>(hash));
    return std::string(buffer);
}

} // namespace render
