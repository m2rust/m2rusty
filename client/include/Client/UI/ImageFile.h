#pragma once

#include <cstdint>
#include <vector>

// File-level wrappers around the EterBase image compat layer. They replace the
// DevIL calls the guild-mark code used before T2.1.
bool GetImageFileInfo(const char* path, uint32_t* width, uint32_t* height, uint32_t* format);
bool LoadImageFileBGRA(const char* path, uint32_t* width, uint32_t* height, std::vector<uint8_t>& pixels);
