#include <Neoeter/Render/StdAfx.h>
#include <Neoeter/Render/MSWindow.h>
#include <Neoeter/Render/SdlPlatform.h>

#include <SDL2/SDL.h>
#include <Platform/GLCompat.h>

// Windows GL headers only ship the 1.1 subset; the constant comes from
// glext.h, which the loader work (T1.4) adds.
#ifndef GL_LOWER_LEFT
#define GL_LOWER_LEFT 0x8CA1
#endif

SDL_Window* CMSWindow::ms_pSDLWindow = NULL;
SDL_Renderer* CMSWindow::ms_pSDLRenderer = NULL;
SDL_GLContext CMSWindow::ms_pGLContext = NULL;

static void* SDLGetProcAddress(const char* name)
{
	return (void*) SDL_GL_GetProcAddress(name);
}

HINSTANCE CMSWindow::ms_hInstance = NULL;

LRESULT CMSWindow::WindowProcedure(HWND hWnd, UINT uiMsg, WPARAM wParam, LPARAM lParam)
{
	// The window is SDL-owned on every target (T1.2); the WM_* messages are
	// synthesised by CMSApplication from SDL events, so there is no Win32
	// window procedure to chain to.
	(void) hWnd;
	(void) lParam;

	switch (uiMsg)
	{
		case WM_SIZE:
			OnSize(wParam, 0);
			break;

		case WM_ACTIVATEAPP:
			m_isActive = (wParam == WA_ACTIVE) || (wParam == WA_CLICKACTIVE);
			break;
	}

	return 0;
}

void CMSWindow::OnSize(WPARAM wParam, LPARAM /*lParam*/)
{
	if (wParam == SIZE_MINIMIZED) 
	{
		m_isActive = false;        
		m_isVisible = false;
	}
	else
	{
		m_isActive = true;
		m_isVisible = true;
	}
}

void CMSWindow::Destroy()
{
	if (ms_pGLContext)
	{
		SDL_GL_DeleteContext(ms_pGLContext);
		ms_pGLContext = NULL;
	}
	if (ms_pSDLRenderer)
	{
		SDL_DestroyRenderer(ms_pSDLRenderer);
		ms_pSDLRenderer = NULL;
	}
	if (ms_pSDLWindow)
	{
		SDL_DestroyWindow(ms_pSDLWindow);
		ms_pSDLWindow = NULL;
	}

	m_hWnd = NULL;
	m_isVisible = false;
}

