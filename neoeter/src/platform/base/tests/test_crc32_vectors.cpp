#include <support/TestAssert.h>

#include <cstring>

#include <Neoeter/Platform/Base/CRC32.h>

int main()
{
    TEST_SECTION("CRC32 known vectors");

    // Standard IEEE CRC-32 check value.
    TEST_ASSERT(GetCRC32("123456789", 9) == 0xCBF43926u);
    TEST_PASS("check vector 123456789 == 0xCBF43926");

    TEST_ASSERT(GetCRC32("", 0) == 0u);
    TEST_PASS("empty input is 0");

    // A single differing byte must change the result.
    TEST_ASSERT(GetCRC32("abc", 3) != GetCRC32("abd", 3));
    TEST_ASSERT(GetCRC32("abc", 3) != GetCRC32("ab", 2));
    TEST_PASS("length and content sensitivity");

    // The case-insensitive variant folds ASCII case, the plain one does not.
    TEST_ASSERT(GetCaseCRC32("METIN2", 6) == GetCaseCRC32("metin2", 6));
    TEST_ASSERT(GetCaseCRC32("MeTiN2", 6) == GetCRC32("metin2", 6));
    TEST_ASSERT(GetCRC32("METIN2", 6) != GetCRC32("metin2", 6));
    TEST_PASS("GetCaseCRC32 folds ASCII case");

    // Non-ASCII bytes must not be folded (locale independence).
    const char upper_eacute[] = { static_cast<char>(0xC9), 0 };
    const char lower_eacute[] = { static_cast<char>(0xE9), 0 };
    TEST_ASSERT(GetCaseCRC32(upper_eacute, 1) != GetCaseCRC32(lower_eacute, 1));
    TEST_PASS("non-ASCII bytes are not folded");

    return 0;
}
