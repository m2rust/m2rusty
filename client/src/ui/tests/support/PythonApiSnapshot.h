#pragma once

// ============================================================================
// PythonApiSnapshot.h - capture the embedded interpreter's API surface
// ============================================================================
// TA.6: the client embeds Python 2.7 and registers its C++ bindings through
// Py_InitModule().  This helper boots nothing on its own; callers create the
// CPythonLauncher, register the modules they own, then call Capture().
//
// Capture() imports every requested module, lists its public attributes and
// compares the result with a committed golden file.  Set
// OPENMT2_UPDATE_PYTHON_API=1 to rewrite the golden file when a binding is
// added, removed or renamed on purpose.
//
// Notes:
// - Only the Python C API and the standard library are used, so the header can
//   be copied into each package (same pattern as support/TestAssert.h).
// - Attributes starting with "__" are interpreter bookkeeping, not API.
// - The snapshot is text, sorted, and contains no timestamps, so it is stable
//   across runs and machines.

#include <Python.h>  // T1.9: the include dir selects the platform copy

#include <algorithm>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <iterator>
#include <sstream>
#include <string>
#include <vector>

namespace PythonApiSnapshot
{

inline std::string AttributeName(PyObject* key)
{
    return std::string(PyString_AsString(key));
}

inline bool IsBookkeeping(const std::string& name)
{
    return name.size() >= 2 && name[0] == '_' && name[1] == '_';
}

inline std::string AttributeValueRepr(PyObject* value)
{
    PyObject* repr = PyObject_Repr(value);
    std::string text = "<unrepresentable>";
    if (repr && PyString_Check(repr))
        text = PyString_AsString(repr);
    Py_XDECREF(repr);

    // One attribute must stay on one line even for strings with newlines.
    for (std::size_t i = 0; i < text.size(); ++i)
    {
        if (text[i] == '\n' || text[i] == '\r' || text[i] == '\t')
            text[i] = ' ';
    }
    return text;
}

inline std::string AttributeLine(PyObject* key, PyObject* value)
{
    const std::string name = AttributeName(key);
    if (PyCallable_Check(value))
        return name + "\tcallable";

    return name + "\t" + Py_TYPE(value)->tp_name + "\t" +
           AttributeValueRepr(value);
}

inline std::string Build(const char* scope,
                         const std::vector<const char*>& moduleNames)
{
    std::ostringstream out;
    out << "# OpenMT2 embedded Python API surface: " << scope << "\n";
    out << "# Regenerate with: OPENMT2_UPDATE_PYTHON_API=1 <test-binary> "
           "<this-file>\n";

    std::size_t attributeCount = 0;
    for (std::size_t m = 0; m < moduleNames.size(); ++m)
    {
        const char* moduleName = moduleNames[m];
        PyObject* module = PyImport_ImportModule(moduleName);
        if (module == NULL)
        {
            std::fprintf(stderr, "[FAIL] could not import module '%s'\n",
                         moduleName);
            PyErr_Print();
            std::abort();
        }

        out << "module\t" << moduleName << "\n";

        PyObject* dict = PyModule_GetDict(module);
        std::vector<std::string> lines;
        PyObject* key = NULL;
        PyObject* value = NULL;
        Py_ssize_t pos = 0;
        while (PyDict_Next(dict, &pos, &key, &value))
        {
            if (!PyString_Check(key))
                continue;

            const std::string name = AttributeName(key);
            if (IsBookkeeping(name))
                continue;

            lines.push_back(AttributeLine(key, value));
        }

        std::sort(lines.begin(), lines.end());
        for (std::size_t i = 0; i < lines.size(); ++i)
            out << "  " << lines[i] << "\n";

        attributeCount += lines.size();
        Py_DECREF(module);
    }

    std::fprintf(stderr, "[info] %s: %lu modules, %lu attributes\n", scope,
                 (unsigned long)moduleNames.size(),
                 (unsigned long)attributeCount);
    return out.str();
}

inline bool WriteFile(const std::string& path, const std::string& contents)
{
    std::ofstream out(path.c_str(), std::ios::binary | std::ios::trunc);
    if (!out)
        return false;
    out.write(contents.data(), static_cast<std::streamsize>(contents.size()));
    return out.good();
}

inline void ReportMismatch(const std::string& actualPath,
                           const std::string& expected,
                           const std::string& actual)
{
    std::size_t offset = 0;
    std::size_t line = 1;
    while (offset < expected.size() && offset < actual.size() &&
           expected[offset] == actual[offset])
    {
        if (expected[offset] == '\n')
            ++line;
        ++offset;
    }

    std::fprintf(stderr,
                 "[FAIL] API snapshot differs at line %lu; actual surface "
                 "written to %s\n",
                 (unsigned long)line, actualPath.c_str());
}

inline void Capture(const char* scope,
                    const std::vector<const char*>& moduleNames,
                    const char* goldenPath)
{
    const std::string actual = Build(scope, moduleNames);

    const char* update = std::getenv("OPENMT2_UPDATE_PYTHON_API");
    if (update && *update)
    {
        if (!WriteFile(goldenPath, actual))
        {
            std::fprintf(stderr, "[FAIL] could not write %s\n", goldenPath);
            std::abort();
        }
        std::printf("[updated] %s\n", goldenPath);
        return;
    }

    std::ifstream in(goldenPath, std::ios::binary);
    if (!in)
    {
        std::fprintf(stderr, "[FAIL] missing golden file %s\n", goldenPath);
        std::abort();
    }
    const std::string expected((std::istreambuf_iterator<char>(in)),
                               std::istreambuf_iterator<char>());

    if (actual == expected)
    {
        std::printf("[PASS] API surface matches %s\n", goldenPath);
        return;
    }

    const std::string actualPath = std::string(goldenPath) + ".actual";
    WriteFile(actualPath, actual);
    ReportMismatch(actualPath, expected, actual);
    std::abort();
}

} // namespace PythonApiSnapshot
