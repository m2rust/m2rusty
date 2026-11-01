#include <Neoeter/Render/StdAfx.h>
#include <Neoeter/Render/Input.h>

#include <SDL2/SDL.h>

#include <array>

// T1.3: SDL2 is the only keyboard backend on every target. DirectInput's
// device object, coop level and re-acquire dance are gone; the game keeps
// seeing the same DIK_* codes and edge events it always did.

namespace
{

struct ScancodeMapping
{
    int scancode;
    int dik;
};

// SDL scancodes are USB HID usages, DIK codes are AT set-1 scancodes. Both
// describe a physical position, so this table is layout-independent: the key
// labelled "A" on QWERTY and the key labelled "Q" on AZERTY share scancode 4
// and therefore DIK_A.
//
// Keys with no sane DIK equivalent stay unmapped rather than guessed: the
// ABNT extras (DIK_ABNT_C1/C2), the Japanese conversion keys (DIK_CONVERT /
// DIK_NOCONVERT) and the xterm-style AC keys (execute, undo, cut, copy,
// paste, find) are never read by the client.
const ScancodeMapping kScancodeToDik[] = {
    {.scancode = SDL_SCANCODE_ESCAPE, .dik = DIK_ESCAPE},

    {.scancode = SDL_SCANCODE_1, .dik = DIK_1},
    {.scancode = SDL_SCANCODE_2, .dik = DIK_2},
    {.scancode = SDL_SCANCODE_3, .dik = DIK_3},
    {.scancode = SDL_SCANCODE_4, .dik = DIK_4},
    {.scancode = SDL_SCANCODE_5, .dik = DIK_5},
    {.scancode = SDL_SCANCODE_6, .dik = DIK_6},
    {.scancode = SDL_SCANCODE_7, .dik = DIK_7},
    {.scancode = SDL_SCANCODE_8, .dik = DIK_8},
    {.scancode = SDL_SCANCODE_9, .dik = DIK_9},
    {.scancode = SDL_SCANCODE_0, .dik = DIK_0},

    {.scancode = SDL_SCANCODE_MINUS, .dik = DIK_MINUS},
    {.scancode = SDL_SCANCODE_EQUALS, .dik = DIK_EQUALS},
    {.scancode = SDL_SCANCODE_BACKSPACE, .dik = DIK_BACK},
    {.scancode = SDL_SCANCODE_TAB, .dik = DIK_TAB},

    {.scancode = SDL_SCANCODE_Q, .dik = DIK_Q},
    {.scancode = SDL_SCANCODE_W, .dik = DIK_W},
    {.scancode = SDL_SCANCODE_E, .dik = DIK_E},
    {.scancode = SDL_SCANCODE_R, .dik = DIK_R},
    {.scancode = SDL_SCANCODE_T, .dik = DIK_T},
    {.scancode = SDL_SCANCODE_Y, .dik = DIK_Y},
    {.scancode = SDL_SCANCODE_U, .dik = DIK_U},
    {.scancode = SDL_SCANCODE_I, .dik = DIK_I},
    {.scancode = SDL_SCANCODE_O, .dik = DIK_O},
    {.scancode = SDL_SCANCODE_P, .dik = DIK_P},

    {.scancode = SDL_SCANCODE_LEFTBRACKET, .dik = DIK_LBRACKET},
    {.scancode = SDL_SCANCODE_RIGHTBRACKET, .dik = DIK_RBRACKET},
    {.scancode = SDL_SCANCODE_RETURN, .dik = DIK_RETURN},
    {.scancode = SDL_SCANCODE_LCTRL, .dik = DIK_LCONTROL},

    {.scancode = SDL_SCANCODE_A, .dik = DIK_A},
    {.scancode = SDL_SCANCODE_S, .dik = DIK_S},
    {.scancode = SDL_SCANCODE_D, .dik = DIK_D},
    {.scancode = SDL_SCANCODE_F, .dik = DIK_F},
    {.scancode = SDL_SCANCODE_G, .dik = DIK_G},
    {.scancode = SDL_SCANCODE_H, .dik = DIK_H},
    {.scancode = SDL_SCANCODE_J, .dik = DIK_J},
    {.scancode = SDL_SCANCODE_K, .dik = DIK_K},
    {.scancode = SDL_SCANCODE_L, .dik = DIK_L},

    {.scancode = SDL_SCANCODE_SEMICOLON, .dik = DIK_SEMICOLON},
    {.scancode = SDL_SCANCODE_APOSTROPHE, .dik = DIK_APOSTROPHE},
    {.scancode = SDL_SCANCODE_GRAVE, .dik = DIK_GRAVE},
    {.scancode = SDL_SCANCODE_LSHIFT, .dik = DIK_LSHIFT},
    {.scancode = SDL_SCANCODE_BACKSLASH, .dik = DIK_BACKSLASH},

    {.scancode = SDL_SCANCODE_Z, .dik = DIK_Z},
    {.scancode = SDL_SCANCODE_X, .dik = DIK_X},
    {.scancode = SDL_SCANCODE_C, .dik = DIK_C},
    {.scancode = SDL_SCANCODE_V, .dik = DIK_V},
    {.scancode = SDL_SCANCODE_B, .dik = DIK_B},
    {.scancode = SDL_SCANCODE_N, .dik = DIK_N},
    {.scancode = SDL_SCANCODE_M, .dik = DIK_M},

    {.scancode = SDL_SCANCODE_COMMA, .dik = DIK_COMMA},
    {.scancode = SDL_SCANCODE_PERIOD, .dik = DIK_PERIOD},
    {.scancode = SDL_SCANCODE_SLASH, .dik = DIK_SLASH},
    {.scancode = SDL_SCANCODE_RSHIFT, .dik = DIK_RSHIFT},

    {.scancode = SDL_SCANCODE_KP_MULTIPLY, .dik = DIK_MULTIPLY},
    {.scancode = SDL_SCANCODE_LALT, .dik = DIK_LMENU},
    {.scancode = SDL_SCANCODE_SPACE, .dik = DIK_SPACE},
    {.scancode = SDL_SCANCODE_CAPSLOCK, .dik = DIK_CAPITAL},

    {.scancode = SDL_SCANCODE_F1, .dik = DIK_F1},
    {.scancode = SDL_SCANCODE_F2, .dik = DIK_F2},
    {.scancode = SDL_SCANCODE_F3, .dik = DIK_F3},
    {.scancode = SDL_SCANCODE_F4, .dik = DIK_F4},
    {.scancode = SDL_SCANCODE_F5, .dik = DIK_F5},
    {.scancode = SDL_SCANCODE_F6, .dik = DIK_F6},
    {.scancode = SDL_SCANCODE_F7, .dik = DIK_F7},
    {.scancode = SDL_SCANCODE_F8, .dik = DIK_F8},
    {.scancode = SDL_SCANCODE_F9, .dik = DIK_F9},
    {.scancode = SDL_SCANCODE_F10, .dik = DIK_F10},

    {.scancode = SDL_SCANCODE_NUMLOCKCLEAR, .dik = DIK_NUMLOCK},
    {.scancode = SDL_SCANCODE_SCROLLLOCK, .dik = DIK_SCROLL},

    // Keypad: DIK follows the physical 7-8-9 / 4-5-6 / 1-2-3 grid.
    {.scancode = SDL_SCANCODE_KP_7, .dik = DIK_NUMPAD7},
    {.scancode = SDL_SCANCODE_KP_8, .dik = DIK_NUMPAD8},
    {.scancode = SDL_SCANCODE_KP_9, .dik = DIK_NUMPAD9},
    {.scancode = SDL_SCANCODE_KP_MINUS, .dik = DIK_SUBTRACT},
    {.scancode = SDL_SCANCODE_KP_4, .dik = DIK_NUMPAD4},
    {.scancode = SDL_SCANCODE_KP_5, .dik = DIK_NUMPAD5},
    {.scancode = SDL_SCANCODE_KP_6, .dik = DIK_NUMPAD6},
    {.scancode = SDL_SCANCODE_KP_PLUS, .dik = DIK_ADD},
    {.scancode = SDL_SCANCODE_KP_1, .dik = DIK_NUMPAD1},
    {.scancode = SDL_SCANCODE_KP_2, .dik = DIK_NUMPAD2},
    {.scancode = SDL_SCANCODE_KP_3, .dik = DIK_NUMPAD3},
    {.scancode = SDL_SCANCODE_KP_0, .dik = DIK_NUMPAD0},
    {.scancode = SDL_SCANCODE_KP_PERIOD, .dik = DIK_DECIMAL},

    // ISO layouts put a second backslash key left of Z / right of LShift.
    {.scancode = SDL_SCANCODE_NONUSBACKSLASH, .dik = DIK_OEM_102},

    {.scancode = SDL_SCANCODE_F11, .dik = DIK_F11},
    {.scancode = SDL_SCANCODE_F12, .dik = DIK_F12},
    {.scancode = SDL_SCANCODE_F13, .dik = DIK_F13},
    {.scancode = SDL_SCANCODE_F14, .dik = DIK_F14},
    {.scancode = SDL_SCANCODE_F15, .dik = DIK_F15},

    {.scancode = SDL_SCANCODE_KP_EQUALS, .dik = DIK_NUMPADEQUALS},
    {.scancode = SDL_SCANCODE_KP_COMMA, .dik = DIK_NUMPADCOMMA},
    {.scancode = SDL_SCANCODE_KP_DIVIDE, .dik = DIK_DIVIDE},
    {.scancode = SDL_SCANCODE_KP_ENTER, .dik = DIK_NUMPADENTER},
    {.scancode = SDL_SCANCODE_RCTRL, .dik = DIK_RCONTROL},
    {.scancode = SDL_SCANCODE_PRINTSCREEN, .dik = DIK_SYSRQ},
    {.scancode = SDL_SCANCODE_RALT, .dik = DIK_RMENU},
    {.scancode = SDL_SCANCODE_PAUSE, .dik = DIK_PAUSE},

    {.scancode = SDL_SCANCODE_HOME, .dik = DIK_HOME},
    {.scancode = SDL_SCANCODE_UP, .dik = DIK_UP},
    {.scancode = SDL_SCANCODE_PAGEUP, .dik = DIK_PRIOR},
    {.scancode = SDL_SCANCODE_LEFT, .dik = DIK_LEFT},
    {.scancode = SDL_SCANCODE_RIGHT, .dik = DIK_RIGHT},
    {.scancode = SDL_SCANCODE_END, .dik = DIK_END},
    {.scancode = SDL_SCANCODE_DOWN, .dik = DIK_DOWN},
    {.scancode = SDL_SCANCODE_PAGEDOWN, .dik = DIK_NEXT},
    {.scancode = SDL_SCANCODE_INSERT, .dik = DIK_INSERT},
    {.scancode = SDL_SCANCODE_DELETE, .dik = DIK_DELETE},

    {.scancode = SDL_SCANCODE_LGUI, .dik = DIK_LWIN},
    {.scancode = SDL_SCANCODE_RGUI, .dik = DIK_RWIN},
    {.scancode = SDL_SCANCODE_APPLICATION, .dik = DIK_APPS},
    {.scancode = SDL_SCANCODE_POWER, .dik = DIK_POWER},
    {.scancode = SDL_SCANCODE_SLEEP, .dik = DIK_SLEEP},

    // Media / browser keys the Python bindings already expose.
    {.scancode = SDL_SCANCODE_MUTE, .dik = DIK_MUTE},
    {.scancode = SDL_SCANCODE_VOLUMEUP, .dik = DIK_VOLUMEUP},
    {.scancode = SDL_SCANCODE_VOLUMEDOWN, .dik = DIK_VOLUMEDOWN},
    {.scancode = SDL_SCANCODE_AUDIONEXT, .dik = DIK_NEXTTRACK},
    {.scancode = SDL_SCANCODE_AUDIOPREV, .dik = DIK_PREVTRACK},
    {.scancode = SDL_SCANCODE_AUDIOSTOP, .dik = DIK_MEDIASTOP},
    {.scancode = SDL_SCANCODE_AUDIOPLAY, .dik = DIK_PLAYPAUSE},
    {.scancode = SDL_SCANCODE_MEDIASELECT, .dik = DIK_MEDIASELECT},
    {.scancode = SDL_SCANCODE_WWW, .dik = DIK_WEBHOME},
    {.scancode = SDL_SCANCODE_MAIL, .dik = DIK_MAIL},
    {.scancode = SDL_SCANCODE_CALCULATOR, .dik = DIK_CALCULATOR},
    {.scancode = SDL_SCANCODE_COMPUTER, .dik = DIK_MYCOMPUTER},
    {.scancode = SDL_SCANCODE_AC_SEARCH, .dik = DIK_WEBSEARCH},
    {.scancode = SDL_SCANCODE_AC_BACK, .dik = DIK_WEBBACK},
    {.scancode = SDL_SCANCODE_AC_FORWARD, .dik = DIK_WEBFORWARD},
    {.scancode = SDL_SCANCODE_AC_STOP, .dik = DIK_WEBSTOP},
    {.scancode = SDL_SCANCODE_AC_REFRESH, .dik = DIK_WEBREFRESH},
    {.scancode = SDL_SCANCODE_AC_BOOKMARKS, .dik = DIK_WEBFAVORITES},
    {.scancode = SDL_SCANCODE_STOP, .dik = DIK_STOP},

    // Layout-specific keys. Text entry goes through SDL_TEXTINPUT (T1.6), so
    // these are only interesting to code that polls IsPressed().
    {.scancode = SDL_SCANCODE_INTERNATIONAL3, .dik = DIK_YEN},
    {.scancode = SDL_SCANCODE_LANG1, .dik = DIK_KANA},
    {.scancode = SDL_SCANCODE_LANG2, .dik = DIK_KANJI},
};

using DikTable = std::array<unsigned char, SDL_NUM_SCANCODES>;

const DikTable &GetDikTable()
{
    static const DikTable table = []() {
        DikTable result{};
        for (const ScancodeMapping &mapping : kScancodeToDik)
            result[mapping.scancode] = static_cast<unsigned char>(mapping.dik);
        return result;
    }();
    return table;
}

} // namespace

