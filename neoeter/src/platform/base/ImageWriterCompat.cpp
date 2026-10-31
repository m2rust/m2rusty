#include <Neoeter/Platform/Base/ImageLoaderCompat.h>

#include <vector>

// stb_image_write is the writer counterpart of the stb_image reader used by
// ImageLoaderCompat.cpp. It is only instantiated here so the rest of the tree
// stays free of the implementation macros.
#define STB_IMAGE_WRITE_IMPLEMENTATION
#include <Platform/stb_image_write.h>

bool Platform_SaveImageTGA(const char* path, int width, int height, const void* bgraPixels) {
    if (!path || width <= 0 || height <= 0 || !bgraPixels) return false;

    // The rest of the client works in BGRA (D3D9 order); stb_image_write
    // expects RGBA, so swap the red and blue channels on the way out.
    const size_t count = (size_t)width * (size_t)height;
    const unsigned char* src = (const unsigned char*)bgraPixels;
    std::vector<unsigned char> rgba(count * 4);

    for (size_t i = 0; i < count; ++i) {
        rgba[i * 4 + 0] = src[i * 4 + 2];
        rgba[i * 4 + 1] = src[i * 4 + 1];
        rgba[i * 4 + 2] = src[i * 4 + 0];
        rgba[i * 4 + 3] = src[i * 4 + 3];
    }

    return stbi_write_tga(path, width, height, 4, rgba.data()) != 0;
}

bool Platform_SaveImageJPEG(const char* path, int width, int height, const void* rgbPixels, int quality) {
    if (!path || width <= 0 || height <= 0 || !rgbPixels) return false;
    if (quality < 1) quality = 1;
    if (quality > 100) quality = 100;
    return stbi_write_jpg(path, width, height, 3, rgbPixels, quality) != 0;
}
