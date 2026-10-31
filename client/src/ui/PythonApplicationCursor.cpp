#include <Client/UI/StdAfx.h>
#include <Client/UI/PythonApplication.h>
#include <Client/UI/resource.h>
#include <Client/UI/SdlCursor.h>

bool CPythonApplication::CreateCursors()
{
	m_bCursorVisible = TRUE;
	m_bLiarCursorOn = false;

	// The Win32 cursor resources are not part of the source tree, so the SDL
	// window/input path uses SDL system cursors on every target (T3.2).
	static const SDL_SystemCursor s_systemCursors[CURSOR_COUNT] =
	{
		SDL_SYSTEM_CURSOR_ARROW,       // NORMAL
		SDL_SYSTEM_CURSOR_ARROW,       // ATTACK
		SDL_SYSTEM_CURSOR_CROSSHAIR,   // TARGET
		SDL_SYSTEM_CURSOR_HAND,        // TALK
		SDL_SYSTEM_CURSOR_NO,          // CANT_GO
		SDL_SYSTEM_CURSOR_CROSSHAIR,   // PICK

		SDL_SYSTEM_CURSOR_HAND,        // DOOR
		SDL_SYSTEM_CURSOR_ARROW,       // CHAIR
		SDL_SYSTEM_CURSOR_ARROW,       // MAGIC
		SDL_SYSTEM_CURSOR_HAND,        // BUY
		SDL_SYSTEM_CURSOR_HAND,        // SELL

		SDL_SYSTEM_CURSOR_SIZEALL,     // CAMERA_ROTATE
		SDL_SYSTEM_CURSOR_SIZEWE,      // HSIZE
		SDL_SYSTEM_CURSOR_SIZENS,      // VSIZE
		SDL_SYSTEM_CURSOR_SIZENWSE,    // HVSIZE
	};

	m_CursorHandleMap.clear();

	for (int i = 0; i < CURSOR_COUNT; ++i)
	{
		HANDLE hCursor = Platform_CreateSystemCursor(s_systemCursors[i]);

		if (NULL == hCursor)
			return false;

		m_CursorHandleMap.insert(TCursorHandleMap::value_type(i, hCursor));
	}

	return true;
}

void CPythonApplication::DestroyCursors()
{
	TCursorHandleMap::iterator itor;
	for (itor = m_CursorHandleMap.begin(); itor != m_CursorHandleMap.end(); ++itor)
	{
		Platform_DestroyCursor(itor->second);
	}
}

void CPythonApplication::SetCursorVisible(BOOL bFlag, bool bLiarCursorOn)
{
	m_bCursorVisible = bFlag;
	m_bLiarCursorOn = bLiarCursorOn;
	
	if (CURSOR_MODE_HARDWARE == m_iCursorMode)
	{
		// SDL_ShowCursor returns the previous state (0/1) instead of the
		// Win32 reference counter, so it must be called once, not in the
		// Win32 do/while loop.
		Platform_ShowCursorFlag(m_bCursorVisible);
	}
}

BOOL CPythonApplication::GetCursorVisible()
{
	return m_bCursorVisible;
}

bool CPythonApplication::GetLiarCursorOn()
{
	return m_bLiarCursorOn;
}

int CPythonApplication::GetCursorMode()
{
	return m_iCursorMode;
}

BOOL CPythonApplication::__IsContinuousChangeTypeCursor(int iCursorNum)
{
	switch (iCursorNum)
	{
		case CURSOR_SHAPE_NORMAL:
		case CURSOR_SHAPE_ATTACK:
		case CURSOR_SHAPE_TARGET:
		case CURSOR_SHAPE_MAGIC:
		case CURSOR_SHAPE_BUY:
		case CURSOR_SHAPE_SELL:
			return TRUE;
			break;
	}

	return FALSE;
}

BOOL CPythonApplication::SetCursorNum(int iCursorNum)
{
	if (CURSOR_SHAPE_NORMAL == iCursorNum)
	{
		if (!__IsContinuousChangeTypeCursor(m_iCursorNum))
		{
			iCursorNum = m_iContinuousCursorNum;
		}
	}
	else
	{
		if (__IsContinuousChangeTypeCursor(m_iCursorNum))		// 현재 커서가 지속 커서일때만
		{
			m_iContinuousCursorNum = m_iCursorNum;			// 현재의 커서를 저장한다.
		}
	}

	if (CURSOR_MODE_HARDWARE == m_iCursorMode)
	{
		TCursorHandleMap::iterator itor = m_CursorHandleMap.find(iCursorNum);
		if (m_CursorHandleMap.end() == itor)
			return FALSE;

		HANDLE hCursor = itor->second;

		Platform_SetCursorHandle(hCursor);
		m_hCurrentCursor = hCursor;
	}

	m_iCursorNum = iCursorNum;

	PyCallClassMemberFunc(m_poMouseHandler, "ChangeCursor", Py_BuildValue("(i)", m_iCursorNum));

	return TRUE;
}

void CPythonApplication::SetCursorMode(int iMode)
{
	switch (iMode)
	{
		case CURSOR_MODE_HARDWARE:
			m_iCursorMode = CURSOR_MODE_HARDWARE;
			Platform_ShowCursorFlag(TRUE);
			break;

		case CURSOR_MODE_SOFTWARE:
			m_iCursorMode = CURSOR_MODE_SOFTWARE;
			Platform_SetCursorHandle(NULL);
			Platform_ShowCursorFlag(FALSE);
			break;
	}
}
