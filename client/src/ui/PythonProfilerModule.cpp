#include <Client/UI/StdAfx.h>
#include <Neoeter/Render/Profiler.h>

PyObject *profilerPush(PyObject *poSelf, PyObject *poArgs)
{
    char *szName = nullptr;
    if (!PyTuple_GetString(poArgs, 0, &szName))
        return Py_BuildException();

    return Py_BuildNone();
}

PyObject *profilerPop(PyObject *poSelf, PyObject *poArgs)
{
    char *szName = nullptr;
    if (!PyTuple_GetString(poArgs, 0, &szName))
        return Py_BuildException();

    return Py_BuildNone();
}

void initProfiler()
{
    static PyMethodDef s_methods[] = {
        {"Push", profilerPush, METH_VARARGS},
        {"Pop", profilerPop, METH_VARARGS},

        {nullptr, nullptr, NULL},
    };

    Py_InitModule("profiler", s_methods);
}