bool CInputKeyboard::ms_bPressedKey[256];
char CInputKeyboard::ms_diks[256];

CInputKeyboard::CInputKeyboard()
{
    ResetKeyboard();
}

CInputKeyboard::~CInputKeyboard() = default;

bool CInputKeyboard::InitializeKeyboard(HWND /*hWnd*/)
{
    // SDL2 drives the device from the window/event loop (CMSWindow::Create,
    // CMSApplication::MessageProcess); there is nothing to acquire here.
    ResetKeyboard();
    return true;
}

void CInputKeyboard::ResetKeyboard()
{
    memset(ms_diks, 0, sizeof(ms_diks));
    memset(ms_bPressedKey, 0, sizeof(ms_bPressedKey));
}

int CInputKeyboard::ScancodeToDik(int iScancode)
{
    if (iScancode < 0 || iScancode >= SDL_NUM_SCANCODES)
        return -1;

    const unsigned char dik = GetDikTable()[iScancode];
    return dik ? static_cast<int>(dik) : -1;
}

void CInputKeyboard::UpdateKeyboardFromState(const unsigned char *pKeyState)
{
    const DikTable &table = GetDikTable();

    if (pKeyState)
    {
        for (int scancode = 0; scancode < SDL_NUM_SCANCODES; ++scancode)
        {
            const unsigned char dik = table[scancode];
            if (!dik)
                continue;

            ms_diks[dik] = pKeyState[scancode] ? static_cast<char>(0x80) : 0;
        }
    }
    else
    {
        // Focus loss: release everything so no key stays stuck down.
        memset(ms_diks, 0, sizeof(ms_diks));
    }

    for (int i = 0; i < 256; ++i)
    {
        if (ms_diks[i] & 0x80)
        {
            if (!IsPressed(i))
                KeyDown(i);
        }
        else if (IsPressed(i))
        {
            KeyUp(i);
        }
    }
}

void CInputKeyboard::UpdateKeyboard()
{
    // SDL resets the keyboard state when the window loses focus; a missing
    // focus window therefore means "all released". DirectInput needed an
    // explicit Unacquire/Acquire round trip to get the same effect.
    if (SDL_GetKeyboardFocus() == nullptr)
    {
        UpdateKeyboardFromState(nullptr);
        return;
    }

    UpdateKeyboardFromState(SDL_GetKeyboardState(nullptr));
}

void CInputKeyboard::KeyDown(int iIndex)
{
    ms_bPressedKey[iIndex] = true;
    OnKeyDown(iIndex);
}

void CInputKeyboard::KeyUp(int iIndex)
{
    ms_bPressedKey[iIndex] = false;
    OnKeyUp(iIndex);
}

bool CInputKeyboard::IsPressed(int iIndex)
{
    return ms_bPressedKey[iIndex];
}
