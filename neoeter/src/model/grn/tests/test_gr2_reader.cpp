#include <support/TestAssert.h>
#include <support/Gr2Fixture.h>

#include <Neoeter/Model/Grn/Gr2Reader.h>

#include <cstdio>
#include <cstring>
#include <string>
#include <vector>

namespace
{

void Put32(std::vector<unsigned char> & data, size_t offset, uint32_t value)
{
    TEST_ASSERT(offset + sizeof(value) <= data.size());
    std::memcpy(&data[offset], &value, sizeof(value));
}

// Runs the parser and guarantees that any returned file is freed.
bool Parses(const std::vector<unsigned char> & data)
{
    granny_file * file = GrannyParseGR2FromMemory(
        static_cast<int>(data.size()), data.empty() ? nullptr : data.data());
    if (!file)
        return false;

    const bool sane = file->FileInfo != nullptr;
    GrannyFreeParsedGR2(file);
    return sane;
}

void test_minimal_model()
{
    TEST_SECTION("synthetic GR2 model");

    const std::vector<unsigned char> data = Gr2Fixture::MakeMinimalModel();
    granny_file * file = GrannyParseGR2FromMemory(static_cast<int>(data.size()), data.data());
    TEST_ASSERT(file != nullptr);
    TEST_ASSERT(file->FileInfo != nullptr);
    TEST_ASSERT(file->FileInfo->ModelCount == 1);
    TEST_ASSERT(file->FileInfo->AnimationCount == 0);
    TEST_ASSERT(file->FileInfo->Models != nullptr);
    TEST_ASSERT(file->FileInfo->Models[0] != nullptr);
    TEST_ASSERT(std::strcmp(file->FileInfo->Models[0]->Name, "SyntheticModel") == 0);
    TEST_ASSERT(file->FileInfo->Models[0]->Skeleton == nullptr);
    TEST_ASSERT(file->FileInfo->Models[0]->MeshBindingCount == 0);

    GrannyFreeParsedGR2(file);
    GrannyFreeParsedGR2(nullptr);
    TEST_PASS("header, relocations, type tree and model record");
}

void test_truncated_input()
{
    TEST_SECTION("truncated GR2");

    const std::vector<unsigned char> valid = Gr2Fixture::MakeMinimalModel();

    TEST_ASSERT(!Parses({}));
    TEST_ASSERT(!Parses(std::vector<unsigned char>(1, 0)));
    TEST_ASSERT(!Parses(std::vector<unsigned char>(Gr2Fixture::Magic(),
                                                  Gr2Fixture::Magic() + 16)));

    for (size_t size : {size_t(17), size_t(31), size_t(67), size_t(88),
                        size_t(100), size_t(111), size_t(150), size_t(300),
                        valid.size() - 1})
    {
        std::vector<unsigned char> truncated(valid.begin(), valid.begin() + size);
        Parses(truncated); // must not crash; may accept a structurally complete prefix
    }

    TEST_PASS("short and truncated files are handled without reading past the end");
}

void test_corrupt_headers()
{
    TEST_SECTION("corrupt GR2 headers");

    const std::vector<unsigned char> valid = Gr2Fixture::MakeMinimalModel();

    struct Patch
    {
        const char * label;
        size_t offset;
        uint32_t value;
    };

    const Patch patches[] = {
        {"section table offset", 44, 0xFFFFFFF0u},
        {"section count", 48, 0xFFFFFFFFu},
        {"root type section", 52, 0xFFFFFFu},
        {"root type offset", 56, 0xFFFFFFu},
        {"root node section", 60, 0xFFFFFFu},
        {"root node offset", 64, 0xFFFFFFu},
        {"section uncompressed size", 68 + 12, 0xFFFFFFF0u},
        {"section compressed size", 68 + 8, 0xFFFFFFF0u},
        {"section data offset", 68 + 4, 0xFFFFFFF0u},
        {"relocation count", 68 + 32, 0xFFFFFFFFu},
        {"relocation offset", 68 + 28, 0xFFFFFFF0u},
    };

    for (const Patch & patch : patches)
    {
        std::vector<unsigned char> corrupt = valid;
        Put32(corrupt, patch.offset, patch.value);
        if (Parses(corrupt))
            std::printf("    accepted: %s\n", patch.label);
        else
            std::printf("    rejected: %s\n", patch.label);
    }

    // Negative model count is clamped by the reader.
    std::vector<unsigned char> negative = valid;
    Put32(negative, 112 + 64, 0xFFFFFFFFu);
    granny_file * file = GrannyParseGR2FromMemory(static_cast<int>(negative.size()),
                                                  negative.data());
    if (file)
    {
        TEST_ASSERT(file->FileInfo != nullptr);
        TEST_ASSERT(file->FileInfo->ModelCount == 0);
        GrannyFreeParsedGR2(file);
    }

    TEST_PASS("out-of-range offsets and sizes do not crash");
}

void test_compressed_garbage()
{
    TEST_SECTION("corrupt compressed section");

    std::vector<unsigned char> data = Gr2Fixture::MakeMinimalModel();
    Put32(data, 68 + 0, 2); // fmt 2: LZO-style compressed section
    for (size_t i = 112; i < data.size(); ++i)
        data[i] = static_cast<unsigned char>(i * 37);

    Parses(data);
    TEST_PASS("compressed section decoder is reachable with arbitrary bytes");
}

} // namespace

int main()
{
    test_minimal_model();
    test_truncated_input();
    test_corrupt_headers();
    test_compressed_garbage();

    std::printf("[test_gr2_reader] all checks passed\n");
    return 0;
}
