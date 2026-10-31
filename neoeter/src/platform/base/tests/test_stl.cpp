#include <cstdio>
#include <cstdlib>
#include <cassert>
#include <string>
#include <vector>
#include <Neoeter/Platform/Base/Stl.h>

int main()
{
    printf("[TEST] Starting Stl utility tests...\n");

    // 1. split_string basic (returns number of delimiters found)
    {
        std::vector<std::string> results;
        int delimCount = split_string("warrior,ninja,sura,shaman", ",", results, false);
        assert(delimCount == 3);
        assert(results.size() == 4);
        assert(results[0] == "warrior");
        assert(results[1] == "ninja");
        assert(results[2] == "sura");
        assert(results[3] == "shaman");
        printf("  [PASS] split_string CSV: 3 delimiters found, 4 tokens parsed.\n");
    }

    // 2. split_string with multiple delimiters and empty tokens
    {
        std::vector<std::string> results;
        int delimCount = split_string("alpha::beta::::gamma", "::", results, false);
        assert(delimCount == 3);
        assert(results.size() == 3);
        assert(results[0] == "alpha");
        assert(results[1] == "beta");
        assert(results[2] == "gamma");
        printf("  [PASS] split_string with empty tokens omitted: 3 tokens parsed.\n");
    }

    // 3. stl_lowers
    {
        std::string text = "Metin2 CLIENT Linux 2026";
        stl_lowers(text);
        assert(text == "metin2 client linux 2026");
        printf("  [PASS] stl_lowers lowercases ASCII characters.\n");
    }

    // 4. korean_tolower
    {
        assert(korean_tolower('A') == 'a');
        assert(korean_tolower('Z') == 'z');
        assert(korean_tolower('c') == 'c');
        assert(korean_tolower('1') == '1');
        printf("  [PASS] korean_tolower correctly maps uppercase letters.\n");
    }

    // 5. korean_tolower passes non-ASCII bytes through unchanged.
    // CP949 lead/trail bytes must survive lowercasing, and libc tolower() is
    // undefined for negative char values, so the function must not delegate
    // to it outside 'A'..'Z'.
    {
        for (int value = 0x80; value <= 0xFF; ++value)
        {
            char c = static_cast<char>(value);
            assert(korean_tolower(c) == c);
        }
        assert(korean_tolower(static_cast<char>(0xC3)) == static_cast<char>(0xC3));
        assert(korean_tolower(static_cast<char>(0xE3)) == static_cast<char>(0xE3));
        printf("  [PASS] korean_tolower leaves non-ASCII bytes unchanged.\n");
    }

    printf("[TEST] Stl utility tests PASSED successfully.\n");
    return 0;
}
