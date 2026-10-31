#pragma once

// SDL window-state helpers.
//
// Platform.h provides these only on its Linux/POSIX compatibility path (the
// Windows branch is the real Win32 API). Since T1.2 the window and event loop
// are SDL on every target, so the Windows build needs the same state helpers.
// They are defined here for Windows only, to avoid clashing with the inline
// definitions in Platform.h on Linux.
//
// TB.3 removes this duplication when the platform headers are consolidated.

#if defined(_WIN32)

#include <atomic>

#ifndef OPENMT2_SDL_PLATFORM_STATE_DEFINED
#define OPENMT2_SDL_PLATFORM_STATE_DEFINED

struct PlatformWindowState
{
    int windowWidth = 0;
    int windowHeight = 0;
    int mouseX = 0;
    int mouseY = 0;
    HWND hwnd = (HWND)0;
};

inline PlatformWindowState& GetPlatformWindowState()
{
    static PlatformWindowState s_ws;
    return s_ws;
}

inline void Platform_SetCursorPos(int x, int y)
{
    GetPlatformWindowState().mouseX = x;
    GetPlatformWindowState().mouseY = y;
}

typedef void (*Platform_WarpMouseCallback)(int x, int y);
inline Platform_WarpMouseCallback& Platform_GetWarpMouseCallback()
{
    static Platform_WarpMouseCallback s_cb = nullptr;
    return s_cb;
}
inline void Platform_SetWarpMouseCallback(Platform_WarpMouseCallback cb)
{
    Platform_GetWarpMouseCallback() = cb;
}
inline void Platform_WarpMouse(int x, int y)
{
    auto cb = Platform_GetWarpMouseCallback();
    if (cb) cb(x, y);
}

inline void Platform_SetWindowResolution(int w, int h)
{
    GetPlatformWindowState().windowWidth = w;
    GetPlatformWindowState().windowHeight = h;
}
inline void Platform_GetWindowResolution(int* w, int* h)
{
    if (w) *w = GetPlatformWindowState().windowWidth;
    if (h) *h = GetPlatformWindowState().windowHeight;
}

typedef void (*Platform_QuitCallback)(int);
inline Platform_QuitCallback& Platform_GetQuitCallback()
{
    static Platform_QuitCallback s_cb = nullptr;
    return s_cb;
}
inline void Platform_SetQuitCallback(Platform_QuitCallback cb)
{
    Platform_GetQuitCallback() = cb;
}
inline std::atomic<bool>& Platform_GetQuitRequested()
{
    static std::atomic<bool> s_quitRequested(false);
    return s_quitRequested;
}
inline bool Platform_IsQuitRequested()
{
    return Platform_GetQuitRequested().load();
}

#endif // OPENMT2_SDL_PLATFORM_STATE_DEFINED
#endif // _WIN32
