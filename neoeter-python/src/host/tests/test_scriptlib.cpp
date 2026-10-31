#include <Neoeter/Python/Host/StdAfx.h>
#include <Neoeter/Python/Host/PythonLauncher.h>
#include <Neoeter/Python/Host/PythonUtils.h>
#include <Neoeter/Python/Host/PythonMarshal.h>
#include <Neoeter/Python/Host/Resource.h>

#include <cassert>
#include <cstdio>
#include <cstring>
#include <string>

namespace {

class TestExceptionSender : public IPythonExceptionSender
{
public:
    std::string lastException;
    void Send() override
    {
        lastException = m_strExceptionString;
    }
    const std::string& GetException() const
    {
        return m_strExceptionString;
    }
};

void test_python_launcher_and_execution()
{
    CPythonLauncher& launcher = CPythonLauncher::Instance();

    // 1. Basic line execution
    bool lineOk = launcher.RunLine("val_int = 12345");
    assert(lineOk);

    lineOk = launcher.RunLine("val_str = 'metin2_open'");
    assert(lineOk);

    lineOk = launcher.RunLine("val_sum = 100 + 250");
    assert(lineOk);

    PyObject* mainMod = PyImport_AddModule("__main__");
    assert(mainMod != nullptr);
    PyObject* dict = PyModule_GetDict(mainMod);
    assert(dict != nullptr);

    PyObject* pyInt = PyDict_GetItemString(dict, "val_int");
    assert(pyInt != nullptr);
    assert(PyInt_AsLong(pyInt) == 12345);

    PyObject* pyStr = PyDict_GetItemString(dict, "val_str");
    assert(pyStr != nullptr);
    assert(std::strcmp(PyString_AsString(pyStr), "metin2_open") == 0);

    PyObject* pySum = PyDict_GetItemString(dict, "val_sum");
    assert(pySum != nullptr);
    assert(PyInt_AsLong(pySum) == 350);

    // 2. Check builtins TRUE and FALSE via script execution
    bool boolOk = launcher.RunLine(
        "test_true = TRUE\n"
        "test_false = FALSE\n"
    );
    assert(boolOk);
    PyObject* pyTrue = PyDict_GetItemString(dict, "test_true");
    assert(pyTrue != nullptr);
    assert(PyInt_AsLong(pyTrue) == 1);

    PyObject* pyFalse = PyDict_GetItemString(dict, "test_false");
    assert(pyFalse != nullptr);
    assert(PyInt_AsLong(pyFalse) == 0);

    // 3. Multiline code block
    bool scriptOk = launcher.RunLine(
        "def compute_fib(n):\n"
        "    if n <= 1:\n"
        "        return n\n"
        "    return compute_fib(n-1) + compute_fib(n-2)\n"
        "fib_result = compute_fib(7)\n"
    );
    assert(scriptOk);

    PyObject* pyFib = PyDict_GetItemString(dict, "fib_result");
    assert(pyFib != nullptr);
    assert(PyInt_AsLong(pyFib) == 13);

    std::printf("[PASS] test_python_launcher_and_execution\n");
}

void test_python_utils()
{
    // 1. Tuple parsing
    PyObject* args = Py_BuildValue("(isflb)", 77, "client_test", 3.14159f, 987654321L, 1);
    assert(args != nullptr);

    int intVal = 0;
    assert(PyTuple_GetInteger(args, 0, &intVal));
    assert(intVal == 77);

    char* strVal = nullptr;
    assert(PyTuple_GetString(args, 1, &strVal));
    assert(std::strcmp(strVal, "client_test") == 0);

    float floatVal = 0.0f;
    assert(PyTuple_GetFloat(args, 2, &floatVal));
    assert(std::fabs(floatVal - 3.14159f) < 1e-4f);

    long longVal = 0;
    assert(PyTuple_GetLong(args, 3, &longVal));
    assert(longVal == 987654321L);

    bool boolVal = false;
    assert(PyTuple_GetBoolean(args, 4, &boolVal));
    assert(boolVal == true);

    Py_DECREF(args);

    // 2. Class member function invocation
    CPythonLauncher::Instance().RunLine(
        "class Calculator:\n"
        "    def multiply(self, a, b):\n"
        "        return a * b\n"
        "calc_inst = Calculator()\n"
    );

    PyObject* mainMod = PyImport_AddModule("__main__");
    PyObject* dict = PyModule_GetDict(mainMod);
    PyObject* calcInst = PyDict_GetItemString(dict, "calc_inst");
    assert(calcInst != nullptr);

    PyObject* callArgs = Py_BuildValue("(ii)", 6, 7);
    long resultVal = 0;
    bool callOk = PyCallClassMemberFunc(calcInst, "multiply", callArgs, &resultVal);
    assert(callOk);
    assert(resultVal == 42);

    std::printf("[PASS] test_python_utils\n");
}

void test_debug_module()
{
    TestExceptionSender sender;
    SetExceptionSender(&sender);

    initdbg();

    CPythonLauncher::Instance().RunLine(
        "import dbg\n"
        "dbg.Trace('Test debug message from Python')\n"
        "dbg.Tracen('Test tracen message')\n"
        "dbg.RegisterExceptionString('CriticalScriptError: Test')\n"
    );

    assert(sender.GetException() == "CriticalScriptError: Test");

    SetExceptionSender(nullptr);
    std::printf("[PASS] test_debug_module\n");
}

static bool g_customDestroyCalled = false;
void CustomDestroyHandler()
{
    g_customDestroyCalled = true;
}

void test_python_resource()
{
    CPythonResource::RegisterDestroyHandler(CustomDestroyHandler);

    CPythonResource res;
    res.Destroy();

    assert(g_customDestroyCalled == true);
    std::printf("[PASS] test_python_resource\n");
}

} // namespace

void test_embedded_modules()
{
    CPythonLauncher& launcher = CPythonLauncher::Instance();
    bool ok = launcher.RunLine(
        "import math\n"
        "assert math.pi > 3.14\n"
        "assert math.sin(0) == 0.0\n"
        "assert math.cos(0) == 1.0\n"
        "assert math.ceil(2.3) == 3.0\n"
        "import time\n"
        "t = time.time()\n"
        "assert t > 0\n"
    );
    assert(ok);
    std::printf("[PASS] test_embedded_modules\n");
}

int main()
{
    CPythonLauncher launcher;
    bool created = launcher.Create("test.launcher");
    assert(created);

    test_python_launcher_and_execution();
    test_python_utils();
    test_debug_module();
    test_python_resource();
    test_embedded_modules();

    std::printf("All ScriptLib smoke tests passed!\n");
    return 0;
}
