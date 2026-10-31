#pragma once

#include <Platform/Platform.h>
#include <Neoeter/Model/Grn/GrannyCompat.h>

#ifdef __cplusplus
extern "C" {
#endif

granny_file* GrannyParseGR2FromMemory(int size, const void* memory);
void GrannyFreeParsedGR2(granny_file* file);

#ifdef __cplusplus
}
#endif
