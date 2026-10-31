#include <Neoeter/Render/StdAfx.h>
#include <Neoeter/Render/MSApplication.h>
#include <Neoeter/Render/SdlPlatform.h>
#include <Neoeter/Render/IME.h>

#include <SDL2/SDL.h>

#ifndef MAKELPARAM
#define MAKELPARAM(l, h) ((LPARAM)(((WORD)(((DWORD_PTR)(l)) & 0xffff)) | ((DWORD)((WORD)(((DWORD_PTR)(h)) & 0xffff))) << 16))
#endif
#ifndef MAKEWPARAM
#define MAKEWPARAM(l, h) ((WPARAM)(((WORD)(((DWORD_PTR)(l)) & 0xffff)) | ((DWORD)((WORD)(((DWORD_PTR)(h)) & 0xffff))) << 16))
#endif

static void SDL_QuitCallback(int /*exitCode*/)
{
	SDL_Event quitEvent;
	memset(&quitEvent, 0, sizeof(quitEvent));
	quitEvent.type = SDL_QUIT;
	quitEvent.quit.type = SDL_QUIT;
	quitEvent.quit.timestamp = SDL_GetTicks();
	SDL_PushEvent(&quitEvent);
}

static WPARAM SDLKeycodeToVK(SDL_Keycode key, SDL_Scancode scancode = SDL_SCANCODE_UNKNOWN)
{
	if (scancode == SDL_SCANCODE_DELETE || key == SDLK_DELETE)
		return VK_DELETE;
	if (scancode == SDL_SCANCODE_BACKSPACE || key == SDLK_BACKSPACE)
		return VK_BACK;
	if (scancode == SDL_SCANCODE_KP_PERIOD || key == SDLK_KP_PERIOD)
	{
		if (!(SDL_GetModState() & KMOD_NUM))
			return VK_DELETE;
		return VK_DECIMAL;
	}

	switch (key)
	{
		case SDLK_BACKSPACE: return VK_BACK;
		case SDLK_TAB: return VK_TAB;
		case SDLK_CLEAR: return VK_CLEAR;
		case SDLK_RETURN:
		case SDLK_KP_ENTER: return VK_RETURN;
		case SDLK_PAUSE: return VK_PAUSE;
		case SDLK_CAPSLOCK: return VK_CAPITAL;
		case SDLK_ESCAPE: return VK_ESCAPE;
		case SDLK_SPACE: return VK_SPACE;
		case SDLK_PAGEUP: return VK_PRIOR;
		case SDLK_PAGEDOWN: return VK_NEXT;
		case SDLK_END: return VK_END;
		case SDLK_HOME: return VK_HOME;
		case SDLK_LEFT: return VK_LEFT;
		case SDLK_UP: return VK_UP;
		case SDLK_RIGHT: return VK_RIGHT;
		case SDLK_DOWN: return VK_DOWN;
		case SDLK_PRINTSCREEN: return VK_SNAPSHOT;
		case SDLK_INSERT: return VK_INSERT;
		case SDLK_DELETE: return VK_DELETE;
		case SDLK_HELP: return VK_HELP;
		case SDLK_LSHIFT:
		case SDLK_RSHIFT: return VK_SHIFT;
		case SDLK_LCTRL:
		case SDLK_RCTRL: return VK_CONTROL;
		case SDLK_LALT:
		case SDLK_RALT: return VK_MENU;
		case SDLK_KP_0: return VK_NUMPAD0;
		case SDLK_KP_1: return VK_NUMPAD1;
		case SDLK_KP_2: return VK_NUMPAD2;
		case SDLK_KP_3: return VK_NUMPAD3;
		case SDLK_KP_4: return VK_NUMPAD4;
		case SDLK_KP_5: return VK_NUMPAD5;
		case SDLK_KP_6: return VK_NUMPAD6;
		case SDLK_KP_7: return VK_NUMPAD7;
		case SDLK_KP_8: return VK_NUMPAD8;
		case SDLK_KP_9: return VK_NUMPAD9;
		case SDLK_KP_MULTIPLY: return VK_MULTIPLY;
		case SDLK_KP_PLUS: return VK_ADD;
		case SDLK_KP_MINUS: return VK_SUBTRACT;
		case SDLK_KP_DECIMAL:
		case SDLK_KP_PERIOD: return VK_DECIMAL;
		case SDLK_KP_DIVIDE: return VK_DIVIDE;
		case SDLK_F1: return VK_F1;
		case SDLK_F2: return VK_F2;
		case SDLK_F3: return VK_F3;
		case SDLK_F4: return VK_F4;
		case SDLK_F5: return VK_F5;
		case SDLK_F6: return VK_F6;
		case SDLK_F7: return VK_F7;
		case SDLK_F8: return VK_F8;
		case SDLK_F9: return VK_F9;
		case SDLK_F10: return VK_F10;
		case SDLK_F11: return VK_F11;
		case SDLK_F12: return VK_F12;
		case SDLK_SCROLLLOCK: return VK_SCROLL;
		default:
			if (key >= SDLK_a && key <= SDLK_z)
				return 'A' + (key - SDLK_a);
			if (key >= SDLK_0 && key <= SDLK_9)
				return '0' + (key - SDLK_0);
			return (WPARAM)key;
	}
}

