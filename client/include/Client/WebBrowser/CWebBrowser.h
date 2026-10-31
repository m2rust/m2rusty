#pragma once

#if (defined(_WIN32) || defined(_WIN64)) && !defined(USE_PLATFORM_COMPAT)
    #define WIN32_LEAN_AND_MEAN
    #include <windows.h>
#else
    #include <Platform/Platform.h>
#endif

#ifdef __cplusplus
extern "C" {
#endif

int		WebBrowser_Startup(HINSTANCE hInstance);
void	WebBrowser_Cleanup();
void	WebBrowser_Destroy();
int		WebBrowser_Show(HWND parent, const char* addr, const RECT* rcWebBrowser);
void	WebBrowser_Hide();
void	WebBrowser_Move(const RECT* rcWebBrowser);

int		WebBrowser_IsVisible();

#ifdef __cplusplus
}
const RECT& WebBrowser_GetRect();
#endif
