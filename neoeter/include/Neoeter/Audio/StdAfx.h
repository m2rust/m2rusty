#ifndef __INC_MILESLIB_STDAFX_H__
#define __INC_MILESLIB_STDAFX_H__

#if _MSC_VER > 1000
#pragma once
#endif // _MSC_VER > 1000

#ifdef _WIN32
#include <windows.h>
#else
#include "Platform/Platform.h"
#endif

//#include <crtdbg.h>

#include <Neoeter/Platform/Base/CRC32.h>
#include <Neoeter/Platform/Base/Stl.h>
#include <Neoeter/Platform/Base/Utils.h>
#include <Neoeter/Platform/Base/Debug.h>

//{{AFX_INSERT_LOCATION}}
// Microsoft Visual C++ will insert additional declarations immediately before the previous line.

#endif