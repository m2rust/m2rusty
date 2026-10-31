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
// Keys with no sane DIK equivalent stay unmapped rather than guessed: the ABNT
// extras (DIK_ABNT_C1/C2), the Japanese conversion keys (DIK_CONVERT /
// DIK_NOCONVERT) and the xterm-style AC keys (execute, undo, cut, copy, paste,
// find) are never read by the client.
const ScancodeMapping kScancodeToDik[] = {
	{ SDL_SCANCODE_ESCAPE, DIK_ESCAPE },

	{ SDL_SCANCODE_1, DIK_1 },
	{ SDL_SCANCODE_2, DIK_2 },
	{ SDL_SCANCODE_3, DIK_3 },
	{ SDL_SCANCODE_4, DIK_4 },
	{ SDL_SCANCODE_5, DIK_5 },
	{ SDL_SCANCODE_6, DIK_6 },
	{ SDL_SCANCODE_7, DIK_7 },
	{ SDL_SCANCODE_8, DIK_8 },
	{ SDL_SCANCODE_9, DIK_9 },
	{ SDL_SCANCODE_0, DIK_0 },

	{ SDL_SCANCODE_MINUS, DIK_MINUS },
	{ SDL_SCANCODE_EQUALS, DIK_EQUALS },
	{ SDL_SCANCODE_BACKSPACE, DIK_BACK },
	{ SDL_SCANCODE_TAB, DIK_TAB },

	{ SDL_SCANCODE_Q, DIK_Q },
	{ SDL_SCANCODE_W, DIK_W },
	{ SDL_SCANCODE_E, DIK_E },
	{ SDL_SCANCODE_R, DIK_R },
	{ SDL_SCANCODE_T, DIK_T },
	{ SDL_SCANCODE_Y, DIK_Y },
	{ SDL_SCANCODE_U, DIK_U },
	{ SDL_SCANCODE_I, DIK_I },
	{ SDL_SCANCODE_O, DIK_O },
	{ SDL_SCANCODE_P, DIK_P },

	{ SDL_SCANCODE_LEFTBRACKET, DIK_LBRACKET },
	{ SDL_SCANCODE_RIGHTBRACKET, DIK_RBRACKET },
	{ SDL_SCANCODE_RETURN, DIK_RETURN },
	{ SDL_SCANCODE_LCTRL, DIK_LCONTROL },

	{ SDL_SCANCODE_A, DIK_A },
	{ SDL_SCANCODE_S, DIK_S },
	{ SDL_SCANCODE_D, DIK_D },
	{ SDL_SCANCODE_F, DIK_F },
	{ SDL_SCANCODE_G, DIK_G },
	{ SDL_SCANCODE_H, DIK_H },
	{ SDL_SCANCODE_J, DIK_J },
	{ SDL_SCANCODE_K, DIK_K },
	{ SDL_SCANCODE_L, DIK_L },

	{ SDL_SCANCODE_SEMICOLON, DIK_SEMICOLON },
	{ SDL_SCANCODE_APOSTROPHE, DIK_APOSTROPHE },
	{ SDL_SCANCODE_GRAVE, DIK_GRAVE },
	{ SDL_SCANCODE_LSHIFT, DIK_LSHIFT },
	{ SDL_SCANCODE_BACKSLASH, DIK_BACKSLASH },

	{ SDL_SCANCODE_Z, DIK_Z },
	{ SDL_SCANCODE_X, DIK_X },
	{ SDL_SCANCODE_C, DIK_C },
	{ SDL_SCANCODE_V, DIK_V },
	{ SDL_SCANCODE_B, DIK_B },
	{ SDL_SCANCODE_N, DIK_N },
	{ SDL_SCANCODE_M, DIK_M },

	{ SDL_SCANCODE_COMMA, DIK_COMMA },
	{ SDL_SCANCODE_PERIOD, DIK_PERIOD },
	{ SDL_SCANCODE_SLASH, DIK_SLASH },
	{ SDL_SCANCODE_RSHIFT, DIK_RSHIFT },

	{ SDL_SCANCODE_KP_MULTIPLY, DIK_MULTIPLY },
	{ SDL_SCANCODE_LALT, DIK_LMENU },
	{ SDL_SCANCODE_SPACE, DIK_SPACE },
	{ SDL_SCANCODE_CAPSLOCK, DIK_CAPITAL },

	{ SDL_SCANCODE_F1, DIK_F1 },
	{ SDL_SCANCODE_F2, DIK_F2 },
	{ SDL_SCANCODE_F3, DIK_F3 },
	{ SDL_SCANCODE_F4, DIK_F4 },
	{ SDL_SCANCODE_F5, DIK_F5 },
	{ SDL_SCANCODE_F6, DIK_F6 },
	{ SDL_SCANCODE_F7, DIK_F7 },
	{ SDL_SCANCODE_F8, DIK_F8 },
	{ SDL_SCANCODE_F9, DIK_F9 },
	{ SDL_SCANCODE_F10, DIK_F10 },

	{ SDL_SCANCODE_NUMLOCKCLEAR, DIK_NUMLOCK },
	{ SDL_SCANCODE_SCROLLLOCK, DIK_SCROLL },

	// Keypad: DIK follows the physical 7-8-9 / 4-5-6 / 1-2-3 grid.
	{ SDL_SCANCODE_KP_7, DIK_NUMPAD7 },
	{ SDL_SCANCODE_KP_8, DIK_NUMPAD8 },
	{ SDL_SCANCODE_KP_9, DIK_NUMPAD9 },
	{ SDL_SCANCODE_KP_MINUS, DIK_SUBTRACT },
	{ SDL_SCANCODE_KP_4, DIK_NUMPAD4 },
	{ SDL_SCANCODE_KP_5, DIK_NUMPAD5 },
	{ SDL_SCANCODE_KP_6, DIK_NUMPAD6 },
	{ SDL_SCANCODE_KP_PLUS, DIK_ADD },
	{ SDL_SCANCODE_KP_1, DIK_NUMPAD1 },
	{ SDL_SCANCODE_KP_2, DIK_NUMPAD2 },
	{ SDL_SCANCODE_KP_3, DIK_NUMPAD3 },
	{ SDL_SCANCODE_KP_0, DIK_NUMPAD0 },
	{ SDL_SCANCODE_KP_PERIOD, DIK_DECIMAL },

	// ISO layouts put a second backslash key left of Z / right of LShift.
	{ SDL_SCANCODE_NONUSBACKSLASH, DIK_OEM_102 },

	{ SDL_SCANCODE_F11, DIK_F11 },
	{ SDL_SCANCODE_F12, DIK_F12 },
	{ SDL_SCANCODE_F13, DIK_F13 },
	{ SDL_SCANCODE_F14, DIK_F14 },
	{ SDL_SCANCODE_F15, DIK_F15 },

	{ SDL_SCANCODE_KP_EQUALS, DIK_NUMPADEQUALS },
	{ SDL_SCANCODE_KP_COMMA, DIK_NUMPADCOMMA },
	{ SDL_SCANCODE_KP_DIVIDE, DIK_DIVIDE },
	{ SDL_SCANCODE_KP_ENTER, DIK_NUMPADENTER },
	{ SDL_SCANCODE_RCTRL, DIK_RCONTROL },
	{ SDL_SCANCODE_PRINTSCREEN, DIK_SYSRQ },
	{ SDL_SCANCODE_RALT, DIK_RMENU },
	{ SDL_SCANCODE_PAUSE, DIK_PAUSE },

	{ SDL_SCANCODE_HOME, DIK_HOME },
	{ SDL_SCANCODE_UP, DIK_UP },
	{ SDL_SCANCODE_PAGEUP, DIK_PRIOR },
	{ SDL_SCANCODE_LEFT, DIK_LEFT },
	{ SDL_SCANCODE_RIGHT, DIK_RIGHT },
	{ SDL_SCANCODE_END, DIK_END },
	{ SDL_SCANCODE_DOWN, DIK_DOWN },
	{ SDL_SCANCODE_PAGEDOWN, DIK_NEXT },
	{ SDL_SCANCODE_INSERT, DIK_INSERT },
	{ SDL_SCANCODE_DELETE, DIK_DELETE },

	{ SDL_SCANCODE_LGUI, DIK_LWIN },
	{ SDL_SCANCODE_RGUI, DIK_RWIN },
	{ SDL_SCANCODE_APPLICATION, DIK_APPS },
	{ SDL_SCANCODE_POWER, DIK_POWER },
	{ SDL_SCANCODE_SLEEP, DIK_SLEEP },

	// Media / browser keys the Python bindings already expose.
	{ SDL_SCANCODE_MUTE, DIK_MUTE },
	{ SDL_SCANCODE_VOLUMEUP, DIK_VOLUMEUP },
	{ SDL_SCANCODE_VOLUMEDOWN, DIK_VOLUMEDOWN },
	{ SDL_SCANCODE_AUDIONEXT, DIK_NEXTTRACK },
	{ SDL_SCANCODE_AUDIOPREV, DIK_PREVTRACK },
	{ SDL_SCANCODE_AUDIOSTOP, DIK_MEDIASTOP },
	{ SDL_SCANCODE_AUDIOPLAY, DIK_PLAYPAUSE },
	{ SDL_SCANCODE_MEDIASELECT, DIK_MEDIASELECT },
	{ SDL_SCANCODE_WWW, DIK_WEBHOME },
	{ SDL_SCANCODE_MAIL, DIK_MAIL },
	{ SDL_SCANCODE_CALCULATOR, DIK_CALCULATOR },
	{ SDL_SCANCODE_COMPUTER, DIK_MYCOMPUTER },
	{ SDL_SCANCODE_AC_SEARCH, DIK_WEBSEARCH },
	{ SDL_SCANCODE_AC_BACK, DIK_WEBBACK },
	{ SDL_SCANCODE_AC_FORWARD, DIK_WEBFORWARD },
	{ SDL_SCANCODE_AC_STOP, DIK_WEBSTOP },
	{ SDL_SCANCODE_AC_REFRESH, DIK_WEBREFRESH },
	{ SDL_SCANCODE_AC_BOOKMARKS, DIK_WEBFAVORITES },
	{ SDL_SCANCODE_STOP, DIK_STOP },

	// Layout-specific keys. Text entry goes through SDL_TEXTINPUT (T1.6), so
	// these are only interesting to code that polls IsPressed().
	{ SDL_SCANCODE_INTERNATIONAL3, DIK_YEN },
	{ SDL_SCANCODE_LANG1, DIK_KANA },
	{ SDL_SCANCODE_LANG2, DIK_KANJI },
};

