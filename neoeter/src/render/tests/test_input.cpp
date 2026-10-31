// T1.3 — SDL2 input on every target.
//
// The game speaks DirectInput scan codes (DIK_*) everywhere: the Python
// bindings expose them, the player input handler switches on them, and the
// keyboard widget state is indexed by them. SDL2 reports USB HID scancodes, so
// the translation SDL scancode -> DIK is the contract this task delivers.
//
// These tests are headless: they feed a synthetic SDL keyboard-state array into
// CInputKeyboard, so no window, video driver or focus is required.

#include <support/TestAssert.h>

#include <Neoeter/Render/Input.h>

#include <Platform/Platform.h>

#include <SDL2/SDL.h>

#include <cstdio>
#include <cstring>
#include <vector>

namespace
{

constexpr int kScancodeCount = CInputKeyboard::SCANCODE_COUNT;

class RecordingKeyboard : public CInputKeyboard
{
public:
	void Clear()
	{
		downs.clear();
		ups.clear();
		ResetKeyboard();
	}

	std::vector<int> downs;
	std::vector<int> ups;

protected:
	void OnKeyDown(int iIndex) override { downs.push_back(iIndex); }
	void OnKeyUp(int iIndex) override { ups.push_back(iIndex); }
};

struct KeyState
{
	KeyState() { std::memset(state, 0, sizeof(state)); }

	void press(int scancode) { state[scancode] = 1; }
	void release(int scancode) { state[scancode] = 0; }

