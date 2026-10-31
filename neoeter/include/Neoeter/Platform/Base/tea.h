#ifndef __INC_TEA_H__
#define __INC_TEA_H__

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

	#define TEA_KEY_LENGTH 16

    int tea_encrypt(uint32_t *dest, const uint32_t *src, const uint32_t *key, int size);
    int tea_decrypt(uint32_t *dest, const uint32_t *src, const uint32_t *key, int size);

#ifdef __cplusplus
};
#endif

#endif
