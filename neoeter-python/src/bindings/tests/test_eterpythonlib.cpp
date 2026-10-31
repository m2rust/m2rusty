#include <Neoeter/Python/Bindings/StdAfx.h>
#include <Neoeter/Python/Bindings/PythonWindow.h>
#include <Neoeter/Python/Host/PythonLauncher.h>
#include <Neoeter/Python/Host/PythonUtils.h>
#include <cassert>
#include <cmath>
#include <cstdio>
#include <cstring>
#include <string>

namespace
{

void test_wndmgr_window_lifecycle_and_properties()
{
    CPythonLauncher& launcher = CPythonLauncher::Instance();

    // 1. Define and instantiate a window in Python
    bool runOk = launcher.RunLine(
        "import wndMgr\n"
        "\n"
        "class BasicWindow(object):\n"
        "    def __init__(self):\n"
        "        self.hWnd = wndMgr.Register(self, 'GAME')\n"
        "        wndMgr.SetName(self.hWnd, 'TestWindow')\n"
        "        wndMgr.SetWindowPosition(self.hWnd, 150, 250)\n"
        "        wndMgr.SetWindowSize(self.hWnd, 320, 240)\n"
        "        wndMgr.Show(self.hWnd)\n"
        "\n"
        "win = BasicWindow()\n"
        "pos_x, pos_y = wndMgr.GetWindowLocalPosition(win.hWnd)\n"
        "win_name = wndMgr.GetName(win.hWnd)\n"
        "is_shown = wndMgr.IsShow(win.hWnd)\n"
    );
    assert(runOk);

    PyObject* mainMod = PyImport_AddModule("__main__");
    assert(mainMod != nullptr);
    PyObject* dict = PyModule_GetDict(mainMod);
    assert(dict != nullptr);

    PyObject* pyPosX = PyDict_GetItemString(dict, "pos_x");
    PyObject* pyPosY = PyDict_GetItemString(dict, "pos_y");
    assert(pyPosX && pyPosY);
    assert(PyInt_AsLong(pyPosX) == 150);
    assert(PyInt_AsLong(pyPosY) == 250);

    PyObject* pyName = PyDict_GetItemString(dict, "win_name");
    assert(pyName && std::strcmp(PyString_AsString(pyName), "TestWindow") == 0);

    PyObject* pyShow = PyDict_GetItemString(dict, "is_shown");
    assert(pyShow && PyInt_AsLong(pyShow) == 1);

    // Verify handle in C++ side
    PyObject* pyWin = PyDict_GetItemString(dict, "win");
    assert(pyWin != nullptr);
    PyObject* pyHwnd = PyObject_GetAttrString(pyWin, "hWnd");
    assert(pyHwnd != nullptr);

    long handleVal = PyLong_AsLong(pyHwnd);
    assert(handleVal != 0);

    UI::CWindow* pCppWindow = reinterpret_cast<UI::CWindow*>(handleVal);
    assert(pCppWindow != nullptr);
    assert(std::strcmp(pCppWindow->GetName(), "TestWindow") == 0);
    assert(pCppWindow->GetWidth() == 320);
    assert(pCppWindow->GetHeight() == 240);
    assert(pCppWindow->IsShow() == true);

    std::printf("[PASS] test_wndmgr_window_lifecycle_and_properties\n");
}

void test_wndmgr_hierarchy_and_nesting()
{
    CPythonLauncher& launcher = CPythonLauncher::Instance();

    bool runOk = launcher.RunLine(
        "class ParentWindow(object):\n"
        "    def __init__(self):\n"
        "        self.hWnd = wndMgr.Register(self, 'GAME')\n"
        "        wndMgr.SetName(self.hWnd, 'Parent')\n"
        "\n"
        "class ChildWindow(object):\n"
        "    def __init__(self, parent_hwnd):\n"
        "        self.hWnd = wndMgr.Register(self, 'GAME')\n"
        "        wndMgr.SetName(self.hWnd, 'Child')\n"
        "        wndMgr.SetParent(self.hWnd, parent_hwnd)\n"
        "\n"
        "parent_obj = ParentWindow()\n"
        "child_obj1 = ChildWindow(parent_obj.hWnd)\n"
        "child_obj2 = ChildWindow(parent_obj.hWnd)\n"
        "child_count = wndMgr.GetChildCount(parent_obj.hWnd)\n"
    );
    assert(runOk);

    PyObject* dict = PyModule_GetDict(PyImport_AddModule("__main__"));
    PyObject* pyChildCount = PyDict_GetItemString(dict, "child_count");
    assert(pyChildCount != nullptr);
    assert(PyInt_AsLong(pyChildCount) == 2);

    std::printf("[PASS] test_wndmgr_hierarchy_and_nesting\n");
}

void test_wndmgr_controls()
{
    CPythonLauncher& launcher = CPythonLauncher::Instance();

    // 1. TextLine control
    bool runOk = launcher.RunLine(
        "class LabelWindow(object):\n"
        "    def __init__(self):\n"
        "        self.hWnd = wndMgr.RegisterTextLine(self, 'GAME')\n"
        "        wndMgr.SetText(self.hWnd, 'Linux Client TextLine')\n"
        "\n"
        "label = LabelWindow()\n"
        "label_text = wndMgr.GetText(label.hWnd)\n"
    );
    assert(runOk);

    PyObject* dict = PyModule_GetDict(PyImport_AddModule("__main__"));
    PyObject* pyText = PyDict_GetItemString(dict, "label_text");
    assert(pyText != nullptr);
    assert(std::strcmp(PyString_AsString(pyText), "Linux Client TextLine") == 0);

    // 2. Button control
    runOk = launcher.RunLine(
        "class ButtonWindow(object):\n"
        "    def __init__(self):\n"
        "        self.hWnd = wndMgr.RegisterButton(self, 'GAME')\n"
        "\n"
        "button = ButtonWindow()\n"
        "is_pressed = wndMgr.IsDown(button.hWnd)\n"
    );
    assert(runOk);

    PyObject* pyPressed = PyDict_GetItemString(dict, "is_pressed");
    assert(pyPressed != nullptr);
    assert(PyInt_AsLong(pyPressed) == 0);

    std::printf("[PASS] test_wndmgr_controls\n");
}

void test_window_event_routing()
{
    CPythonLauncher& launcher = CPythonLauncher::Instance();

    bool runOk = launcher.RunLine(
        "class EventHandler(object):\n"
        "    def __init__(self):\n"
        "        self.hWnd = wndMgr.Register(self, 'GAME')\n"
        "        self.click_received = False\n"
        "        self.last_move = (0, 0)\n"
        "    def OnMouseLeftButtonDown(self):\n"
        "        self.click_received = True\n"
        "        return True\n"
        "    def OnMoveWindow(self, x, y):\n"
        "        self.last_move = (x, y)\n"
        "        return True\n"
        "\n"
        "handler = EventHandler()\n"
    );
    assert(runOk);

    PyObject* dict = PyModule_GetDict(PyImport_AddModule("__main__"));
    PyObject* pyHandler = PyDict_GetItemString(dict, "handler");
    assert(pyHandler != nullptr);
    PyObject* pyHwnd = PyObject_GetAttrString(pyHandler, "hWnd");
    assert(pyHwnd != nullptr);

    UI::CWindow* pWin = reinterpret_cast<UI::CWindow*>(PyLong_AsLong(pyHwnd));
    assert(pWin != nullptr);

    // Dispatch click event from C++
    pWin->OnMouseLeftButtonDown();

    // Dispatch move window event from C++
    pWin->OnMoveWindow(42, 84);

    runOk = launcher.RunLine(
        "handler_clicked = handler.click_received\n"
        "move_x, move_y = handler.last_move\n"
    );
    assert(runOk);

    PyObject* pyClicked = PyDict_GetItemString(dict, "handler_clicked");
    assert(pyClicked != nullptr);
    assert(PyObject_IsTrue(pyClicked) == 1);

    PyObject* pyMoveX = PyDict_GetItemString(dict, "move_x");
    PyObject* pyMoveY = PyDict_GetItemString(dict, "move_y");
    assert(pyMoveX && pyMoveY);
    assert(PyInt_AsLong(pyMoveX) == 42);
    assert(PyInt_AsLong(pyMoveY) == 84);

    std::printf("[PASS] test_window_event_routing\n");
}

void test_grp_module()
{
    CPythonLauncher& launcher = CPythonLauncher::Instance();

    bool runOk = launcher.RunLine(
        "import grp\n"
        "color_val = grp.GenerateColor(1.0, 0.5, 0.25, 1.0)\n"
    );
    assert(runOk);

    PyObject* dict = PyModule_GetDict(PyImport_AddModule("__main__"));
    PyObject* pyColor = PyDict_GetItemString(dict, "color_val");
    assert(pyColor != nullptr);

    DWORD expectedColor = CPythonGraphic::Instance().GenerateColor(1.0f, 0.5f, 0.25f, 1.0f);
    assert(static_cast<DWORD>(PyLong_AsUnsignedLong(pyColor)) == expectedColor);

    std::printf("[PASS] test_grp_module\n");
}

} // namespace

int main()
{
    CPythonGraphic graphic;
    UI::CWindowManager windowManager;

    CPythonLauncher launcher;
    bool created = launcher.Create("test.eterpython");
    assert(created);

    initwndMgr();
    initgrp();
    initgrpImage();
    initgrpText();
    initgrpThing();

    test_wndmgr_window_lifecycle_and_properties();
    test_wndmgr_hierarchy_and_nesting();
    test_wndmgr_controls();
    test_window_event_routing();
    test_grp_module();

    std::printf("All EterPythonLib tests passed successfully!\n");
    return 0;
}
