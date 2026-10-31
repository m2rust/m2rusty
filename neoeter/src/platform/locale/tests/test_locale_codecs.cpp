#include <support/TestAssert.h>

#include <cstring>
#include <cwchar>

#include <Neoeter/Platform/Locale/CodePageId.h>
#include <Neoeter/Platform/Locale/StringCodec.h>

namespace
{

// Round-trip a byte string through wide characters and back.
void roundtrip(UINT codePage, const char * input, int inputLength, const char * label)
{
    wchar_t wide[256] = {};
    int wideLength = MultiByteToWideChar(codePage, 0, input, inputLength, wide,
                                         static_cast<int>(sizeof(wide) / sizeof(wide[0])));
    TEST_ASSERT(wideLength > 0);

    char back[256] = {};
    int backLength = WideCharToMultiByte(codePage, 0, wide, wideLength, back,
                                         static_cast<int>(sizeof(back)), nullptr, nullptr);
    TEST_ASSERT(backLength > 0);

    // With a negative length the conversion includes the terminator, and so
    // does the reverse conversion.
    int expected = (inputLength < 0) ? static_cast<int>(std::strlen(input)) + 1 : inputLength;
    TEST_ASSERT_MSG(backLength == expected, label);
    TEST_ASSERT_MSG(std::memcmp(back, input, static_cast<size_t>(expected)) == 0, label);
    TEST_PASS(label);
}

} // namespace

int main()
{
    TEST_SECTION("EterLocale codecs");

    // UTF-8: multi-byte sequences survive the round trip.
    roundtrip(CP_UTF8, "Caf\xC3\xA9", -1, "UTF-8 accented characters");
    roundtrip(CP_UTF8, "\xE6\x97\xA5\xE6\x9C\xAC\xE8\xAA\x9E", -1, "UTF-8 CJK");

    // CP1252: single-byte Latin-1 supplement.
    roundtrip(CP_1252, "Caf\xE9", -1, "CP1252 accented characters");

    // CP949: Korean double-byte sequence for U+D55C.
    roundtrip(CP_949, "\xC7\xD1", -1, "CP949 Korean");

    // Explicit length: no terminator required.
    roundtrip(CP_UTF8, "abcdef", 3, "explicit length, no terminator");

    // Size query (cchWideChar == 0) must report what the real call returns.
    const char * sample = "Metin2";
    int needed = MultiByteToWideChar(CP_UTF8, 0, sample, -1, nullptr, 0);
    wchar_t wide[32] = {};
    int written = MultiByteToWideChar(CP_UTF8, 0, sample, -1, wide, 32);
    TEST_ASSERT(needed == written);
    TEST_ASSERT(written == static_cast<int>(std::strlen(sample)) + 1);
    TEST_PASS("size query matches the buffer call");

    // The reverse direction also honours the size query.
    int neededBytes = WideCharToMultiByte(CP_UTF8, 0, wide, written, nullptr, 0, nullptr, nullptr);
    TEST_ASSERT(neededBytes == static_cast<int>(std::strlen(sample)) + 1);
    TEST_PASS("WideCharToMultiByte size query");

    return 0;
}
