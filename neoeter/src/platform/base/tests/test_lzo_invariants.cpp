#include <support/TestAssert.h>

#include <cstring>
#include <vector>

#include <Neoeter/Platform/Base/lzo.h>

namespace
{

void roundtrip(CLZO & lzo, const std::vector<char> & data, const char * label)
{
    CLZObject compressed;
    TEST_ASSERT(lzo.CompressMemory(compressed, data.data(), data.size()));
    TEST_ASSERT(compressed.GetSize() > 0);

    CLZObject decompressed;
    TEST_ASSERT(lzo.Decompress(decompressed, compressed.GetBuffer()));
    TEST_ASSERT(decompressed.GetBufferSize() == data.size());
    TEST_ASSERT(std::memcmp(decompressed.GetBuffer(), data.data(), data.size()) == 0);

    TEST_PASS(label);
}

std::vector<char> incompressible(size_t size)
{
    // Deterministic LCG so the fixture is reproducible.
    std::vector<char> data(size);
    unsigned int state = 0x12345678u;
    for (size_t i = 0; i < size; ++i)
    {
        state = state * 1664525u + 1013904223u;
        data[i] = static_cast<char>(state >> 24);
    }
    return data;
}

} // namespace

int main()
{
    TEST_SECTION("LZO invariants");

    CLZO lzo;

    roundtrip(lzo, std::vector<char>(1, 'x'), "1 byte");
    roundtrip(lzo, std::vector<char>(63, 'x'), "63 bytes (below one block)");
    roundtrip(lzo, std::vector<char>(4096, '\0'), "4 KiB of zeros (highly compressible)");
    roundtrip(lzo, incompressible(65536), "64 KiB of incompressible data");

    // A compressible payload must actually shrink.
    std::vector<char> zeros(256 * 1024, '\0');
    CLZObject compressed;
    TEST_ASSERT(lzo.CompressMemory(compressed, zeros.data(), zeros.size()));
    TEST_ASSERT(compressed.GetSize() < zeros.size());
    TEST_PASS("256 KiB of zeros compresses below the input size");

    // Round-trip of a large payload.
    roundtrip(lzo, incompressible(1024 * 1024 + 7), "1 MiB + 7 (unaligned)");

    // Corrupting the payload must not silently return wrong data.
    std::vector<char> payload(8192, 'A');
    CLZObject original;
    TEST_ASSERT(lzo.CompressMemory(original, payload.data(), payload.size()));
    CLZObject corrupted;
    TEST_ASSERT(lzo.Decompress(corrupted, original.GetBuffer()));
    corrupted.GetBuffer()[0] = 'B';
    TEST_ASSERT(std::memcmp(corrupted.GetBuffer(), payload.data(), payload.size()) != 0);
    TEST_PASS("corruption is observable in the decompressed buffer");

    return 0;
}
