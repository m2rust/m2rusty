#pragma once

// SDL hardware cursors (T3.2).
//
// The original client loaded its cursor bitmaps from Win32 resources
// (`LoadImage(..., IMAGE_CURSOR, ...)`). No resource script ships with the
// sources, so on the Windows target that call always fails and the client stops
// with a "CREATE_CURSOR" message box before the login screen. Since T1.2/T1.3
// both targets use SDL for the window and input, use SDL system cursors instead.
//
// The helpers keep the Windows-only SetCursor/ShowCursor/DestroyCursor calls in
// one place so the call sites stay platform-neutral.

#include <SDL2/SDL.h>

inline HANDLE Platform_CreateSystemCursor(SDL_SystemCursor cursor)
{
    return reinterpret_cast<HANDLE>(SDL_CreateSystemCursor(cursor));
}

inline void Platform_DestroyCursor(HANDLE handle)
{
    if (handle)
        SDL_FreeCursor(reinterpret_cast<SDL_Cursor*>(handle));
}

inline void Platform_SetCursorHandle(HANDLE handle)
{
    SDL_SetCursor(reinterpret_cast<SDL_Cursor*>(handle));
}

// SDL_ShowCursor returns the previous 0/1 state, not the Win32 reference
// counter; callers must not drive a do/while loop with it.
inline int Platform_ShowCursorFlag(BOOL show)
{
    return SDL_ShowCursor(show ? SDL_ENABLE : SDL_DISABLE);
}

#if defined(_WIN32) || defined(_WIN64)
// The engine's SdlPlatform.h owns the real implementation; the client does not
// include it, so keep the Windows resize hook identical to the Linux branch.
inline void Platform_SetWindowResolution(int /*w*/, int /*h*/) {}
#endif