	unsigned char state[kScancodeCount];
};

void expect_mapping(int scancode, int expected, const char* name)
{
	const int actual = CInputKeyboard::ScancodeToDik(scancode);
	if (actual != expected)
	{
		std::fprintf(stderr, "  [mapping] %s: scancode %d -> %d, expected %d\n",
					 name, scancode, actual, expected);
	}
	TEST_ASSERT_MSG(actual == expected, name);
}

void test_table_consistency()
{
	TEST_SECTION("SDL scancode -> DIK table");

	bool seen[256] = {false};
	int mapped = 0;

	for (int scancode = 0; scancode < kScancodeCount; ++scancode)
	{
		const int dik = CInputKeyboard::ScancodeToDik(scancode);
		if (dik < 0)
			continue;

		TEST_ASSERT(dik > 0 && dik < 256);
		TEST_ASSERT_MSG(!seen[dik], "two SDL scancodes map to the same DIK code");
		seen[dik] = true;
		++mapped;
	}

	// The classic 104-key set alone is ~100 DIKs; anything less means a group
	// of keys silently stopped reaching the game.
	TEST_ASSERT_MSG(mapped >= 100, "the scancode table lost coverage");

	// Out-of-range and unassigned inputs must not index out of bounds.
	TEST_ASSERT(CInputKeyboard::ScancodeToDik(-1) == -1);
	TEST_ASSERT(CInputKeyboard::ScancodeToDik(kScancodeCount) == -1);
	TEST_ASSERT(CInputKeyboard::ScancodeToDik(SDL_SCANCODE_UNKNOWN) == -1);
	TEST_ASSERT(CInputKeyboard::ScancodeToDik(130) == -1); // unassigned in SDL2

	TEST_PASS("unique, in-range mapping with a full key set");
}

void test_letters_and_digits()
{
	TEST_SECTION("letters and digits");

	static const int kLetterDik[26] = {
		DIK_A, DIK_B, DIK_C, DIK_D, DIK_E, DIK_F, DIK_G, DIK_H, DIK_I,
		DIK_J, DIK_K, DIK_L, DIK_M, DIK_N, DIK_O, DIK_P, DIK_Q, DIK_R,
		DIK_S, DIK_T, DIK_U, DIK_V, DIK_W, DIK_X, DIK_Y, DIK_Z,
	};
	static const char* const kLetterName[26] = {
		"A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M",
		"N", "O", "P", "Q", "R", "S", "T", "U", "V", "W", "X", "Y", "Z",
	};

	// SDL orders A..Z alphabetically; DIK orders A..L then M..Z (M jumps to
	// 0x32 and N..Z run backwards from there), so this is not a simple offset.
	for (int i = 0; i < 26; ++i)
		expect_mapping(SDL_SCANCODE_A + i, kLetterDik[i], kLetterName[i]);

	static const int kDigitDik[10] = {
		DIK_1, DIK_2, DIK_3, DIK_4, DIK_5, DIK_6, DIK_7, DIK_8, DIK_9, DIK_0,
	};
	static const char* const kDigitName[10] = {
		"1", "2", "3", "4", "5", "6", "7", "8", "9", "0",
	};

	// DIK_1..DIK_9 are 0x02..0x0A and DIK_0 is 0x0B, matching the number row.
	for (int i = 0; i < 10; ++i)
		expect_mapping(SDL_SCANCODE_1 + i, kDigitDik[i], kDigitName[i]);

	TEST_PASS("26 letters and 10 digits");
}

void test_named_keys()
{
	TEST_SECTION("named keys");

	expect_mapping(SDL_SCANCODE_ESCAPE, DIK_ESCAPE, "escape");
	expect_mapping(SDL_SCANCODE_MINUS, DIK_MINUS, "minus");
	expect_mapping(SDL_SCANCODE_EQUALS, DIK_EQUALS, "equals");
	expect_mapping(SDL_SCANCODE_BACKSPACE, DIK_BACK, "backspace");
	expect_mapping(SDL_SCANCODE_TAB, DIK_TAB, "tab");
	expect_mapping(SDL_SCANCODE_LEFTBRACKET, DIK_LBRACKET, "left bracket");
	expect_mapping(SDL_SCANCODE_RIGHTBRACKET, DIK_RBRACKET, "right bracket");
	expect_mapping(SDL_SCANCODE_RETURN, DIK_RETURN, "return");
	expect_mapping(SDL_SCANCODE_LCTRL, DIK_LCONTROL, "left control");
	expect_mapping(SDL_SCANCODE_RCTRL, DIK_RCONTROL, "right control");
	expect_mapping(SDL_SCANCODE_SEMICOLON, DIK_SEMICOLON, "semicolon");
	expect_mapping(SDL_SCANCODE_APOSTROPHE, DIK_APOSTROPHE, "apostrophe");
	expect_mapping(SDL_SCANCODE_GRAVE, DIK_GRAVE, "grave");
	expect_mapping(SDL_SCANCODE_LSHIFT, DIK_LSHIFT, "left shift");
	expect_mapping(SDL_SCANCODE_RSHIFT, DIK_RSHIFT, "right shift");
	expect_mapping(SDL_SCANCODE_BACKSLASH, DIK_BACKSLASH, "backslash");
	expect_mapping(SDL_SCANCODE_COMMA, DIK_COMMA, "comma");
	expect_mapping(SDL_SCANCODE_PERIOD, DIK_PERIOD, "period");
	expect_mapping(SDL_SCANCODE_SLASH, DIK_SLASH, "slash");
	expect_mapping(SDL_SCANCODE_LALT, DIK_LMENU, "left alt");
	expect_mapping(SDL_SCANCODE_RALT, DIK_RMENU, "right alt");
	expect_mapping(SDL_SCANCODE_SPACE, DIK_SPACE, "space");
	expect_mapping(SDL_SCANCODE_CAPSLOCK, DIK_CAPITAL, "caps lock");
	expect_mapping(SDL_SCANCODE_NUMLOCKCLEAR, DIK_NUMLOCK, "num lock");
	expect_mapping(SDL_SCANCODE_SCROLLLOCK, DIK_SCROLL, "scroll lock");
	expect_mapping(SDL_SCANCODE_PRINTSCREEN, DIK_SYSRQ, "print screen");
	expect_mapping(SDL_SCANCODE_PAUSE, DIK_PAUSE, "pause");
	expect_mapping(SDL_SCANCODE_LGUI, DIK_LWIN, "left win");
	expect_mapping(SDL_SCANCODE_RGUI, DIK_RWIN, "right win");
	expect_mapping(SDL_SCANCODE_APPLICATION, DIK_APPS, "application");
	expect_mapping(SDL_SCANCODE_NONUSBACKSLASH, DIK_OEM_102, "iso key 102");

	// SDL F1..F12 are contiguous; DIK F1..F10 are 0x3B..0x44 but F11/F12 jump
	// to 0x57/0x58.
	for (int i = 0; i < 10; ++i)
	{
		char name[8];
		std::snprintf(name, sizeof(name), "F%d", i + 1);
		expect_mapping(SDL_SCANCODE_F1 + i, DIK_F1 + i, name);
	}
	expect_mapping(SDL_SCANCODE_F11, DIK_F11, "F11");
	expect_mapping(SDL_SCANCODE_F12, DIK_F12, "F12");

	expect_mapping(SDL_SCANCODE_F13, DIK_F13, "F13");
	expect_mapping(SDL_SCANCODE_F14, DIK_F14, "F14");
	expect_mapping(SDL_SCANCODE_F15, DIK_F15, "F15");

	TEST_PASS("modifiers, navigation and function keys");
}

void test_navigation_and_keypad()
{
	TEST_SECTION("navigation and keypad");

	expect_mapping(SDL_SCANCODE_INSERT, DIK_INSERT, "insert");
	expect_mapping(SDL_SCANCODE_DELETE, DIK_DELETE, "delete");
	expect_mapping(SDL_SCANCODE_HOME, DIK_HOME, "home");
	expect_mapping(SDL_SCANCODE_END, DIK_END, "end");
	expect_mapping(SDL_SCANCODE_PAGEUP, DIK_PRIOR, "page up");
	expect_mapping(SDL_SCANCODE_PAGEDOWN, DIK_NEXT, "page down");
	expect_mapping(SDL_SCANCODE_UP, DIK_UP, "up");
	expect_mapping(SDL_SCANCODE_DOWN, DIK_DOWN, "down");
	expect_mapping(SDL_SCANCODE_LEFT, DIK_LEFT, "left");
	expect_mapping(SDL_SCANCODE_RIGHT, DIK_RIGHT, "right");

	expect_mapping(SDL_SCANCODE_KP_DIVIDE, DIK_DIVIDE, "keypad /");
	expect_mapping(SDL_SCANCODE_KP_MULTIPLY, DIK_MULTIPLY, "keypad *");
	expect_mapping(SDL_SCANCODE_KP_MINUS, DIK_SUBTRACT, "keypad -");
	expect_mapping(SDL_SCANCODE_KP_PLUS, DIK_ADD, "keypad +");
	expect_mapping(SDL_SCANCODE_KP_ENTER, DIK_NUMPADENTER, "keypad enter");
	expect_mapping(SDL_SCANCODE_KP_PERIOD, DIK_DECIMAL, "keypad .");
	expect_mapping(SDL_SCANCODE_KP_COMMA, DIK_NUMPADCOMMA, "keypad ,");
	expect_mapping(SDL_SCANCODE_KP_EQUALS, DIK_NUMPADEQUALS, "keypad =");

	// SDL numbers the keypad 1..9 then 0; DIK follows the physical grid
	// (7,8,9 / 4,5,6 / 1,2,3), so NUMPAD1..3 and 4..9 are two contiguous runs.
	static const int kNumpadDik[9] = {
		DIK_NUMPAD1, DIK_NUMPAD2, DIK_NUMPAD3,
		DIK_NUMPAD4, DIK_NUMPAD5, DIK_NUMPAD6,
		DIK_NUMPAD7, DIK_NUMPAD8, DIK_NUMPAD9,
	};
	for (int i = 0; i < 9; ++i)
	{
		char name[8];
		std::snprintf(name, sizeof(name), "pad %d", i + 1);
		expect_mapping(SDL_SCANCODE_KP_1 + i, kNumpadDik[i], name);
	}
	expect_mapping(SDL_SCANCODE_KP_0, DIK_NUMPAD0, "pad 0");

	TEST_PASS("navigation cluster and keypad");
}

void test_key_events()
{
	TEST_SECTION("key down / up events");

	RecordingKeyboard keyboard;
	KeyState keys;

	keys.press(SDL_SCANCODE_A);
	keys.press(SDL_SCANCODE_LSHIFT);
	keyboard.UpdateKeyboardFromState(keys.state);

	TEST_ASSERT(keyboard.downs.size() == 2);
	TEST_ASSERT(keyboard.downs[0] == DIK_A);
	TEST_ASSERT(keyboard.downs[1] == DIK_LSHIFT);
	TEST_ASSERT(keyboard.ups.empty());
	TEST_ASSERT(keyboard.IsPressed(DIK_A));
	TEST_ASSERT(keyboard.IsPressed(DIK_LSHIFT));

	// Polling the same state twice must not repeat the key-down event; the
	// game uses OnKeyDown to toggle windows and cast skills.
	keyboard.UpdateKeyboardFromState(keys.state);
	TEST_ASSERT(keyboard.downs.size() == 2);
	TEST_ASSERT(keyboard.ups.empty());

	keys.release(SDL_SCANCODE_A);
	keyboard.UpdateKeyboardFromState(keys.state);
	TEST_ASSERT(keyboard.ups.size() == 1);
	TEST_ASSERT(keyboard.ups[0] == DIK_A);
	TEST_ASSERT(!keyboard.IsPressed(DIK_A));
	TEST_ASSERT(keyboard.IsPressed(DIK_LSHIFT));

	TEST_PASS("edge detection, no auto-repeat, per-key release");
}

void test_focus_loss_releases_keys()
{
	TEST_SECTION("focus loss");

	RecordingKeyboard keyboard;
	KeyState keys;

	static const int kHeld[] = {
		SDL_SCANCODE_W, SDL_SCANCODE_A, SDL_SCANCODE_S, SDL_SCANCODE_D,
		SDL_SCANCODE_SPACE,
	};
	for (int scancode : kHeld)
		keys.press(scancode);

	keyboard.UpdateKeyboardFromState(keys.state);
	TEST_ASSERT(keyboard.downs.size() == 5);

	// A null state means "no keyboard focus": every held key is released so it
	// cannot stay stuck down across Alt+Tab.
	keyboard.downs.clear();
	keyboard.UpdateKeyboardFromState(NULL);

	TEST_ASSERT(keyboard.ups.size() == 5);
	TEST_ASSERT(!keyboard.IsPressed(DIK_W));
	TEST_ASSERT(!keyboard.IsPressed(DIK_A));
	TEST_ASSERT(!keyboard.IsPressed(DIK_S));
	TEST_ASSERT(!keyboard.IsPressed(DIK_D));
	TEST_ASSERT(!keyboard.IsPressed(DIK_SPACE));

	// Repeated focus-loss polls are no-ops.
	keyboard.ups.clear();
	keyboard.UpdateKeyboardFromState(NULL);
	TEST_ASSERT(keyboard.ups.empty());

	TEST_PASS("all held keys released exactly once");
}

} // namespace

int main()
{
	test_table_consistency();
	test_letters_and_digits();
	test_named_keys();
	test_navigation_and_keypad();
	test_key_events();
	test_focus_loss_releases_keys();

	std::printf("Input SDL mapping tests passed.\n");
	return 0;
}
