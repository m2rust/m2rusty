#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <cassert>
#include <vector>
#include <Neoeter/Platform/Base/lzo.h>

int main()
{
    printf("[TEST] Starting LZO compression/decompression tests...\n");

    CLZO lzo;

    // Create a 16KB compressible payload
    const size_t rawSize = 16 * 1024;
    std::vector<char> rawData(rawSize);
    for (size_t i = 0; i < rawSize; ++i)
    {
        rawData[i] = static_cast<char>('A' + (i % 26));
    }

    // 1. Unencrypted compression and decompression
    CLZObject compObj;
    bool compResult = lzo.CompressMemory(compObj, rawData.data(), rawSize);
    assert(compResult == true);
    assert(compObj.GetSize() > 0);
    assert(compObj.GetSize() < rawSize);
    printf("  [PASS] Compressed 16384 bytes to %u bytes (ratio: %.1f%%)\n",
           compObj.GetSize(),
           (static_cast<float>(compObj.GetSize()) / rawSize) * 100.0f);

    CLZObject decompObj;
    bool decompResult = lzo.Decompress(decompObj, compObj.GetBuffer());
    assert(decompResult == true);
    assert(decompObj.GetBufferSize() == rawSize);
    assert(memcmp(decompObj.GetBuffer(), rawData.data(), rawSize) == 0);
    printf("  [PASS] Decompressed data matches original data byte-for-byte.\n");

    // 2. Encrypted LZO compression & decompression
    DWORD teaKey[4] = { 0xDEADBEEF, 0xCAFEBABE, 0x01234567, 0x89ABCDEF };
    CLZObject encCompObj;
    bool encCompResult = lzo.CompressEncryptedMemory(encCompObj, rawData.data(), rawSize, teaKey);
    assert(encCompResult == true);

    CLZObject decDecompObj;
    bool decDecompResult = lzo.Decompress(decDecompObj, encCompObj.GetBuffer(), teaKey);
    assert(decDecompResult == true);
    assert(decDecompObj.GetBufferSize() == rawSize);
    assert(memcmp(decDecompObj.GetBuffer(), rawData.data(), rawSize) == 0);
    printf("  [PASS] Encrypted LZO round-trip verified successfully.\n");

    printf("[TEST] LZO tests PASSED successfully.\n");
    return 0;
}