using DikTable = std::array<unsigned char, SDL_NUM_SCANCODES>;

const DikTable& GetDikTable()
{
	static const DikTable table = []() {
		DikTable result{};
		for (const ScancodeMapping& mapping : kScancodeToDik)
			result[mapping.scancode] = static_cast<unsigned char>(mapping.dik);
		return result;
	}();
	return table;
}

} // namespace

bool					CInputKeyboard::ms_bPressedKey[256];
char					CInputKeyboard::ms_diks[256];

CInputKeyboard::CInputKeyboard()
{
	ResetKeyboard();
}

CInputKeyboard::~CInputKeyboard()
{
}

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

void CInputKeyboard::UpdateKeyboardFromState(const unsigned char* pKeyState)
{
	const DikTable& table = GetDikTable();

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
			KeyUp(i);
	}
}

void CInputKeyboard::UpdateKeyboard()
{
	// SDL resets the keyboard state when the window loses focus; a missing
	// focus window therefore means "all released". DirectInput needed an
	// explicit Unacquire/Acquire round trip to get the same effect.
	if (SDL_GetKeyboardFocus() == NULL)
	{
		UpdateKeyboardFromState(NULL);
		return;
	}

	UpdateKeyboardFromState(SDL_GetKeyboardState(NULL));
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
