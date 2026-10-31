#include <Client/WebBrowser/CWebBrowser.h>

#include <cstring>

static int gs_bVisible = 0;
static RECT gs_rcWebBrowser = { 0, 0, 0, 0 };
static char gs_szCurrentUrl[1024] = { 0 };

extern "C" {

int WebBrowser_Startup(HINSTANCE hInstance)
{
    (void)hInstance;
    gs_bVisible = 0;
    std::memset(&gs_rcWebBrowser, 0, sizeof(gs_rcWebBrowser));
    std::memset(gs_szCurrentUrl, 0, sizeof(gs_szCurrentUrl));
    return 1;
}

void WebBrowser_Cleanup()
{
    WebBrowser_Hide();
}

void WebBrowser_Destroy()
{
    WebBrowser_Hide();
}

int WebBrowser_Show(HWND parent, const char* addr, const RECT* rcWebBrowser)
{
    (void)parent;
    if (gs_bVisible)
        return 0;

    if (!addr || !rcWebBrowser)
        return 0;

    std::strncpy(gs_szCurrentUrl, addr, sizeof(gs_szCurrentUrl) - 1);
    gs_szCurrentUrl[sizeof(gs_szCurrentUrl) - 1] = '\0';
    gs_rcWebBrowser = *rcWebBrowser;
    gs_bVisible = 1;
    return 1;
}

void WebBrowser_Hide()
{
    if (!gs_bVisible)
        return;

    gs_bVisible = 0;
}

void WebBrowser_Move(const RECT* rcWebBrowser)
{
    if (!rcWebBrowser)
        return;

    gs_rcWebBrowser = *rcWebBrowser;
}

int WebBrowser_IsVisible()
{
    return gs_bVisible;
}

}

const RECT& WebBrowser_GetRect()
{
    return gs_rcWebBrowser;
}
