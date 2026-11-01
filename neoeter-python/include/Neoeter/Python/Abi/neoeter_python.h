/* C ABI over the Python 2.7 host, for the Rust client (docs/bevy-port-plan.md).
 *
 * Follows neoeter/docs/abi.md: extern "C", no C++ types, UTF-8 strings,
 * results as an enum, main thread only. */
#ifndef NEOETER_PYTHON_ABI_H
#define NEOETER_PYTHON_ABI_H

#ifdef __cplusplus
extern "C" {
#endif

#if defined(_WIN32)
#define NEOETER_PYTHON_API __declspec(dllexport)
#else
#define NEOETER_PYTHON_API __attribute__((visibility("default")))
#endif

#define NEOETER_PYTHON_ABI_VERSION_MAJOR 0
#define NEOETER_PYTHON_ABI_VERSION_MINOR 1

typedef enum neoeter_result
{
    NEOETER_OK = 0,
    NEOETER_INVALID_ARGUMENT = 1,
    NEOETER_NOT_FOUND = 2,
    NEOETER_IO = 3,
    NEOETER_UNSUPPORTED = 4,
    NEOETER_INTERNAL = 5
} neoeter_result;

/* (major << 16) | minor of the header the library was built from. */
NEOETER_PYTHON_API unsigned int neoeter_python_version(void);

/* Starts the interpreter and registers the host modules (dbg, time, math).
 * INVALID_ARGUMENT if it is already running. */
NEOETER_PYTHON_API neoeter_result neoeter_python_init(const char *program_name);

/* Runs source in __main__. INTERNAL if it raised; the traceback is logged. */
NEOETER_PYTHON_API neoeter_result neoeter_python_run_line(const char *source);

/* Stops the interpreter. A no-op when it is not running. */
NEOETER_PYTHON_API void neoeter_python_shutdown(void);

#ifdef __cplusplus
}
#endif

#endif
