#include <Neoeter/Model/Grn/Gr2Reader.h>

#include <cstddef>
#include <cstdint>

extern "C" int LLVMFuzzerTestOneInput(const uint8_t * data, size_t size)
{
    if (size > (1u << 20))
        return 0;

    granny_file * file = GrannyParseGR2FromMemory(static_cast<int>(size), data);
    if (file)
        GrannyFreeParsedGR2(file);
    return 0;
}
