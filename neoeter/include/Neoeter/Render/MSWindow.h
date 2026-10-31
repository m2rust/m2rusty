#pragma once

#include <Neoeter/Platform/Base/Stl.h>

#include <SDL2/SDL.h>

class CMSWindow
{
	public:
		CMSWindow();
		
		virtual ~CMSWindow();
		
		void Destroy();
		bool Create(const char* c_szName, int brush=BLACK_BRUSH, DWORD cs=0, DWORD ws=WS_OVERLAPPEDWINDOW, HICON hIcon=NULL, int iCursorResource=32512, int width=0, int height=0);
		
		void Show();
		void Hide();

		void SetVisibleMode(bool isVisible);

		void SetPosition(int x, int y);
		void SetCenterPosition();

		void SetText(const char* c_szText);

		void AdjustSize(int width, int height);
		void SetSize(int width, int height);

		bool IsVisible();
		bool IsActive();

		void GetMousePosition(POINT* ppt);
		void GetClientRect(RECT* prc);
		void GetWindowRect(RECT* prc);

		int	GetScreenWidth();
		int	GetScreenHeight();

		HWND GetWindowHandle();
		HINSTANCE GetInstance();

		static SDL_Window* ms_pSDLWindow;
		static SDL_Renderer* ms_pSDLRenderer;
		static SDL_GLContext ms_pGLContext;
		static SDL_Window* GetSDLWindow() { return ms_pSDLWindow; }
		static SDL_Renderer* GetSDLRenderer() { return ms_pSDLRenderer; }
		static SDL_GLContext GetGLContext() { return ms_pGLContext; }

		virtual LRESULT	WindowProcedure(HWND hWnd, UINT uiMsg, WPARAM wParam, LPARAM lParam);
		virtual void	OnSize(WPARAM wParam, LPARAM lParam);

	protected:
		HWND m_hWnd;
		RECT m_rect;
		bool m_isActive;
		bool m_isVisible;

	protected:
		static HINSTANCE ms_hInstance;
};
