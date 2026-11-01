#include <Neoeter/Python/Abi/neoeter_python.h>

#include <Neoeter/Python/Host/StdAfx.h>
#include <Neoeter/Python/Host/PythonDebugModule.h>
#include <Neoeter/Python/Host/PythonLauncher.h>

#include <memory>

namespace
{
// The launcher is a CSingleton: constructing it runs Py_Initialize and
// destroying it runs Py_Finalize.
std::unique_ptr<CPythonLauncher> g_launcher;
} // namespace

unsigned int neoeter_python_version(void)
{
    return (NEOETER_PYTHON_ABI_VERSION_MAJOR << 16) | NEOETER_PYTHON_ABI_VERSION_MINOR;
}

neoeter_result neoeter_python_init(const char *program_name)
{
    if (g_launcher)
        return NEOETER_INVALID_ARGUMENT;

    g_launcher = std::make_unique<CPythonLauncher>();
    if (!g_launcher->Create(program_name ? program_name : "eter.python"))
    {
        g_launcher.reset();
        return NEOETER_INTERNAL;
    }

    initdbg();
    return NEOETER_OK;
}

neoeter_result neoeter_python_run_line(const char *source)
{
    if (!g_launcher)
        return NEOETER_INVALID_ARGUMENT;
    if (!source)
        return NEOETER_INVALID_ARGUMENT;

    return g_launcher->RunLine(source) ? NEOETER_OK : NEOETER_INTERNAL;
}

void neoeter_python_shutdown(void)
{
    g_launcher.reset();
}
