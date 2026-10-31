#pragma once
#include <cstdint>
#include <cstddef>
#include <vector>

bool Platform_GetImageInfo(const void* data, size_t size, uint32_t* outW, uint32_t* outH, uint32_t* outFormat);
bool Platform_DecodeImage(const void* data, size_t size, uint32_t* outW, uint32_t* outH, std::vector<uint8_t>& outPixels);

// Writes a BGRA image as an uncompressed TGA (the format used by guild marks).
// Returns false when the file cannot be written.
bool Platform_SaveImageTGA(const char* path, int width, int height, const void* bgraPixels);

// Writes an RGB image as a baseline JPEG (quality 1-100). Used for client
// screenshots; replaces the libjpeg dependency (T2.2).
bool Platform_SaveImageJPEG(const char* path, int width, int height, const void* rgbPixels, int quality);
