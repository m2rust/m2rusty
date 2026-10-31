#include <Client/WebBrowser/CWebBrowser.h>

#include <cassert>
#include <cstdio>

int main()
{
    printf("[test_webbrowser] Running CWebBrowser unit tests...\n");

    assert(WebBrowser_Startup(nullptr) == 1);
    assert(WebBrowser_IsVisible() == 0);

    RECT rcInitial = { 10, 20, 810, 620 };
    assert(WebBrowser_Show(nullptr, "https://example.com/itemshop", &rcInitial) == 1);
    assert(WebBrowser_IsVisible() == 1);

    // Calling Show while already visible should return 0
    assert(WebBrowser_Show(nullptr, "https://example.com/other", &rcInitial) == 0);

    const RECT& rcCurrent = WebBrowser_GetRect();
    assert(rcCurrent.left == 10 && rcCurrent.top == 20 && rcCurrent.right == 810 && rcCurrent.bottom == 620);

    RECT rcMoved = { 50, 60, 1050, 760 };
    WebBrowser_Move(&rcMoved);
    const RECT& rcAfterMove = WebBrowser_GetRect();
    assert(rcAfterMove.left == 50 && rcAfterMove.top == 60 && rcAfterMove.right == 1050 && rcAfterMove.bottom == 760);

    WebBrowser_Hide();
    assert(WebBrowser_IsVisible() == 0);

    WebBrowser_Destroy();
    WebBrowser_Cleanup();
    printf("  [PASS] Lifecycle, visibility state, and rect tracking\n");

    printf("[test_webbrowser] All tests passed successfully!\n");
    return 0;
}
