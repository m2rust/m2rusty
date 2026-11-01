/* Runs a Python 2.7 script inside the repository's embedded host (dbg, time, math registered). */
#include <stdio.h>
#include <stdlib.h>

#include "Neoeter/Python/Abi/neoeter_python.h"

int main(int argc, char **argv)
{
    if (argc < 2)
        return 2;
    FILE *f = fopen(argv[1], "rb");
    if (!f)
        return 2;
    fseek(f, 0, SEEK_END);
    long n = ftell(f);
    fseek(f, 0, SEEK_SET);
    char *src = malloc((size_t)n + 1);
    if (fread(src, 1, (size_t)n, f) != (size_t)n)
        return 2;
    src[n] = 0;
    fclose(f);
    if (neoeter_python_init("m2-modgen-harness") != NEOETER_OK)
        return 3;
    int rc = neoeter_python_run_line(src) == NEOETER_OK ? 0 : 1;
    neoeter_python_shutdown();
    return rc;
}