bool CMSWindow::Create(const char* c_szName, int /*brush*/, DWORD /*cs*/, DWORD /*ws*/, HICON /*hIcon*/, int /*iCursorResource*/, int width, int height)
{
	Destroy();

	if (SDL_WasInit(SDL_INIT_VIDEO) == 0)
	{
		if (SDL_Init(SDL_INIT_VIDEO | SDL_INIT_EVENTS) != 0)
		{
			TraceError("CMSWindow::Create: SDL_Init failed: %s", SDL_GetError());
			return false;
		}
	}

	if (width <= 0 || height <= 0)
	{
		Platform_GetWindowResolution(&width, &height);
		if (width <= 0) width = 1024;
		if (height <= 0) height = 768;
	}

	Uint32 windowFlags = SDL_WINDOW_SHOWN;

#if defined(OPENMT2_RENDERER_GL)
	SDL_GL_SetAttribute(SDL_GL_CONTEXT_MAJOR_VERSION, 2);
	SDL_GL_SetAttribute(SDL_GL_CONTEXT_MINOR_VERSION, 1);
	SDL_GL_SetAttribute(SDL_GL_DOUBLEBUFFER, 1);
	SDL_GL_SetAttribute(SDL_GL_DEPTH_SIZE, 24);
	windowFlags |= SDL_WINDOW_OPENGL;
#endif

	// Do NOT specify SDL_WINDOW_RESIZABLE; set min/max size equal to target dimensions.
	// Tiling WMs (i3, Sway, Hyprland, bspwm) automatically treat fixed-size windows as floating.
	ms_pSDLWindow = SDL_CreateWindow(
		(c_szName && c_szName[0]) ? c_szName : "Metin2",
		SDL_WINDOWPOS_CENTERED,
		SDL_WINDOWPOS_CENTERED,
		width,
		height,
		windowFlags
	);

	if (!ms_pSDLWindow)
	{
		TraceError("CMSWindow::Create: SDL_CreateWindow failed: %s", SDL_GetError());
		return false;
	}

	SDL_SetWindowResizable(ms_pSDLWindow, SDL_FALSE);
	SDL_SetWindowMinimumSize(ms_pSDLWindow, width, height);
	SDL_SetWindowMaximumSize(ms_pSDLWindow, width, height);
	SDL_SetWindowSize(ms_pSDLWindow, width, height);
	SDL_SetWindowPosition(ms_pSDLWindow, SDL_WINDOWPOS_CENTERED, SDL_WINDOWPOS_CENTERED);

#if defined(OPENMT2_RENDERER_GL)
	ms_pGLContext = SDL_GL_CreateContext(ms_pSDLWindow);
	if (!ms_pGLContext)
	{
		TraceError("[SDL2] SDL_GL_CreateContext failed: %s\n", SDL_GetError());
	}
	else
	{
		SDL_GL_MakeCurrent(ms_pSDLWindow, ms_pGLContext);
		SDL_GL_SetSwapInterval(1);

		if (!Platform_LoadGLFunctions(SDLGetProcAddress))
		{
			TraceError("[SDL2] Platform_LoadGLFunctions failed: missing GL entry points\n");
		}

		typedef void (*PFNGLCLIPCONTROLPROC)(GLenum origin, GLenum depth);
		PFNGLCLIPCONTROLPROC pfnClipControl = (PFNGLCLIPCONTROLPROC)SDL_GL_GetProcAddress("glClipControl");
		if (pfnClipControl)
		{
			// Direct3D 9 depth range [0, 1] natively handled by OpenGL clipping
			pfnClipControl(GL_LOWER_LEFT, 0x935F /* GL_ZERO_TO_ONE */);
			Tracef("[SDL2] glClipControl(GL_LOWER_LEFT, GL_ZERO_TO_ONE) enabled\n");
		}

		glClearColor(0.094f, 0.11f, 0.14f, 1.0f);
		glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);
		SDL_GL_SwapWindow(ms_pSDLWindow);
		Tracef("[SDL2] Initialized OpenGL context for window %p\n", ms_pSDLWindow);
	}
#endif

	m_hWnd = (HWND)ms_pSDLWindow;
	m_isActive = true;
	m_isVisible = true;
	Platform_SetWindowResolution(width, height);
	Platform_SetWarpMouseCallback([](int x, int y) {
		if (ms_pSDLWindow) {
			SDL_WarpMouseInWindow(ms_pSDLWindow, x, y);
		}
	});
	SetRect(&m_rect, 0, 0, width, height);

	Tracef("[SDL2] Created native game window (floating): %p ('%s', %dx%d, video driver: %s)\n",
		ms_pSDLWindow, c_szName, width, height, SDL_GetCurrentVideoDriver());
	return true;
}

void CMSWindow::SetVisibleMode(bool isVisible)
{
	m_isVisible = isVisible;

	if (ms_pSDLWindow)
	{
		if (isVisible)
			SDL_ShowWindow(ms_pSDLWindow);
		else
			SDL_HideWindow(ms_pSDLWindow);
	}
}

void CMSWindow::Show()
{
	m_isVisible = true;
	if (ms_pSDLWindow)
		SDL_ShowWindow(ms_pSDLWindow);
}

void CMSWindow::Hide()
{
	m_isVisible = false;
	if (ms_pSDLWindow)
		SDL_HideWindow(ms_pSDLWindow);
}

bool CMSWindow::IsVisible()
{
	return m_isVisible;
}

