#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <cassert>
#include <Neoeter/Platform/Base/tea.h>

int main()
{
    printf("[TEST] Starting TEA encryption/decryption tests...\n");

    const uint32_t key[4] = {
        0x12345678,
        0x9ABCDEF0,
        0xFEDCBA98,
        0x76543210
    };

    // 1. Aligned 64-byte buffer round-trip
    char original[64] = "Metin2 Linux Porting - TEA encryption verification string test!";
    uint32_t encrypted[16] = {0};
    uint32_t decrypted[16] = {0};

    int encSize = tea_encrypt(encrypted, reinterpret_cast<const uint32_t*>(original), key, sizeof(original));
    assert(encSize == sizeof(original));
    assert(memcmp(original, encrypted, sizeof(original)) != 0);

    int decSize = tea_decrypt(decrypted, encrypted, key, encSize);
    assert(decSize == sizeof(original));
    assert(memcmp(original, decrypted, sizeof(original)) == 0);
    printf("  [PASS] 64-byte block encryption & decryption match perfectly.\n");

    // 2. Unaligned size test (padding to multiple of 8)
    char unalignedInput[24] = {0};
    memcpy(unalignedInput, "Unaligned TEA 19b!", 18);
    uint32_t encUnaligned[8] = {0};
    uint32_t decUnaligned[8] = {0};

    int paddedEncSize = tea_encrypt(encUnaligned, reinterpret_cast<const uint32_t*>(unalignedInput), key, 18);
    // 18 bytes padded to multiple of 8 is 24 bytes
    assert(paddedEncSize == 24);

    int paddedDecSize = tea_decrypt(decUnaligned, encUnaligned, key, paddedEncSize);
    assert(paddedDecSize == 24);
    assert(memcmp(unalignedInput, decUnaligned, 18) == 0);
    printf("  [PASS] Unaligned 18-byte input padded to 24-bytes matches.\n");

    printf("[TEST] TEA tests PASSED successfully.\n");
    return 0;
}
