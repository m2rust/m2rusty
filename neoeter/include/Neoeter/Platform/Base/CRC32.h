#ifndef __INC_CRC32_H__
#define __INC_CRC32_H__

#if (defined(_WIN32) || defined(_WIN64)) && !defined(USE_PLATFORM_COMPAT)
#include <windows.h>
#else
#include <Platform/Platform.h>
#endif

DWORD GetCRC32(const char* buffer, size_t count);
DWORD GetCaseCRC32(const char * buf, size_t len);
DWORD GetHFILECRC32(HANDLE hFile);
DWORD GetFileCRC32(const char* c_szFileName);
DWORD GetFileSize(const char* c_szFileName);

#endif
