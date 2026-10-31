#pragma once

#if defined(OPENMT2_PLATFORM_LINUX)
#include <string>

// Resolves a Windows font face name (e.g. "Tahoma") to a font file on disk.
// Order: the name as a path, $OPENMT2_FONT, fontconfig (family, then
// sans-serif), then a few well-known FHS paths. Results are cached per name.
// Returns an empty string when nothing usable is found.
std::string FontLocator_FindFontFile(const char* fontName);
#endif
