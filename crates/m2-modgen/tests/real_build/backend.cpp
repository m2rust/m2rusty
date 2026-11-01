// Test backend for the generated C ABI: dbg, time, math and pack.
// The math/time logic is copied from the hand-written modules (PythonLauncher.cpp)
// so that the generated shim can be compared with them; pack follows PythonPackModule.cpp.
#include <Python.h>

#include <chrono>
#include <cmath>
#include <ctime>
#include <cstdio>
#include <cstring>
#include <map>
#include <numbers>
#include <string>

#include "m2_dbg.h"
#include "m2_math.h"
#include "m2_pack.h"
#include "m2_time.h"

static m2_result ok() { return {M2_OK, nullptr}; }
static m2_result domain() { return {M2_VALUE_ERROR, "math domain error"}; }

// ---- dbg: record the calls so the script can look at them -----------------
static std::string g_dbg_log;
extern "C" const char *m2test_dbg_log() { return g_dbg_log.c_str(); }
m2_result m2_dbg_log_box(const char *p0, const char *p1) { g_dbg_log += std::string("LogBox:") + p0 + ":" + (p1 ? p1 : "-") + "\n"; return ok(); }
m2_result m2_dbg_register_exception_string(const char *p0) { g_dbg_log += std::string("Reg:") + p0 + "\n"; return ok(); }
m2_result m2_dbg_trace(const char *p0) { g_dbg_log += std::string("Trace:") + p0 + "\n"; return ok(); }
m2_result m2_dbg_trace_error(const char *p0) { g_dbg_log += std::string("TraceError:") + p0 + "\n"; return ok(); }
m2_result m2_dbg_tracen(const char *p0) { g_dbg_log += std::string("Tracen:") + p0 + "\n"; return ok(); }

// ---- math -----------------------------------------------------------------
#define M1(name, expr) \
    m2_result m2_math_##name(double x, double *out0) { *out0 = (expr); return ok(); }
M1(sin, std::sin(x))
M1(cos, std::cos(x))
M1(tan, std::tan(x))
M1(asin, std::asin(x))
M1(acos, std::acos(x))
M1(atan, std::atan(x))
M1(ceil, std::ceil(x))
M1(floor, std::floor(x))
M1(fabs, std::fabs(x))
M1(exp, std::exp(x))
M1(degrees, x * (180.0 / std::numbers::pi))
M1(radians, x * (std::numbers::pi / 180.0))
m2_result m2_math_atan2(double y, double x, double *out0) { *out0 = std::atan2(y, x); return ok(); }
m2_result m2_math_hypot(double x, double y, double *out0) { *out0 = std::hypot(x, y); return ok(); }
m2_result m2_math_pow(double x, double y, double *out0) { *out0 = std::pow(x, y); return ok(); }
m2_result m2_math_sqrt(double x, double *out0)
{
    if (x < 0.0) return domain();
    *out0 = std::sqrt(x);
    return ok();
}
m2_result m2_math_log10(double x, double *out0)
{
    if (x <= 0.0) return domain();
    *out0 = std::log10(x);
    return ok();
}
m2_result m2_math_fmod(double x, double y, double *out0)
{
    if (y == 0.0) return domain();
    *out0 = std::fmod(x, y);
    return ok();
}
m2_result m2_math_modf(double x, double *out0, double *out1)
{
    double ip = 0;
    *out0 = std::modf(x, &ip);
    *out1 = ip;
    return ok();
}
m2_result m2_math_log(double x, void *baseObjV, double *out0)
{
    PyObject *baseObj = static_cast<PyObject *>(baseObjV);
    if (x <= 0.0) return domain();
    if (baseObj && baseObj != Py_None)
    {
        double const base = PyFloat_AsDouble(baseObj);
        if (PyErr_Occurred()) return {M2_PYERR, nullptr};
        if (base <= 0.0 || base == 1.0) return domain();
        *out0 = std::log(x) / std::log(base);
        return ok();
    }
    *out0 = std::log(x);
    return ok();
}

// ---- time -----------------------------------------------------------------
m2_result m2_time_time(double *out0)
{
    *out0 = std::chrono::duration<double>(std::chrono::system_clock::now().time_since_epoch()).count();
    return ok();
}
m2_result m2_time_clock(double *out0)
{
    static auto start = std::chrono::steady_clock::now();
    *out0 = std::chrono::duration<double>(std::chrono::steady_clock::now() - start).count();
    return ok();
}
m2_result m2_time_sleep(double secs)
{
    struct timespec ts{};
    ts.tv_sec = static_cast<time_t>(secs);
    ts.tv_nsec = static_cast<long>((secs - ts.tv_sec) * 1e9);
    nanosleep(&ts, nullptr);
    return ok();
}
m2_result m2_time_strftime(const char *fmt, void *tupV, const char **out0)
{
    PyObject *tup = static_cast<PyObject *>(tupV);
    time_t const t = ::time(nullptr);
    struct tm *tm_info = localtime(&t);
    struct tm custom_tm{};
    if (tup && PyTuple_Check(tup) && PyTuple_Size(tup) >= 9)
    {
        custom_tm.tm_year = PyInt_AsLong(PyTuple_GetItem(tup, 0)) - 1900;
        custom_tm.tm_mon = PyInt_AsLong(PyTuple_GetItem(tup, 1)) - 1;
        custom_tm.tm_mday = PyInt_AsLong(PyTuple_GetItem(tup, 2));
        custom_tm.tm_hour = PyInt_AsLong(PyTuple_GetItem(tup, 3));
        custom_tm.tm_min = PyInt_AsLong(PyTuple_GetItem(tup, 4));
        custom_tm.tm_sec = PyInt_AsLong(PyTuple_GetItem(tup, 5));
        custom_tm.tm_wday = PyInt_AsLong(PyTuple_GetItem(tup, 6));
        custom_tm.tm_yday = PyInt_AsLong(PyTuple_GetItem(tup, 7)) - 1;
        custom_tm.tm_isdst = PyInt_AsLong(PyTuple_GetItem(tup, 8));
        tm_info = &custom_tm;
    }
    static char buf[256];
    size_t const len = strftime(buf, sizeof(buf), fmt, tm_info);
    buf[len] = 0;
    *out0 = buf;
    return ok();
}

// ---- pack: an in-memory pack ---------------------------------------------
static const std::map<std::string, std::string> &files()
{
    static const std::map<std::string, std::string> f = {
        {"a.py", "print 1\n"}, {"b.txt", std::string("hello\0world", 11)}, {"c.bin", "binary"}};
    return f;
}
m2_result m2_pack_exist(const char *name, int32_t *out0)
{
    *out0 = files().count(name) ? 1 : 0;
    return ok();
}
m2_result m2_pack_get(const char *name, const uint8_t **out0, size_t *out0_len)
{
    const char *ext = strrchr(name, '.');
    if (ext && (strcasecmp(ext, ".py") == 0 || strcasecmp(ext, ".pyc") == 0 || strcasecmp(ext, ".txt") == 0))
    {
        auto it = files().find(name);
        if (it != files().end())
        {
            *out0 = reinterpret_cast<const uint8_t *>(it->second.data());
            *out0_len = it->second.size();
            return ok();
        }
    }
    return {M2_NONE, nullptr}; // Py_BuildException(): None
}
