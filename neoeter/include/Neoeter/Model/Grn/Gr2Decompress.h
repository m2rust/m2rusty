#pragma once

#ifdef __cplusplus
#include <cstdint>
extern "C" {
#else
#include <stdint.h>
#endif

/* Returns 1 on success, 0 if the input is truncated, inconsistent or corrupt. */
int gr2_decompress(uint32_t compressed_size, const uint8_t* compressed_buffer,
                   uint32_t step1, uint32_t step2,
                   uint32_t decompressed_size, uint8_t* decompressed_buffer);

#ifdef __cplusplus
}
#endif