CMSApplication::CMSApplication()
{
	Platform_SetQuitCallback(SDL_QuitCallback);
}

CMSApplication::~CMSApplication()
{
//	for (TWindowClassSet::iterator i=ms_stWCSet.begin(); i!=ms_stWCSet.end(); ++i)
//		UnregisterClass(*i, ms_hInstance);
}

void CMSApplication::Initialize(HINSTANCE hInstance)
{
	ms_hInstance = hInstance;
}

void CMSApplication::MessageLoop()
{
	while (MessageProcess());
}

bool CMSApplication::IsMessage()
{
	if (Platform_IsQuitRequested())
		return true;
	SDL_PumpEvents();
	return (SDL_PeepEvents(NULL, 1, SDL_PEEKEVENT, SDL_FIRSTEVENT, SDL_LASTEVENT) > 0);
}

bool CMSApplication::MessageProcess()
{
	if (Platform_IsQuitRequested())
		return false;

	SDL_Event event;
	while (SDL_PollEvent(&event))
	{
		if (Platform_IsQuitRequested() || event.type == SDL_QUIT)
		{
			Tracef("[SDL2] Received SDL_QUIT event or quit requested. Exiting MessageProcess.\n");
			Platform_GetQuitRequested().store(true);
			return false;
		}

		if (event.type == SDL_WINDOWEVENT)
		{
			if (event.window.event == SDL_WINDOWEVENT_CLOSE)
			{
				Tracef("[SDL2] Received SDL_WINDOWEVENT_CLOSE event. Exiting MessageProcess.\n");
				Platform_GetQuitRequested().store(true);
				return false;
			}
			else if (event.window.event == SDL_WINDOWEVENT_MAXIMIZED)
			{
				if (ms_pSDLWindow)
				{
					SDL_RestoreWindow(ms_pSDLWindow);
					int targetW = 1024, targetH = 768;
					Platform_GetWindowResolution(&targetW, &targetH);
					if (targetW > 0 && targetH > 0)
						SDL_SetWindowSize(ms_pSDLWindow, targetW, targetH);
				}
			}
			else if (event.window.event == SDL_WINDOWEVENT_SIZE_CHANGED || event.window.event == SDL_WINDOWEVENT_RESIZED)
			{
				int targetW = 1024, targetH = 768;
				Platform_GetWindowResolution(&targetW, &targetH);
				if (targetW > 0 && targetH > 0 && (event.window.data1 != targetW || event.window.data2 != targetH))
				{
					if (ms_pSDLWindow)
					{
						SDL_SetWindowSize(ms_pSDLWindow, targetW, targetH);
					}
				}
				else
				{
					OnSize(SIZE_RESTORED, MAKELPARAM(event.window.data1, event.window.data2));
					WindowProcedure(m_hWnd, WM_SIZE, SIZE_RESTORED, MAKELPARAM(event.window.data1, event.window.data2));
				}
			}
			else if (event.window.event == SDL_WINDOWEVENT_FOCUS_GAINED)
			{
				m_isActive = true;
				WindowProcedure(m_hWnd, WM_ACTIVATEAPP, WA_ACTIVE, 0);
			}
			else if (event.window.event == SDL_WINDOWEVENT_FOCUS_LOST)
			{
				m_isActive = false;
				WindowProcedure(m_hWnd, WM_ACTIVATEAPP, WA_INACTIVE, 0);
			}
		}
		else if (event.type == SDL_TEXTEDITING)
		{
			// OS IME composition; SDL forwards start/length in characters.
			CIME::OnSDLTextEditing(event.edit.text, event.edit.start, event.edit.length);
		}
		else if (event.type == SDL_TEXTINPUT)
		{
			// Committed text from the OS IME.
			CIME::OnSDLTextInput(event.text.text);
		}
		else if (event.type == SDL_MOUSEMOTION)
		{
			Platform_SetCursorPos(event.motion.x, event.motion.y);
			WindowProcedure(m_hWnd, WM_MOUSEMOVE, 0, MAKELPARAM(event.motion.x, event.motion.y));
		}
		else if (event.type == SDL_MOUSEBUTTONDOWN)
		{
			Platform_SetCursorPos(event.button.x, event.button.y);
			UINT uMsg = WM_LBUTTONDOWN;
			if (event.button.button == SDL_BUTTON_RIGHT) uMsg = WM_RBUTTONDOWN;
			else if (event.button.button == SDL_BUTTON_MIDDLE) uMsg = WM_MBUTTONDOWN;
			WindowProcedure(m_hWnd, uMsg, 0, MAKELPARAM(event.button.x, event.button.y));
		}
		else if (event.type == SDL_MOUSEBUTTONUP)
		{
			Platform_SetCursorPos(event.button.x, event.button.y);
			UINT uMsg = WM_LBUTTONUP;
			if (event.button.button == SDL_BUTTON_RIGHT) uMsg = WM_RBUTTONUP;
			else if (event.button.button == SDL_BUTTON_MIDDLE) uMsg = WM_MBUTTONUP;
			WindowProcedure(m_hWnd, uMsg, 0, MAKELPARAM(event.button.x, event.button.y));
		}
		else if (event.type == SDL_MOUSEWHEEL)
		{
			short zDelta = (short)(event.wheel.y * 120);
			WindowProcedure(m_hWnd, WM_MOUSEWHEEL, MAKEWPARAM(0, zDelta), 0);
		}
		else if (event.type == SDL_KEYDOWN)
		{
			WPARAM vKey = SDLKeycodeToVK(event.key.keysym.sym, event.key.keysym.scancode);
			WindowProcedure(m_hWnd, WM_KEYDOWN, vKey, 0);

			if (event.key.keysym.scancode == SDL_SCANCODE_BACKSPACE || event.key.keysym.sym == SDLK_BACKSPACE)
			{
				WindowProcedure(m_hWnd, WM_CHAR, 8, 0);
			}
			else if (event.key.keysym.scancode == SDL_SCANCODE_DELETE || event.key.keysym.sym == SDLK_DELETE)
			{
				WindowProcedure(m_hWnd, WM_CHAR, 127, 0);
			}
			// Windows also sends WM_CHAR for these, and the IME relies on it:
			// Enter opens/sends chat and confirms dialogs, Tab moves focus.
			// SDL_TEXTINPUT never carries them.
			else if (event.key.keysym.sym == SDLK_RETURN || event.key.keysym.sym == SDLK_KP_ENTER)
			{
				WindowProcedure(m_hWnd, WM_CHAR, VK_RETURN, 0);
			}
			else if (event.key.keysym.sym == SDLK_TAB)
			{
				WindowProcedure(m_hWnd, WM_CHAR, VK_TAB, 0);
			}
			else if (event.key.keysym.sym == SDLK_ESCAPE)
			{
				WindowProcedure(m_hWnd, WM_CHAR, VK_ESCAPE, 0);
			}
		}
		else if (event.type == SDL_KEYUP)
		{
			WPARAM vKey = SDLKeycodeToVK(event.key.keysym.sym, event.key.keysym.scancode);
			WindowProcedure(m_hWnd, WM_KEYUP, vKey, 0);
		}

		if (Platform_IsQuitRequested())
		{
			Tracef("[SDL2] Quit requested during event processing. Exiting MessageProcess.\n");
			return false;
		}
	}

	if (Platform_IsQuitRequested())
		return false;

	int mx = 0, my = 0;
	SDL_GetMouseState(&mx, &my);
	Platform_SetCursorPos(mx, my);

	return true;
}

LRESULT CMSApplication::WindowProcedure(HWND hWnd, UINT uiMsg, WPARAM wParam, LPARAM lParam)
{
	switch (uiMsg)
	{
		case WM_CLOSE:
			Platform_GetQuitRequested().store(true);
			break;
	}

	return CMSWindow::WindowProcedure(hWnd, uiMsg, wParam, lParam);
}