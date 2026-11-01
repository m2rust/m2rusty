#include <Client/UI/StdAfx.h>
#include <Client/Game/GameEventManager.h>
#include <math.h>

PyObject *eventMgrUpdate(PyObject *poSelf, PyObject *poArgs)
{
    float fx = NAN;
    if (!PyTuple_GetFloat(poArgs, 0, &fx))
        return Py_BuildException();
    float fy = NAN;
    if (!PyTuple_GetFloat(poArgs, 1, &fy))
        return Py_BuildException();
    float fz = NAN;
    if (!PyTuple_GetFloat(poArgs, 2, &fz))
        return Py_BuildException();

    CGameEventManager::Instance().SetCenterPosition(fx, fy, fz);
    CGameEventManager::Instance().Update();
    return Py_BuildNone();
}

void initeventmgr()
{
    static PyMethodDef s_methods[] = {
        {"Update", eventMgrUpdate, METH_VARARGS},
        {nullptr, nullptr, NULL},
    };

    Py_InitModule("eventMgr", s_methods);
}
