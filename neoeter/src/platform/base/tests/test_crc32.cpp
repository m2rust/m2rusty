#include <support/TestAssert.h>

#include <cstring>

#include <Neoeter/Platform/Base/CRC32.h>

int main()
{
    TEST_SECTION("CRC32");

    const char* str1 = "TestStringForCRC32";
    const char* str2 = "TestStringForCRC32";
    const char* str3 = "DifferentString";

    DWORD crc1 = GetCRC32(str1, strlen(str1));
    DWORD crc2 = GetCRC32(str2, strlen(str2));
    DWORD crc3 = GetCRC32(str3, strlen(str3));

    // Consistency
    TEST_ASSERT(crc1 != 0);
    TEST_ASSERT(crc1 == crc2);
    TEST_ASSERT(crc1 != crc3);
    TEST_PASS("consistency: crc1 == crc2, crc1 != crc3");

    // Case CRC check: the case-insensitive hash folds ASCII case.
    DWORD case1 = GetCaseCRC32(str1, strlen(str1));
    TEST_ASSERT(case1 == GetCRC32("teststringforcrc32", 18));
    TEST_ASSERT(case1 != crc1);
    TEST_PASS("GetCaseCRC32 folds ASCII case");

    // Differentiation between cases
    const char* upper = "METIN2";
    const char* lower = "metin2";
    DWORD crcUpper = GetCRC32(upper, strlen(upper));
    DWORD crcLower = GetCRC32(lower, strlen(lower));
    TEST_ASSERT(crcUpper != crcLower);
    TEST_PASS("case difference verified");

    TEST_INFO("crc=0x%08X upper=0x%08X lower=0x%08X", crc1, crcUpper, crcLower);
    return 0;
}
