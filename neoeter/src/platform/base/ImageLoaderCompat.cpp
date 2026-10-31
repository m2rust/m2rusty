#include <Neoeter/Platform/Base/ImageLoaderCompat.h>
#include <cstring>
#include <algorithm>

#define STB_IMAGE_IMPLEMENTATION
#define STBI_NO_STDIO
#define STBI_NO_LINEAR
#define STBI_NO_HDR
#include <Platform/stb_image.h>

bool Platform_GetImageInfo(const void* data, size_t size, uint32_t* outW, uint32_t* outH, uint32_t* outFormat) {
    if (!data || size < 18) return false;

    // Check DDS magic "DDS " (0x20534444)
    const uint8_t* bytes = (const uint8_t*)data;
    if (bytes[0] == 'D' && bytes[1] == 'D' && bytes[2] == 'S' && bytes[3] == ' ') {
        uint32_t h = *(const uint32_t*)(bytes + 12);
        uint32_t w = *(const uint32_t*)(bytes + 16);
        if (outW) *outW = w;
        if (outH) *outH = h;
        if (outFormat) {
            if (size >= 88 && bytes[84] == 'D' && bytes[85] == 'X' && bytes[86] == 'T') {
                if (bytes[87] == '1') *outFormat = 0x31545844; // D3DFMT_DXT1
                else if (bytes[87] == '3') *outFormat = 0x33545844; // D3DFMT_DXT3
                else if (bytes[87] == '5') *outFormat = 0x35545844; // D3DFMT_DXT5
                else *outFormat = 21; // D3DFMT_A8R8G8B8
            } else {
                *outFormat = 21; // D3DFMT_A8R8G8B8
            }
        }
        return true;
    }

    // Try stb_image
    int w = 0, h = 0, comp = 0;
    if (stbi_info_from_memory(bytes, (int)size, &w, &h, &comp)) {
        if (outW) *outW = (uint32_t)w;
        if (outH) *outH = (uint32_t)h;
        if (outFormat) *outFormat = (comp == 4) ? 21 : 20; // D3DFMT_A8R8G8B8 : D3DFMT_R8G8B8
        return true;
    }

    // Try TGA raw header fallback (bytes 12-15)
    uint16_t tgaW = *(const uint16_t*)(bytes + 12);
    uint16_t tgaH = *(const uint16_t*)(bytes + 14);
    if (tgaW > 0 && tgaW <= 4096 && tgaH > 0 && tgaH <= 4096) {
        if (outW) *outW = tgaW;
        if (outH) *outH = tgaH;
        if (outFormat) *outFormat = 21;
        return true;
    }

    return false;
}

bool Platform_DecodeImage(const void* data, size_t size, uint32_t* outW, uint32_t* outH, std::vector<uint8_t>& outPixels) {
    if (!data || size < 4) return false;

    int w = 0, h = 0, comp = 0;
    stbi_uc* pixels = stbi_load_from_memory((const stbi_uc*)data, (int)size, &w, &h, &comp, 4);
    if (!pixels) return false;

    if (outW) *outW = (uint32_t)w;
    if (outH) *outH = (uint32_t)h;

    outPixels.resize(w * h * 4);
    for (int i = 0; i < w * h; ++i) {
        outPixels[i * 4 + 0] = pixels[i * 4 + 2]; // B
        outPixels[i * 4 + 1] = pixels[i * 4 + 1]; // G
        outPixels[i * 4 + 2] = pixels[i * 4 + 0]; // R
        outPixels[i * 4 + 3] = pixels[i * 4 + 3]; // A
    }

    stbi_image_free(pixels);
    return true;
}