bool CMSWindow::IsActive()
{
	return m_isActive;
}

HINSTANCE CMSWindow::GetInstance()
{
	return ms_hInstance;
}

HWND CMSWindow::GetWindowHandle()
{
	return m_hWnd;
}

int	CMSWindow::GetScreenWidth()
{
	SDL_DisplayMode dm;
	if (SDL_GetCurrentDisplayMode(0, &dm) == 0)
		return dm.w;
	return 0;
}

int	CMSWindow::GetScreenHeight()
{
	SDL_DisplayMode dm;
	if (SDL_GetCurrentDisplayMode(0, &dm) == 0)
		return dm.h;
	return 0;
}

void CMSWindow::GetWindowRect(RECT* prc)
{
	if (ms_pSDLWindow && prc)
	{
		int x = 0, y = 0, w = 0, h = 0;
		SDL_GetWindowPosition(ms_pSDLWindow, &x, &y);
		SDL_GetWindowSize(ms_pSDLWindow, &w, &h);
		prc->left = x;
		prc->top = y;
		prc->right = x + w;
		prc->bottom = y + h;
	}
}

void CMSWindow::GetClientRect(RECT* prc)
{
	if (ms_pSDLWindow && prc)
	{
		int w = 0, h = 0;
		SDL_GetWindowSize(ms_pSDLWindow, &w, &h);
		prc->left = 0;
		prc->top = 0;
		prc->right = w;
		prc->bottom = h;
	}
}

void CMSWindow::GetMousePosition(POINT* ppt)
{
	if (ppt)
	{
		int x = 0, y = 0;
		SDL_GetMouseState(&x, &y);
		ppt->x = x;
		ppt->y = y;
	}
}

void CMSWindow::SetPosition(int x, int y)
{
	if (ms_pSDLWindow)
		SDL_SetWindowPosition(ms_pSDLWindow, x, y);
}

void CMSWindow::SetCenterPosition()
{
	RECT rc;

	GetClientRect(&rc);

	int windowWidth = rc.right - rc.left;
	int windowHeight = rc.bottom - rc.top;

	SetPosition((GetScreenWidth()-windowWidth)/2, (GetScreenHeight()-windowHeight)/2);
}

void CMSWindow::AdjustSize(int width, int height)
{
	if (ms_pSDLWindow)
	{
		SDL_SetWindowResizable(ms_pSDLWindow, SDL_FALSE);
		SDL_SetWindowMinimumSize(ms_pSDLWindow, width, height);
		SDL_SetWindowMaximumSize(ms_pSDLWindow, width, height);
		SDL_SetWindowSize(ms_pSDLWindow, width, height);
		SDL_SetWindowPosition(ms_pSDLWindow, SDL_WINDOWPOS_CENTERED, SDL_WINDOWPOS_CENTERED);
	}
	Platform_SetWindowResolution(width, height);
	SetRect(&m_rect, 0, 0, width, height);
}

void CMSWindow::SetText(const char* c_szText)
{
	if (ms_pSDLWindow && c_szText)
		SDL_SetWindowTitle(ms_pSDLWindow, c_szText);
}

void CMSWindow::SetSize(int width, int height)
{
	if (ms_pSDLWindow)
	{
		SDL_SetWindowResizable(ms_pSDLWindow, SDL_FALSE);
		SDL_SetWindowMinimumSize(ms_pSDLWindow, width, height);
		SDL_SetWindowMaximumSize(ms_pSDLWindow, width, height);
		SDL_SetWindowSize(ms_pSDLWindow, width, height);
		SDL_SetWindowPosition(ms_pSDLWindow, SDL_WINDOWPOS_CENTERED, SDL_WINDOWPOS_CENTERED);
	}
	Platform_SetWindowResolution(width, height);
	SetRect(&m_rect, 0, 0, width, height);
}

CMSWindow::CMSWindow()
{
	m_hWnd = NULL;
	m_isVisible = false;
	m_isActive = true;
}

CMSWindow::~CMSWindow()
{
}
