#include <support/TestAssert.h>

#include <cstring>

#include <Neoeter/Platform/Base/tea.h>

namespace
{

const uint32_t kKey[4] = { 0x12345678u, 0x9ABCDEF0u, 0xFEDCBA98u, 0x76543210u };

} // namespace

int main()
{
    TEST_SECTION("TEA invariants");

    // Exactly one block: no padding, no tail handling.
    {
        char input[8] = { '1', '2', '3', '4', '5', '6', '7', '8' };
        uint32_t encrypted[2] = { 0, 0 };
        uint32_t decrypted[2] = { 0, 0 };

        TEST_ASSERT(tea_encrypt(encrypted, reinterpret_cast<const uint32_t *>(input), kKey, 8) == 8);
        TEST_ASSERT(std::memcmp(input, encrypted, 8) != 0);
        TEST_ASSERT(tea_decrypt(decrypted, encrypted, kKey, 8) == 8);
        TEST_ASSERT(std::memcmp(input, decrypted, 8) == 0);
        TEST_PASS("single block round trip");
    }

    // 9 bytes pad to 16. The source buffer is exactly 9 bytes: the old
    // implementation wrote the padding into it (7-byte overflow).
    {
        char input[9] = { '1', '2', '3', '4', '5', '6', '7', '8', '9' };
        uint32_t encrypted[4] = { 0, 0, 0, 0 };
        uint32_t decrypted[4] = { 0, 0, 0, 0 };

        TEST_ASSERT(tea_encrypt(encrypted, reinterpret_cast<const uint32_t *>(input), kKey, 9) == 16);
        TEST_ASSERT(tea_decrypt(decrypted, encrypted, kKey, 16) == 16);
        TEST_ASSERT(std::memcmp(input, decrypted, 9) == 0);
        TEST_PASS("9 bytes pad to 16 and round trip");

        // Deterministic for the same key and input.
        uint32_t again[4] = { 0, 0, 0, 0 };
        TEST_ASSERT(tea_encrypt(again, reinterpret_cast<const uint32_t *>(input), kKey, 9) == 16);
        TEST_ASSERT(std::memcmp(encrypted, again, 16) == 0);
        TEST_PASS("encryption is deterministic");

        // A different key must produce different ciphertext.
        uint32_t otherKey[4] = { kKey[0] ^ 1u, kKey[1], kKey[2], kKey[3] };
        uint32_t other[4] = { 0, 0, 0, 0 };
        TEST_ASSERT(tea_encrypt(other, reinterpret_cast<const uint32_t *>(input), otherKey, 9) == 16);
        TEST_ASSERT(std::memcmp(encrypted, other, 16) != 0);
        TEST_PASS("key sensitivity");
    }

    // Empty input is a no-op.
    {
        char input[8] = { 0 };
        uint32_t output[2] = { 0, 0 };
        TEST_ASSERT(tea_encrypt(output, reinterpret_cast<const uint32_t *>(input), kKey, 0) == 0);
        TEST_PASS("zero length returns zero");
    }

    return 0;
}
