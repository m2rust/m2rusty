#include <Neoeter/Python/Bindings/StdAfx.h>
#include <Neoeter/Python/Host/PythonLauncher.h>

#include <support/PythonApiSnapshot.h>
#include <support/TestAssert.h>

#include <cstdio>
#include <vector>

// TA.6: boot the embedded Python 2.7 interpreter without a display, register
// the bindings owned by EterPythonLib and pin the exposed API surface.
int main(int argc, char** argv)
{
    TEST_ASSERT_MSG(argc == 2,
                    "usage: test_python_eterpythonlib_api <golden>");

    TEST_SECTION("EterPythonLib embedded interpreter");
    CPythonLauncher launcher;
    TEST_ASSERT_MSG(launcher.Create("test.python.eterpythonlib"),
                    "CPythonLauncher::Create failed");
    TEST_PASS("interpreter booted (Py_NoSiteFlag=1, no display)");

    initgrp();
    initgrpImage();
    initgrpText();
    initgrpThing();
    initwndMgr();

    const std::vector<const char*> modules = {
        "grp", "grpImage", "grpText", "grpThing", "wndMgr"};
    PythonApiSnapshot::Capture("EterPythonLib", modules, argv[1]);

    TEST_PASS("grp, grpImage, grpText, grpThing and wndMgr import");
    return 0;
}
