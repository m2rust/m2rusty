#include <cstdio>
#include <cstdlib>
#include <cassert>
#include <cmath>
#include <Neoeter/Platform/Base/Poly/Poly.h>

int main()
{
    printf("[TEST] Starting CPoly mathematical expression parser tests...\n");

    CPoly poly;

    // 1. Basic arithmetic with operator precedence
    int ok = poly.Analyze("10 + 20 * 3");
    assert(ok != 0);
    float val = poly.Eval();
    assert(std::fabs(val - 70.0f) < 0.001f);
    printf("  [PASS] '10 + 20 * 3' evaluated to %.1f\n", val);

    // 2. Parentheses
    poly.Clear();
    ok = poly.Analyze("(10 + 20) * 3");
    assert(ok != 0);
    val = poly.Eval();
    assert(std::fabs(val - 90.0f) < 0.001f);
    printf("  [PASS] '(10 + 20) * 3' evaluated to %.1f\n", val);

    // 3. Variables (alphabetical identifiers only in Metin2 CPoly)
    poly.Clear();
    ok = poly.Analyze("atk * mult + 25");
    assert(ok != 0);
    poly.SetVar("atk", 150.0);
    poly.SetVar("mult", 1.5);
    val = poly.Eval();
    assert(std::fabs(val - 250.0f) < 0.001f);
    printf("  [PASS] 'atk * mult + 25' with atk=150, mult=1.5 evaluated to %.1f\n", val);

    printf("[TEST] CPoly tests PASSED successfully.\n");
    return 0;
}
