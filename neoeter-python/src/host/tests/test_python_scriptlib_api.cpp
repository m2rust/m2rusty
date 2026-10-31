#include <Neoeter/Python/Host/StdAfx.h>
#include <Neoeter/Python/Host/PythonLauncher.h>

#include <support/PythonApiSnapshot.h>
#include <support/TestAssert.h>

#include <cstdio>
#include <vector>

// TA.6: boot the embedded Python 2.7 interpreter without a display, register
// the modules owned by ScriptLib and pin the exposed API surface.
int main(int argc, char** argv)
{
    TEST_ASSERT_MSG(argc == 2, "usage: test_python_scriptlib_api <golden>");

    TEST_SECTION("ScriptLib embedded interpreter");
    CPythonLauncher launcher;
    TEST_ASSERT_MSG(launcher.Create("test.python.scriptlib"),
                    "CPythonLauncher::Create failed");
    TEST_PASS("interpreter booted (Py_NoSiteFlag=1, no display)");

    initdbg();

    const std::vector<const char*> modules = {"dbg", "math", "time"};
    PythonApiSnapshot::Capture("ScriptLib", modules, argv[1]);

    TEST_PASS("dbg, math and time import");
    return 0;
}
