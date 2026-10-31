#pragma once

#ifndef _CRT_SECURE_NO_WARNINGS
#define _CRT_SECURE_NO_WARNINGS
#endif

#if (defined(_WIN32) || defined(_WIN64)) && !defined(USE_PLATFORM_COMPAT)
#define WIN32_LEAN_AND_MEAN
#pragma warning(disable:4710)	// not inlined
#pragma warning(disable:4786)	// character 255 넘어가는거 끄기
#pragma warning(disable:4244)	// type conversion possible lose of data

#include <windows.h>
#include <assert.h>
#include <stdio.h>
#pragma warning ( disable : 4201 )
#include <mmsystem.h>
#pragma warning ( default : 4201 )
#include <imagehlp.h>
#include <time.h>
#else
#include <Platform/Platform.h>
#include <cassert>
#include <cstdio>
#include <ctime>
#endif

#include <algorithm>
#include <string>
#include <vector>
#include <deque>
#include <list>
#include <map>

#if defined(_MSC_VER) && _MSC_VER >= 1400
#define stricmp _stricmp
#define strnicmp _strnicmp
#define strupt _strupr
#define strcmpi _strcmpi
#define fileno _fileno
#define atoi _atoi64
#endif

#include "vk.h"
#include "Filename.h"
#include "ServiceDefs.h"
