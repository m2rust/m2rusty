#pragma once

// T1.3: the keyboard is SDL2 on every target. This header only exposes the
// game-facing contract (DirectInput scan codes + edge events); the SDL
// translation lives in Input.cpp.

#include <Platform/Platform.h>

class CInputKeyboard
{
	public:
		CInputKeyboard();
		virtual ~CInputKeyboard();

		// Kept as HWND for the historical call sites (CPythonApplication); SDL2
		// owns the device, so the handle is not used.
		bool			InitializeKeyboard(HWND hWnd);
		void			UpdateKeyboard();
		void			ResetKeyboard();

		bool			IsPressed(int iIndex);
		void			KeyDown(int iIndex);
		void			KeyUp(int iIndex);

		// Number of entries in the SDL keyboard-state array (SDL_NUM_SCANCODES).
		static constexpr int	SCANCODE_COUNT = 512;

		// SDL scancode -> DirectInput scan code (DIK_*), the game's input ABI.
		// Returns -1 when the key has no DIK equivalent.
		static int		ScancodeToDik(int iScancode);

		// Diffs a SDL keyboard-state array (one byte per scancode, non-zero =
		// held) against the previous frame and fires OnKeyDown/OnKeyUp.
		// A null pointer means "no keyboard focus" and releases every held key.
		void			UpdateKeyboardFromState(const unsigned char* pKeyState);

	protected:
		virtual void	OnKeyDown(int iIndex) = 0;
		virtual void	OnKeyUp(int iIndex) = 0;

	protected:
		static bool					ms_bPressedKey[256];
		static char					ms_diks[256];
};
