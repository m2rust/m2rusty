#pragma once

// ============================================================================
// TestAssert.h - minimal, dependency-free test helpers
// ============================================================================
// Decision D15: tests use a hand-rolled assertion layer rather than a third
// party framework, so the image needs no extra dependency and a failing assert
// always prints file, line and expression.
//
// Usage:
//     #include <support/TestAssert.h>
//     int main() {
//         TEST_SECTION("CRC32");
//         TEST_ASSERT(crc != 0);
//         TEST_ASSERT_MSG(a == b, "round trip");
//     }
//
// Notes:
// - Functions here are written against the C standard library only, so they
//   work in every package regardless of its platform shims.
// - Asserts are always active: test targets are compiled with -UNDEBUG.

#include <cstdio>
#include <cstdlib>

#define TEST_ASSERT(cond)                                                      \
    do {                                                                       \
        if (!(cond)) {                                                         \
            std::fprintf(stderr, "[FAIL] %s:%d: %s\n", __FILE__, __LINE__,      \
                         #cond);                                               \
            std::abort();                                                      \
        }                                                                      \
    } while (0)

#define TEST_ASSERT_MSG(cond, msg)                                             \
    do {                                                                       \
        if (!(cond)) {                                                         \
            std::fprintf(stderr, "[FAIL] %s:%d: %s (%s)\n", __FILE__,           \
                         __LINE__, #cond, (msg));                              \
            std::abort();                                                      \
        }                                                                      \
    } while (0)

#define TEST_SECTION(name) std::printf("[TEST] %s\n", (name))
#define TEST_PASS(msg) std::printf("  [PASS] %s\n", (msg))
#define TEST_INFO(fmt, ...) std::printf("  [info] " fmt "\n", ##__VA_ARGS__)
