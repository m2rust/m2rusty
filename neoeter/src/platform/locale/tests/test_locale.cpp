#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <cwchar>
#include <cassert>
#include <Neoeter/Platform/Locale/StringCodec.h>
#include <Neoeter/Platform/Locale/Japanese.h>
#include <Neoeter/Platform/Locale/CodePageId.h>

int main()
{
    printf("[TEST] Starting EterLocale smoke tests...\n");

    // 1. UTF-8 <-> WideChar transcoding round-trip
    const char* originalUtf8 = "Metin2 Linux Port - Locale & UTF-8 Test!";
    wchar_t wbuf[128] = {0};

    int wlen = MultiByteToWideChar(CP_UTF8, 0, originalUtf8, -1, wbuf, 128);
    assert(wlen > 0);
    printf("  [PASS] MultiByteToWideChar converted %d wide characters.\n", wlen);

    char utf8Back[128] = {0};
    int mblen = WideCharToMultiByte(CP_UTF8, 0, wbuf, -1, utf8Back, 128, nullptr, nullptr);
    assert(mblen > 0);
    assert(strcmp(originalUtf8, utf8Back) == 0);
    printf("  [PASS] WideCharToMultiByte round-trip matches original string.\n");

    // 2. CP1252 Latin transcoding
    const char* latinText = "Cafe / Naive / Resume";
    wchar_t wbufLatin[128] = {0};
    int wlenLatin = MultiByteToWideChar(CP_1252, 0, latinText, -1, wbufLatin, 128);
    assert(wlenLatin > 0);

    char latinBack[128] = {0};
    int mblenLatin = WideCharToMultiByte(CP_1252, 0, wbufLatin, -1, latinBack, 128, nullptr, nullptr);
    assert(mblenLatin > 0);
    assert(strcmp(latinText, latinBack) == 0);
    printf("  [PASS] CP1252 round-trip verified.\n");

    // 3. ShiftJIS Japanese character checks
    assert(ShiftJIS_IsLeadByte(static_cast<char>(0x82)) == TRUE);
    assert(ShiftJIS_IsLeadByte('A') == FALSE);
    assert(ShiftJIS_StringCompareCI("test_case", "TEST_CASE", 9) == 0);
    printf("  [PASS] Shift-JIS lead byte & case-insensitive compare verified.\n");

    printf("[TEST] EterLocale tests PASSED successfully.\n");
    return 0;
}
