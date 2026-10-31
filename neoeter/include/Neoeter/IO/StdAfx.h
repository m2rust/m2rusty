#pragma once

#if (defined(_WIN32) || defined(_WIN64)) && !defined(USE_PLATFORM_COMPAT)
    #define WIN32_LEAN_AND_MEAN
    #include <windows.h>
    #include <assert.h>
#else
    #include <Platform/Platform.h>
    #include <cassert>
#endif

#if __has_include(<Neoeter/Platform/Base/StdAfx.h>)
    #include <Neoeter/Platform/Base/StdAfx.h>
#endif
