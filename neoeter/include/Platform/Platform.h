#pragma once

#if defined(_WIN32) || defined(_WIN64)
    #ifndef WIN32_LEAN_AND_MEAN
        #define WIN32_LEAN_AND_MEAN
    #endif
    #include <windows.h>
// DirectInput Keyboard Scan Codes
#ifndef DIK_ESCAPE
#define DIK_ESCAPE          0x01
#define DIK_1               0x02
#define DIK_2               0x03
#define DIK_3               0x04
#define DIK_4               0x05
#define DIK_5               0x06
#define DIK_6               0x07
#define DIK_7               0x08
#define DIK_8               0x09
#define DIK_9               0x0A
#define DIK_0               0x0B
#define DIK_MINUS           0x0C
#define DIK_EQUALS          0x0D
#define DIK_BACK            0x0E
#define DIK_TAB             0x0F
#define DIK_Q               0x10
#define DIK_W               0x11
#define DIK_E               0x12
#define DIK_R               0x13
#define DIK_T               0x14
#define DIK_Y               0x15
#define DIK_U               0x16
#define DIK_I               0x17
#define DIK_O               0x18
#define DIK_P               0x19
#define DIK_LBRACKET        0x1A
#define DIK_RBRACKET        0x1B
#define DIK_RETURN          0x1C
#define DIK_LCONTROL        0x1D
#define DIK_A               0x1E
#define DIK_S               0x1F
#define DIK_D               0x20
#define DIK_F               0x21
#define DIK_G               0x22
#define DIK_H               0x23
#define DIK_J               0x24
#define DIK_K               0x25
#define DIK_L               0x26
#define DIK_SEMICOLON       0x27
#define DIK_APOSTROPHE      0x28
#define DIK_GRAVE           0x29
#define DIK_LSHIFT          0x2A
#define DIK_BACKSLASH       0x2B
#define DIK_Z               0x2C
#define DIK_X               0x2D
#define DIK_C               0x2E
#define DIK_V               0x2F
#define DIK_B               0x30
#define DIK_N               0x31
#define DIK_M               0x32
#define DIK_COMMA           0x33
#define DIK_PERIOD          0x34
#define DIK_SLASH           0x35
#define DIK_RSHIFT          0x36
#define DIK_MULTIPLY        0x37
#define DIK_LMENU           0x38
#define DIK_SPACE           0x39
#define DIK_CAPITAL         0x3A
#define DIK_F1              0x3B
#define DIK_F2              0x3C
#define DIK_F3              0x3D
#define DIK_F4              0x3E
#define DIK_F5              0x3F
#define DIK_F6              0x40
#define DIK_F7              0x41
#define DIK_F8              0x42
#define DIK_F9              0x43
#define DIK_F10             0x44
#define DIK_NUMLOCK         0x45
#define DIK_SCROLL          0x46
#define DIK_NUMPAD7         0x47
#define DIK_NUMPAD8         0x48
#define DIK_NUMPAD9         0x49
#define DIK_SUBTRACT        0x4A
#define DIK_NUMPAD4         0x4B
#define DIK_NUMPAD5         0x4C
#define DIK_NUMPAD6         0x4D
#define DIK_ADD             0x4E
#define DIK_NUMPAD1         0x4F
#define DIK_NUMPAD2         0x50
#define DIK_NUMPAD3         0x51
#define DIK_NUMPAD0         0x52
#define DIK_DECIMAL         0x53
#define DIK_OEM_102         0x56
#define DIK_F11             0x57
#define DIK_F12             0x58
#define DIK_F13             0x64
#define DIK_F14             0x65
#define DIK_F15             0x66
#define DIK_KANA            0x70
#define DIK_ABNT_C1         0x73
#define DIK_CONVERT         0x79
#define DIK_NOCONVERT       0x7B
#define DIK_YEN             0x7D
#define DIK_ABNT_C2         0x7E
#define DIK_NUMPADEQUALS    0x8D
#define DIK_PREVTRACK       0x90
#define DIK_AT              0x91
#define DIK_COLON           0x92
#define DIK_UNDERLINE       0x93
#define DIK_KANJI           0x94
#define DIK_STOP            0x95
#define DIK_AX              0x96
#define DIK_UNLABELED       0x97
#define DIK_NEXTTRACK       0x99
#define DIK_NUMPADENTER     0x9C
#define DIK_RCONTROL        0x9D
#define DIK_MUTE            0xA0
#define DIK_CALCULATOR      0xA1
#define DIK_PLAYPAUSE       0xA2
#define DIK_MEDIASTOP       0xA4
#define DIK_VOLUMEDOWN      0xAE
#define DIK_VOLUMEUP        0xB0
#define DIK_WEBHOME         0xB2
#define DIK_NUMPADCOMMA     0xB3
#define DIK_DIVIDE          0xB5
#define DIK_SYSRQ           0xB7
#define DIK_RMENU           0xB8
#define DIK_PAUSE           0xC5
#define DIK_HOME            0xC7
#define DIK_UP              0xC8
#define DIK_PRIOR           0xC9
#define DIK_LEFT            0xCB
#define DIK_RIGHT           0xCD
#define DIK_END             0xCF
#define DIK_DOWN            0xD0
#define DIK_NEXT            0xD1
#define DIK_INSERT          0xD2
#define DIK_DELETE          0xD3
#define DIK_LWIN            0xDB
#define DIK_RWIN            0xDC
#define DIK_APPS            0xDD
#define DIK_POWER           0xDE
#define DIK_SLEEP           0xDF
#define DIK_WAKE            0xE3
#define DIK_WEBSEARCH       0xE5
#define DIK_WEBFAVORITES    0xE6
#define DIK_WEBREFRESH      0xE7
#define DIK_WEBSTOP         0xE8
#define DIK_WEBFORWARD      0xE9
#define DIK_WEBBACK         0xEA
#define DIK_MYCOMPUTER      0xEB
#define DIK_MAIL            0xEC
#define DIK_MEDIASELECT     0xED
#endif

    #ifdef __cplusplus
        // T1.9: MinGW's windows.h does not define the min/max macros MSVC
        // provided, so publish the same helpers the non-Windows branch uses
        // (including mixed-type calls such as min(int, size_t)).
        #include <algorithm>
        #include <type_traits>
        using std::min;
        using std::max;
        template <typename T, typename U, typename std::enable_if<!std::is_same<T, U>::value && (std::is_arithmetic<T>::value || std::is_enum<T>::value) && (std::is_arithmetic<U>::value || std::is_enum<U>::value), int>::type = 0>
        inline typename std::common_type<T, U>::type min(T a, U b) {
            using R = typename std::common_type<T, U>::type;
            return (static_cast<R>(a) < static_cast<R>(b)) ? static_cast<R>(a) : static_cast<R>(b);
        }
        template <typename T, typename U, typename std::enable_if<!std::is_same<T, U>::value && (std::is_arithmetic<T>::value || std::is_enum<T>::value) && (std::is_arithmetic<U>::value || std::is_enum<U>::value), int>::type = 0>
        inline typename std::common_type<T, U>::type max(T a, U b) {
            using R = typename std::common_type<T, U>::type;
            return (static_cast<R>(a) > static_cast<R>(b)) ? static_cast<R>(a) : static_cast<R>(b);
        }
    #endif
#else
    // ========================================================
    // Linux / Non-Windows Compatibility Definitions
    // ========================================================
    #include <cstdint>
    #include <cstddef>
    #include <cstdlib>
    #include <cstring>
    #include <strings.h>
    #include <cassert>
    #include <cstdarg>
    #include <cwchar>
    #include <cwctype>
    #include <alloca.h>
    #include <unistd.h>
    #include <pthread.h>
    #include <sys/stat.h>
    #include <sys/types.h>
    #include <sys/socket.h>
    #include <sys/select.h>
    #include <sys/ioctl.h>
    #include <netinet/in.h>
    #include <arpa/inet.h>
    #include <netdb.h>
    #include <errno.h>
    #include <time.h>
    #include <string>
    #include <glob.h>
    #include <atomic>

    // Basic Win32 Scalar Types
    typedef uint32_t      DWORD;
    typedef uint16_t      WORD;
    typedef uint8_t       BYTE;
    typedef int32_t       BOOL;
    typedef int32_t       LONG;
    typedef uint32_t      ULONG;
    typedef int16_t       SHORT;
    typedef uint16_t      USHORT;
    typedef uint32_t      UINT;
    typedef int32_t       INT;
    typedef int64_t       LONGLONG;
    typedef uint64_t      ULONGLONG;
    typedef int64_t       __int64;
    typedef float         FLOAT;
    typedef void          VOID;

    #ifndef IN
        #define IN
    #endif
    #ifndef OUT
        #define OUT
    #endif
    typedef char          CHAR;
    typedef wchar_t       WCHAR;
    typedef char          TCHAR;

    // Pointer-Sized Integer Types (64-bit safe)
    typedef intptr_t      INT_PTR;
    typedef uintptr_t     UINT_PTR;
    typedef intptr_t      LONG_PTR;
    typedef uintptr_t     ULONG_PTR;
    typedef uintptr_t     DWORD_PTR;
    typedef size_t        SIZE_T;

    typedef UINT_PTR      WPARAM;
    typedef LONG_PTR      LPARAM;
    typedef LONG_PTR      LRESULT;

    typedef BYTE*         LPBYTE;
    typedef BYTE*         PBYTE;
    typedef WORD*         LPWORD;
    typedef DWORD*        LPDWORD;
    typedef BOOL*         LPBOOL;
    typedef INT*          PINT;
    typedef INT*          LPINT;
    typedef UINT*         PUINT;
    typedef UINT*         LPUINT;
    typedef CHAR*         PCHAR;
    typedef unsigned char* PUCHAR;
    typedef void*         PVOID;
    typedef int           BOOLEAN;

    #ifndef MAKEFOURCC
    #define MAKEFOURCC(ch0, ch1, ch2, ch3) \
        ((DWORD)(BYTE)(ch0) | ((DWORD)(BYTE)(ch1) << 8) | \
        ((DWORD)(BYTE)(ch2) << 16) | ((DWORD)(BYTE)(ch3) << 24))
    #endif

    #ifndef LOWORD
    #define LOWORD(l) ((WORD)(((DWORD_PTR)(l)) & 0xffff))
    #endif
    #ifndef HIWORD
    #define HIWORD(l) ((WORD)((((DWORD_PTR)(l)) >> 16) & 0xffff))
    #endif
    #ifndef MAKEWORD
    #define MAKEWORD(a, b) ((WORD)(((BYTE)(((DWORD_PTR)(a)) & 0xff)) | ((WORD)((BYTE)(((DWORD_PTR)(b)) & 0xff))) << 8))
    #endif
    #ifndef MAKELONG
    #define MAKELONG(a, b) ((LONG)(((WORD)(((DWORD_PTR)(a)) & 0xffff)) | ((DWORD)((WORD)(((DWORD_PTR)(b)) & 0xffff))) << 16))
    #endif

    #ifndef UNREFERENCED_PARAMETER
    #define UNREFERENCED_PARAMETER(P) ((void)(P))
    #endif

    #ifndef __min
    #define __min(a, b) (((a) < (b)) ? (a) : (b))
    #endif
    #ifndef __max
    #define __max(a, b) (((a) > (b)) ? (a) : (b))
    #endif

    typedef union _LARGE_INTEGER {
        struct {
            DWORD LowPart;
            LONG  HighPart;
        };
        struct {
            DWORD LowPart;
            LONG  HighPart;
        } u;
        LONGLONG QuadPart;
    } LARGE_INTEGER, *PLARGE_INTEGER;

    typedef struct _GUID {
        DWORD Data1;
        WORD  Data2;
        WORD  Data3;
        BYTE  Data4[8];
        bool operator==(const _GUID& other) const {
            return std::memcmp(this, &other, sizeof(_GUID)) == 0;
        }
        bool operator!=(const _GUID& other) const {
            return !(*this == other);
        }
    } GUID, IID, CLSID;

    typedef const GUID& REFGUID;
    typedef const IID&  REFIID;
    typedef const CLSID& REFCLSID;
    inline bool IsEqualGUID(REFGUID rguid1, REFGUID rguid2) { return rguid1 == rguid2; }
    inline bool IsEqualIID(REFIID riid1, REFIID riid2) { return riid1 == riid2; }

    // Handles & Pointers
    typedef void*         HANDLE;
    typedef void*         HWND;
    typedef void*         HINSTANCE;
    typedef void*         HMODULE;
    typedef void*         HDC;
    typedef void*         HFONT;
    typedef void*         HBITMAP;
    typedef void*         HRGN;
    typedef void*         HMENU;
    typedef void*         HICON;
    typedef void*         HCURSOR;
    typedef void*         HBRUSH;
    typedef void*         HGDIOBJ;
    typedef void*         HIMC;
    typedef void*         HIMCC;
    typedef void*         HKL;
    typedef void*         HKEY;
    typedef void*         LPVOID;
    typedef const void*   LPCVOID;

    typedef WORD          LANGID;
    typedef DWORD         LCID;

    // Strings
    typedef char*         LPSTR;
    typedef const char*   LPCSTR;
    typedef char*         LPTSTR;
    typedef const char*   LPCTSTR;
    typedef wchar_t*      LPWSTR;
    typedef const wchar_t* LPCWSTR;

    // Win32 COM / HRESULT
    typedef int32_t       HRESULT;
    #ifndef S_OK
        #define S_OK ((HRESULT)0L)
    #endif
    #ifndef S_FALSE
        #define S_FALSE ((HRESULT)1L)
    #endif
    #ifndef E_FAIL
        #define E_FAIL ((HRESULT)0x80004005L)
    #endif
    #ifndef E_INVALIDARG
        #define E_INVALIDARG ((HRESULT)0x80070057L)
    #endif
    #ifndef E_NOINTERFACE
        #define E_NOINTERFACE ((HRESULT)0x80004002L)
    #endif
    #ifndef E_OUTOFMEMORY
        #define E_OUTOFMEMORY ((HRESULT)0x8007000EL)
    #endif
    #ifndef SUCCEEDED
        #define SUCCEEDED(hr) (((HRESULT)(hr)) >= 0)
    #endif
    #ifndef FAILED
        #define FAILED(hr) (((HRESULT)(hr)) < 0)
    #endif

    // Win32 Constants
    #ifndef TRUE
        #define TRUE 1
    #endif
    #ifndef FALSE
        #define FALSE 0
    #endif
    #ifndef MAX_PATH
        #define MAX_PATH 260
    #endif
    #ifndef _MAX_PATH
        #define _MAX_PATH MAX_PATH
    #endif
    #ifndef INVALID_HANDLE_VALUE
        #define INVALID_HANDLE_VALUE ((HANDLE)(intptr_t)-1)
    #endif

    #ifndef MB_OK
        #define MB_OK 0x00000000L
    #endif
    #ifndef MB_TOPMOST
        #define MB_TOPMOST 0x00040000L
    #endif
    #ifndef MB_ICONSTOP
        #define MB_ICONSTOP         0x00000010L
        #define MB_ICONERROR        0x00000010L
        #define MB_ICONQUESTION     0x00000020L
        #define MB_ICONWARNING      0x00000030L
        #define MB_ICONINFORMATION  0x00000040L
    #endif
    #ifndef IDOK
        #define IDOK 1
    #endif

    inline int MessageBoxA(HWND, LPCSTR, LPCSTR, UINT) { return IDOK; }
    #ifndef MessageBox
        #define MessageBox MessageBoxA
    #endif

    inline DWORD GetModuleFileNameA(HMODULE, LPSTR lpFilename, DWORD nSize) {
        if (!lpFilename || nSize == 0) return 0;
        ssize_t len = readlink("/proc/self/exe", lpFilename, nSize - 1);
        if (len != -1) {
            lpFilename[len] = '\0';
            return (DWORD)len;
        }
        strncpy(lpFilename, "metin2client", nSize - 1);
        lpFilename[nSize - 1] = '\0';
        return (DWORD)strlen(lpFilename);
    }
    #ifndef GetModuleFileName
        #define GetModuleFileName GetModuleFileNameA
    #endif
    inline void OutputDebugStringA(LPCSTR) {}
    inline void OutputDebugStringW(LPCWSTR) {}
    #ifndef OutputDebugString
        #define OutputDebugString OutputDebugStringA
    #endif

    inline BOOL DeleteFileA(LPCSTR lpFileName) { return unlink(lpFileName) == 0; }
    #ifndef DeleteFile
        #define DeleteFile DeleteFileA
    #endif

    inline HMODULE GetModuleHandleA(LPCSTR) { return (HMODULE)1; }
    #ifndef GetModuleHandle
        #define GetModuleHandle GetModuleHandleA
    #endif

    // Calling Conventions & Compiler Attributes
    #define CALLBACK
    #define WINAPI
    #define APIENTRY
    #define __stdcall
    #define __cdecl
    #ifndef __forceinline
        #define __forceinline inline __attribute__((always_inline))
    #endif

    // Memory Allocation Helpers
    #define GMEM_FIXED    0x0000
    #define GMEM_MOVEABLE 0x0002
    #define GMEM_ZEROINIT 0x0040
    inline void* GlobalAlloc(UINT uFlags, size_t dwBytes) { (void)uFlags; return malloc(dwBytes); }
    inline void* GlobalFree(void* hMem) { free(hMem); return NULL; }
    inline void* GlobalLock(void* hMem) { return hMem; }
    inline BOOL  GlobalUnlock(void* hMem) { (void)hMem; return FALSE; }

    #ifndef CopyMemory
        #define CopyMemory(dest, src, count) memcpy((dest), (src), (count))
    #endif
    #ifndef ZeroMemory
        #define ZeroMemory(dest, count) memset((dest), 0, (count))
    #endif
    #ifndef MoveMemory
        #define MoveMemory(dest, src, count) memmove((dest), (src), (count))
    #endif

    // String & Path Helpers
    #ifndef _stricmp
        #define _stricmp strcasecmp
    #endif
    #ifndef stricmp
        #define stricmp strcasecmp
    #endif
    #ifndef strcmpi
        #define strcmpi strcasecmp
    #endif
    #ifndef _strnicmp
        #define _strnicmp strncasecmp
    #endif
    #ifndef strnicmp
        #define strnicmp strncasecmp
    #endif
    #ifndef _snprintf
        #define _snprintf snprintf
    #endif
    #ifndef _vsnprintf
        #define _vsnprintf vsnprintf
    #endif
    #ifndef _getcwd
        #define _getcwd getcwd
    #endif
    #ifndef _chdir
        #define _chdir chdir
    #endif
    #ifndef _access
        #define _access access
    #endif
    #ifndef _unlink
        #define _unlink unlink
    #endif
    #ifndef _alloca
        #define _alloca alloca
    #endif
    #ifndef lstrlenA
        #define lstrlenA strlen
    #endif
    #ifndef lstrlen
        #define lstrlen strlen
    #endif

    inline DWORD GetCurrentDirectoryA(DWORD nBufferLength, LPSTR lpBuffer) {
        if (!lpBuffer || nBufferLength == 0) return 0;
        if (getcwd(lpBuffer, nBufferLength) == nullptr) return 0;
        return (DWORD)strlen(lpBuffer);
    }
    #ifndef GetCurrentDirectory
        #define GetCurrentDirectory GetCurrentDirectoryA
    #endif

    inline BOOL MoveFileA(LPCSTR lpExistingFileName, LPCSTR lpNewFileName) {
        return rename(lpExistingFileName, lpNewFileName) == 0 ? TRUE : FALSE;
    }
    #ifndef MoveFile
        #define MoveFile MoveFileA
    #endif

    inline int LoadStringA(HINSTANCE, UINT, LPSTR, int) { return 0; }
    #ifndef LoadString
        #define LoadString LoadStringA
    #endif

    inline DWORD GetPrivateProfileStringA(LPCSTR /*lpAppName*/, LPCSTR /*lpKeyName*/, LPCSTR lpDefault, LPSTR lpReturnedString, DWORD nSize, LPCSTR /*lpFileName*/) {
        if (!lpReturnedString || nSize == 0) return 0;
        if (lpDefault) {
            strncpy(lpReturnedString, lpDefault, nSize - 1);
            lpReturnedString[nSize - 1] = '\0';
            return (DWORD)strlen(lpReturnedString);
        }
        lpReturnedString[0] = '\0';
        return 0;
    }
    #ifndef GetPrivateProfileString
        #define GetPrivateProfileString GetPrivateProfileStringA
    #endif

    // Structs
    typedef struct tagRECT {
        LONG left;
        LONG top;
        LONG right;
        LONG bottom;
    } RECT, *PRECT, *LPRECT;

    typedef struct tagPOINT {
        LONG x;
        LONG y;
    } POINT, *PPOINT, *LPPOINT;

    typedef struct tagSIZE {
        LONG cx;
        LONG cy;
    } SIZE, *PSIZE, *LPSIZE;

    #ifndef FILE_ATTRIBUTE_DIRECTORY
        #define FILE_ATTRIBUTE_DIRECTORY 0x00000010
    #endif
    #ifndef FILE_ATTRIBUTE_NORMAL
        #define FILE_ATTRIBUTE_NORMAL 0x00000080
    #endif

    typedef struct _WIN32_FIND_DATAA {
        DWORD dwFileAttributes;
        char cFileName[MAX_PATH];
    } WIN32_FIND_DATAA, *PWIN32_FIND_DATAA, *LPWIN32_FIND_DATAA;
    typedef WIN32_FIND_DATAA WIN32_FIND_DATA;

    struct PlatformFindFileData {
        glob_t glob_res;
        size_t index;
    };

    inline HANDLE FindFirstFileA(const char* lpFileName, LPWIN32_FIND_DATAA lpFindFileData) {
        if (!lpFileName || !lpFindFileData) return INVALID_HANDLE_VALUE;
        PlatformFindFileData* pfd = new PlatformFindFileData();
        pfd->index = 0;
        std::string pattern = lpFileName;
        for (char &c : pattern) { if (c == '\\') c = '/'; }
        if (glob(pattern.c_str(), 0, nullptr, &pfd->glob_res) != 0 || pfd->glob_res.gl_pathc == 0) {
            delete pfd;
            return INVALID_HANDLE_VALUE;
        }
        const char* p = pfd->glob_res.gl_pathv[0];
        const char* slash = strrchr(p, '/');
        const char* fname = slash ? (slash + 1) : p;
        strncpy(lpFindFileData->cFileName, fname, MAX_PATH - 1);
        lpFindFileData->cFileName[MAX_PATH - 1] = '\0';
        lpFindFileData->dwFileAttributes = FILE_ATTRIBUTE_NORMAL;
        return (HANDLE)(intptr_t)pfd;
    }

    inline BOOL FindNextFileA(HANDLE hFindFile, LPWIN32_FIND_DATAA lpFindFileData) {
        if (!hFindFile || hFindFile == INVALID_HANDLE_VALUE || !lpFindFileData) return FALSE;
        PlatformFindFileData* pfd = (PlatformFindFileData*)(intptr_t)hFindFile;
        pfd->index++;
        if (pfd->index >= pfd->glob_res.gl_pathc) return FALSE;
        const char* p = pfd->glob_res.gl_pathv[pfd->index];
        const char* slash = strrchr(p, '/');
        const char* fname = slash ? (slash + 1) : p;
        strncpy(lpFindFileData->cFileName, fname, MAX_PATH - 1);
        lpFindFileData->cFileName[MAX_PATH - 1] = '\0';
        lpFindFileData->dwFileAttributes = FILE_ATTRIBUTE_NORMAL;
        return TRUE;
    }

    inline BOOL FindClose(HANDLE hFindFile) {
        if (!hFindFile || hFindFile == INVALID_HANDLE_VALUE) return FALSE;
        PlatformFindFileData* pfd = (PlatformFindFileData*)(intptr_t)hFindFile;
        globfree(&pfd->glob_res);
        delete pfd;
        return TRUE;
    }

    #ifndef FindFirstFile
        #define FindFirstFile FindFirstFileA
    #endif
    #ifndef FindNextFile
        #define FindNextFile FindNextFileA
    #endif

    // ========================================================
    // Threading, Critical Section & Semaphore (POSIX pthreads)
    // ========================================================
    typedef pthread_mutex_t CRITICAL_SECTION, *PCRITICAL_SECTION, *LPCRITICAL_SECTION;

    inline void InitializeCriticalSection(LPCRITICAL_SECTION lpCriticalSection) {
        pthread_mutexattr_t attr;
        pthread_mutexattr_init(&attr);
        pthread_mutexattr_settype(&attr, PTHREAD_MUTEX_RECURSIVE);
        pthread_mutex_init(lpCriticalSection, &attr);
        pthread_mutexattr_destroy(&attr);
    }
    inline void DeleteCriticalSection(LPCRITICAL_SECTION lpCriticalSection) {
        pthread_mutex_destroy(lpCriticalSection);
    }
    inline void EnterCriticalSection(LPCRITICAL_SECTION lpCriticalSection) {
        pthread_mutex_lock(lpCriticalSection);
    }
    inline void LeaveCriticalSection(LPCRITICAL_SECTION lpCriticalSection) {
        pthread_mutex_unlock(lpCriticalSection);
    }

    inline void Sleep(DWORD dwMilliseconds) {
        usleep((useconds_t)dwMilliseconds * 1000);
    }

    #ifndef WAIT_OBJECT_0
        #define WAIT_OBJECT_0 0x00000000L
    #endif
    #ifndef WAIT_TIMEOUT
        #define WAIT_TIMEOUT  0x00000102L
    #endif
    #ifndef WAIT_FAILED
        #define WAIT_FAILED   0xFFFFFFFFL
    #endif
    #ifndef INFINITE
        #define INFINITE      0xFFFFFFFFL
    #endif
    #ifndef THREAD_PRIORITY_NORMAL
        #define THREAD_PRIORITY_NORMAL 0
    #endif

    namespace PlatformPosixSync {
        enum HandleKind { HANDLE_THREAD = 1, HANDLE_SEMAPHORE = 2 };

        struct PosixHandle {
            HandleKind kind;
            pthread_t thread;
            bool joined;
            unsigned (*start_fn)(void*);
            void* arg;
            pthread_mutex_t mutex;
            pthread_cond_t cond;
            long count;
            long maxCount;
        };

        inline void* ThreadTrampoline(void* param) {
            PosixHandle* h = static_cast<PosixHandle*>(param);
            if (h && h->start_fn) {
                h->start_fn(h->arg);
            }
            return nullptr;
        }
    }

    inline uintptr_t _beginthreadex(
        void* /*security*/,
        unsigned /*stack_size*/,
        unsigned (*start_address)(void*),
        void* arglist,
        unsigned /*initflag*/,
        unsigned* thrdaddr)
    {
        PlatformPosixSync::PosixHandle* h = new PlatformPosixSync::PosixHandle();
        h->kind = PlatformPosixSync::HANDLE_THREAD;
        h->joined = false;
        h->start_fn = start_address;
        h->arg = arglist;
        if (pthread_create(&h->thread, nullptr, PlatformPosixSync::ThreadTrampoline, h) != 0) {
            delete h;
            return 0;
        }
        if (thrdaddr) {
            *thrdaddr = (unsigned)(uintptr_t)h->thread;
        }
        return (uintptr_t)h;
    }

    inline BOOL SetThreadPriority(HANDLE /*hThread*/, int /*nPriority*/) {
        return TRUE;
    }

    inline HANDLE CreateSemaphoreA(void* /*lpSemaphoreAttributes*/, LONG lInitialCount, LONG lMaximumCount, LPCSTR /*lpName*/) {
        PlatformPosixSync::PosixHandle* h = new PlatformPosixSync::PosixHandle();
        h->kind = PlatformPosixSync::HANDLE_SEMAPHORE;
        h->joined = false;
        h->start_fn = nullptr;
        h->arg = nullptr;
        pthread_mutex_init(&h->mutex, nullptr);
        pthread_cond_init(&h->cond, nullptr);
        h->count = lInitialCount;
        h->maxCount = lMaximumCount;
        return (HANDLE)h;
    }
    #ifndef CreateSemaphore
        #define CreateSemaphore CreateSemaphoreA
    #endif

    inline BOOL ReleaseSemaphore(HANDLE hSemaphore, LONG lReleaseCount, LONG* lpPreviousCount) {
        if (!hSemaphore || lReleaseCount <= 0) return FALSE;
        PlatformPosixSync::PosixHandle* h = static_cast<PlatformPosixSync::PosixHandle*>(hSemaphore);
        if (h->kind != PlatformPosixSync::HANDLE_SEMAPHORE) return FALSE;
        pthread_mutex_lock(&h->mutex);
        if (lpPreviousCount) *lpPreviousCount = h->count;
        h->count += lReleaseCount;
        if (h->maxCount > 0 && h->count > h->maxCount) h->count = h->maxCount;
        for (LONG i = 0; i < lReleaseCount; ++i) {
            pthread_cond_signal(&h->cond);
        }
        pthread_mutex_unlock(&h->mutex);
        return TRUE;
    }

    inline DWORD WaitForSingleObject(HANDLE hHandle, DWORD dwMilliseconds) {
        if (!hHandle) return WAIT_FAILED;
        PlatformPosixSync::PosixHandle* h = static_cast<PlatformPosixSync::PosixHandle*>(hHandle);
        if (h->kind == PlatformPosixSync::HANDLE_THREAD) {
            if (!h->joined) {
                pthread_join(h->thread, nullptr);
                h->joined = true;
            }
            return WAIT_OBJECT_0;
        } else if (h->kind == PlatformPosixSync::HANDLE_SEMAPHORE) {
            pthread_mutex_lock(&h->mutex);
            if (dwMilliseconds == INFINITE) {
                while (h->count <= 0) {
                    pthread_cond_wait(&h->cond, &h->mutex);
                }
                --h->count;
                pthread_mutex_unlock(&h->mutex);
                return WAIT_OBJECT_0;
            } else {
                struct timespec ts;
                clock_gettime(CLOCK_REALTIME, &ts);
                ts.tv_sec += dwMilliseconds / 1000;
                ts.tv_nsec += (dwMilliseconds % 1000) * 1000000L;
                if (ts.tv_nsec >= 1000000000L) {
                    ts.tv_sec += 1;
                    ts.tv_nsec -= 1000000000L;
                }
                int rc = 0;
                while (h->count <= 0 && rc == 0) {
                    rc = pthread_cond_timedwait(&h->cond, &h->mutex, &ts);
                }
                if (h->count > 0) {
                    --h->count;
                    pthread_mutex_unlock(&h->mutex);
                    return WAIT_OBJECT_0;
                }
                pthread_mutex_unlock(&h->mutex);
                return WAIT_TIMEOUT;
            }
        }
        return WAIT_FAILED;
    }

    inline BOOL CloseHandle(HANDLE hObject) {
        if (!hObject || hObject == INVALID_HANDLE_VALUE) return FALSE;
        PlatformPosixSync::PosixHandle* h = static_cast<PlatformPosixSync::PosixHandle*>(hObject);
        if (h->kind == PlatformPosixSync::HANDLE_THREAD) {
            if (!h->joined) {
                pthread_detach(h->thread);
            }
            delete h;
            return TRUE;
        } else if (h->kind == PlatformPosixSync::HANDLE_SEMAPHORE) {
            pthread_cond_destroy(&h->cond);
            pthread_mutex_destroy(&h->mutex);
            delete h;
            return TRUE;
        }
        return FALSE;
    }

    typedef struct _SECURITY_ATTRIBUTES {
        DWORD  nLength;
        LPVOID lpSecurityDescriptor;
        BOOL   bInheritHandle;
    } SECURITY_ATTRIBUTES, *PSECURITY_ATTRIBUTES, *LPSECURITY_ATTRIBUTES;

    inline HANDLE CreateMutexA(LPSECURITY_ATTRIBUTES, BOOL, LPCSTR) { return (HANDLE)(intptr_t)1; }
    #ifndef CreateMutex
        #define CreateMutex CreateMutexA
    #endif
    inline BOOL ReleaseMutex(HANDLE) { return TRUE; }
    inline HANDLE GetCurrentProcess() { return (HANDLE)(intptr_t)getpid(); }

    // ========================================================
    // Networking Compatibility (Winsock -> POSIX BSD Sockets)
    // ========================================================
    typedef int SOCKET;
    #ifndef INVALID_SOCKET
        #define INVALID_SOCKET (-1)
    #endif
    #ifndef SOCKET_ERROR
        #define SOCKET_ERROR   (-1)
    #endif

    typedef struct sockaddr    SOCKADDR, *PSOCKADDR, *LPSOCKADDR;
    typedef struct sockaddr_in SOCKADDR_IN, *PSOCKADDR_IN, *LPSOCKADDR_IN;
    typedef struct hostent     HOSTENT, *PHOSTENT, *LPHOSTENT;
    typedef struct timeval     TIMEVAL, *PTIMEVAL, *LPTIMEVAL;

    #define WSADESCRIPTION_LEN 256
    #define WSASYS_STATUS_LEN  128
    typedef struct WSAData {
        WORD           wVersion;
        WORD           wHighVersion;
        char           szDescription[WSADESCRIPTION_LEN + 1];
        char           szSystemStatus[WSASYS_STATUS_LEN + 1];
        unsigned short iMaxSockets;
        unsigned short iMaxUdpDg;
        char*          lpVendorInfo;
    } WSADATA, *LPWSADATA;

    inline int WSAStartup(WORD /*wVersionRequested*/, LPWSADATA lpWSAData) {
        if (lpWSAData) std::memset(lpWSAData, 0, sizeof(WSADATA));
        return 0;
    }
    inline int WSACleanup() { return 0; }

    #ifndef WSAEWOULDBLOCK
        #define WSAEWOULDBLOCK EWOULDBLOCK
    #endif

    inline int WSAGetLastError() {
        int e = errno;
        if (e == EINPROGRESS || e == EAGAIN) return WSAEWOULDBLOCK;
        return e;
    }

    inline int closesocket(SOCKET s) {
        return close(s);
    }

    inline int ioctlsocket(SOCKET s, long cmd, DWORD* argp) {
        int flag = argp ? (int)(*argp) : 0;
        return ioctl(s, (unsigned long)cmd, &flag);
    }

    // ========================================================
    // Win32 Windowing, GDI, Font & Input Stubs
    // ========================================================
    typedef LRESULT (CALLBACK* WNDPROC)(HWND, UINT, WPARAM, LPARAM);

    typedef struct tagWNDCLASSA {
        UINT      style;
        WNDPROC   lpfnWndProc;
        int       cbClsExtra;
        int       cbWndExtra;
        HINSTANCE hInstance;
        HICON     hIcon;
        HCURSOR   hCursor;
        HBRUSH    hbrBackground;
        LPCSTR    lpszMenuName;
        LPCSTR    lpszClassName;
    } WNDCLASSA, WNDCLASS, *PWNDCLASS, *LPWNDCLASS;

    typedef struct tagMSG {
        HWND   hwnd;
        UINT   message;
        WPARAM wParam;
        LPARAM lParam;
        DWORD  time;
        POINT  pt;
    } MSG, *PMSG, *LPMSG;

    #ifndef WM_CREATE
        #define WM_CREATE      0x0001
        #define WM_DESTROY     0x0002
        #define WM_MOVE        0x0003
        #define WM_SIZE        0x0005
        #define WM_ACTIVATE    0x0006
        #define WM_SETFOCUS    0x0007
        #define WM_KILLFOCUS   0x0008
        #define WM_PAINT       0x000F
        #define WM_CLOSE       0x0010
        #define WM_QUIT        0x0012
        #define WM_ACTIVATEAPP 0x001C
        #define WM_SETCURSOR   0x0020
        #define WM_INPUTLANGCHANGE      0x0051
        #define WM_KEYDOWN     0x0100
        #define WM_KEYUP       0x0101
        #define WM_CHAR        0x0102
        #define WM_SYSKEYDOWN  0x0104
        #define WM_SYSKEYUP    0x0105
        #define WM_IME_STARTCOMPOSITION 0x010D
        #define WM_IME_ENDCOMPOSITION   0x010E
        #define WM_IME_COMPOSITION      0x010F
        #define WM_IME_KEYLAST          0x010F
        #define WM_MOUSEMOVE   0x0200
        #define WM_LBUTTONDOWN 0x0201
        #define WM_LBUTTONUP   0x0202
        #define WM_RBUTTONDOWN 0x0204
        #define WM_RBUTTONUP   0x0205
        #define WM_MBUTTONDOWN 0x0207
        #define WM_MBUTTONUP   0x0208
        #define WM_MOUSEWHEEL  0x020A
        #define WM_EXITSIZEMOVE 0x0232
        #define WM_IME_SETCONTEXT       0x0281
        #define WM_IME_NOTIFY           0x0282
        #define WM_IME_CONTROL          0x0283
        #define WM_IME_COMPOSITIONFULL  0x0284
        #define WM_IME_SELECT           0x0285
        #define WM_IME_CHAR             0x0286
        #define WM_IME_REQUEST          0x0288
        #define WM_IME_KEYDOWN          0x0290
        #define WM_IME_KEYUP            0x0291

        #define ISC_SHOWUICOMPOSITIONWINDOW     0x80000000
        #define ISC_SHOWUIALLCANDIDATEWINDOW    0x0000000F
    #endif

    #define WA_INACTIVE    0
    #define WA_ACTIVE      1
    #define WA_CLICKACTIVE 2
    #define SIZE_RESTORED  0
    #define SIZE_MINIMIZED 1
    #define SIZE_MAXIMIZED 2

    #define WS_OVERLAPPED       0x00000000L
    #define WS_POPUP            0x80000000L
    #define WS_CHILD            0x40000000L
    #define WS_MINIMIZE         0x20000000L
    #define WS_VISIBLE          0x10000000L
    #define WS_DISABLED         0x08000000L
    #define WS_CLIPSIBLINGS     0x04000000L
    #define WS_CLIPCHILDREN     0x02000000L
    #define WS_MAXIMIZE         0x01000000L
    #define WS_CAPTION          0x00C00000L
    #define WS_BORDER           0x00800000L
    #define WS_DLGFRAME         0x00400000L
    #define WS_VSCROLL          0x00200000L
    #define WS_HSCROLL          0x00100000L
    #define WS_SYSMENU          0x00080000L
    #define WS_THICKFRAME       0x00040000L
    #define WS_GROUP            0x00020000L
    #define WS_TABSTOP          0x00010000L
    #define WS_MINIMIZEBOX      0x00020000L
    #define WS_MAXIMIZEBOX      0x00010000L
    #define WS_OVERLAPPEDWINDOW 0x00CF0000L

    #define SW_HIDE             0
    #define SW_SHOW             5
    #define SW_MINIMIZE         6
    #define SWP_NOSIZE          0x0001
    #define SWP_NOMOVE          0x0002
    #define SWP_NOZORDER        0x0004
    #define SWP_SHOWWINDOW      0x0040
    #define HWND_TOP            ((HWND)(intptr_t)0)
    #define HWND_BOTTOM         ((HWND)(intptr_t)1)
    #define HWND_TOPMOST        ((HWND)(intptr_t)-1)
    #define HWND_NOTOPMOST      ((HWND)(intptr_t)-2)
    #define GWL_USERDATA        (-21)
    #define GWLP_USERDATA       (-21)
    #define GWL_STYLE           (-16)
    #define GWL_EXSTYLE         (-20)
    #define SM_CXSCREEN         0
    #define SM_CYSCREEN         1
    #define SM_CXFULLSCREEN     16
    #define SM_CYFULLSCREEN     17
    #define PM_REMOVE           0x0001
    #define PM_NOREMOVE         0x0000
    #define BLACK_BRUSH         4
    #define IDI_APPLICATION     ((LPCSTR)(uintptr_t)32512)
    #define MAKEINTRESOURCE(i)  ((LPCSTR)(uintptr_t)((WORD)(i)))
    #define KL_NAMELENGTH       9

    typedef struct _OSVERSIONINFOA {
        DWORD dwOSVersionInfoSize;
        DWORD dwMajorVersion;
        DWORD dwMinorVersion;
        DWORD dwBuildNumber;
        DWORD dwPlatformId;
        CHAR  szCSDVersion[128];
    } OSVERSIONINFOA, *POSVERSIONINFOA, *LPOSVERSIONINFOA;

    inline LRESULT DefWindowProcA(HWND, UINT, WPARAM, LPARAM) { return 0; }
    #ifndef DefWindowProc
        #define DefWindowProc DefWindowProcA
    #endif
    inline LONG_PTR GetWindowLongPtrA(HWND, int) { return 0; }
    inline LONG_PTR SetWindowLongPtrA(HWND, int, LONG_PTR) { return 0; }
    inline LONG GetWindowLongA(HWND, int) { return 0; }
    inline LONG SetWindowLongA(HWND, int, LONG) { return 0; }
    #define GetWindowLongPtr GetWindowLongPtrA
    #define SetWindowLongPtr SetWindowLongPtrA
    #define GetWindowLong    GetWindowLongA
    #define SetWindowLong    SetWindowLongA
    #define GetWindowStyle(hwnd)   ((DWORD)0)
    #define GetWindowExStyle(hwnd) ((DWORD)0)

    inline BOOL IsWindow(HWND hWnd) { return hWnd != nullptr; }
    inline BOOL DestroyWindow(HWND) { return TRUE; }
    inline HWND CreateWindowA(LPCSTR, LPCSTR, DWORD, int, int, int, int, HWND, HMENU, HINSTANCE, LPVOID) { return (HWND)1; }
    #define CreateWindow CreateWindowA
    inline BOOL ShowWindow(HWND, int) { return TRUE; }
    inline BOOL InvalidateRect(HWND, const RECT*, BOOL) { return TRUE; }
    inline int  GetSystemMetrics(int nIndex) { return (nIndex == SM_CXSCREEN) ? 1920 : 1080; }
    struct PlatformWindowState {
        int mouseX;
        int mouseY;
        int windowWidth;
        int windowHeight;
        HWND capturedWnd;
    };
    inline PlatformWindowState& GetPlatformWindowState() {
        static PlatformWindowState s_ws = { 0, 0, 1024, 768, (HWND)0 };
        return s_ws;
    }
    inline void Platform_SetCursorPos(int x, int y) {
        GetPlatformWindowState().mouseX = x;
        GetPlatformWindowState().mouseY = y;
    }
    typedef void (*Platform_WarpMouseCallback)(int x, int y);
    inline Platform_WarpMouseCallback& Platform_GetWarpMouseCallback() {
        static Platform_WarpMouseCallback s_cb = nullptr;
        return s_cb;
    }
    inline void Platform_SetWarpMouseCallback(Platform_WarpMouseCallback cb) {
        Platform_GetWarpMouseCallback() = cb;
    }
    inline void Platform_WarpMouse(int x, int y) {
        auto cb = Platform_GetWarpMouseCallback();
        if (cb) cb(x, y);
    }
    inline void Platform_SetWindowResolution(int w, int h) {
        GetPlatformWindowState().windowWidth = w;
        GetPlatformWindowState().windowHeight = h;
    }
    inline void Platform_GetWindowResolution(int* w, int* h) {
        if (w) *w = GetPlatformWindowState().windowWidth;
        if (h) *h = GetPlatformWindowState().windowHeight;
    }
    inline BOOL GetWindowRect(HWND, LPRECT prc) {
        if (prc) {
            prc->left = 0; prc->top = 0;
            prc->right = GetPlatformWindowState().windowWidth;
            prc->bottom = GetPlatformWindowState().windowHeight;
        }
        return TRUE;
    }
    inline BOOL GetClientRect(HWND, LPRECT prc) {
        if (prc) {
            prc->left = 0; prc->top = 0;
            prc->right = GetPlatformWindowState().windowWidth;
            prc->bottom = GetPlatformWindowState().windowHeight;
        }
        return TRUE;
    }
    inline BOOL GetCursorPos(LPPOINT ppt) {
        if (ppt) {
            ppt->x = GetPlatformWindowState().mouseX;
            ppt->y = GetPlatformWindowState().mouseY;
        }
        return TRUE;
    }
    inline BOOL ScreenToClient(HWND, LPPOINT) { return TRUE; }
    inline BOOL SetWindowPos(HWND, HWND, int, int, int, int, UINT) { return TRUE; }
    inline BOOL SetRect(LPRECT lprc, int xLeft, int yTop, int xRight, int yBottom) {
        if (!lprc) return FALSE;
        lprc->left = xLeft; lprc->top = yTop; lprc->right = xRight; lprc->bottom = yBottom;
        return TRUE;
    }
    inline BOOL AdjustWindowRectEx(LPRECT, DWORD, BOOL, DWORD) { return TRUE; }
    inline HMENU GetMenu(HWND) { return nullptr; }
    inline BOOL MoveWindow(HWND, int, int, int, int, BOOL) { return TRUE; }
    inline BOOL SetWindowTextA(HWND, LPCSTR) { return TRUE; }
    #define SetWindowText SetWindowTextA
    inline HCURSOR LoadCursorA(HINSTANCE, LPCSTR) { return nullptr; }
    #define LoadCursor LoadCursorA
    inline HICON LoadIconA(HINSTANCE, LPCSTR) { return nullptr; }
    #define LoadIcon LoadIconA
    inline HGDIOBJ GetStockObject(int) { return nullptr; }
    inline WORD RegisterClassA(const WNDCLASSA*) { return 1; }
    #define RegisterClass RegisterClassA
    inline BOOL PeekMessageA(LPMSG, HWND, UINT, UINT, UINT) { return FALSE; }
    #define PeekMessage PeekMessageA
    inline BOOL GetMessageA(LPMSG, HWND, UINT, UINT) { return FALSE; }
    #define GetMessage GetMessageA
    inline BOOL TranslateMessage(const MSG*) { return TRUE; }
    inline LRESULT DispatchMessageA(const MSG*) { return 0; }
    #define DispatchMessage DispatchMessageA
    typedef void (*Platform_QuitCallback)(int);
    inline Platform_QuitCallback& Platform_GetQuitCallback() {
        static Platform_QuitCallback s_cb = nullptr;
        return s_cb;
    }
    inline void Platform_SetQuitCallback(Platform_QuitCallback cb) {
        Platform_GetQuitCallback() = cb;
    }
    inline std::atomic<bool>& Platform_GetQuitRequested() {
        static std::atomic<bool> s_quitRequested(false);
        return s_quitRequested;
    }
    inline bool Platform_IsQuitRequested() {
        return Platform_GetQuitRequested().load();
    }
    inline void PostQuitMessage(int exitCode) {
        Platform_GetQuitRequested().store(true);
        Platform_QuitCallback cb = Platform_GetQuitCallback();
        if (cb) {
            cb(exitCode);
        }
    }
    inline SHORT GetKeyState(int /*nVirtKey*/) { return 0; }
    inline SHORT GetAsyncKeyState(int /*vKey*/) { return 0; }
    inline BOOL ClientToScreen(HWND /*hWnd*/, LPPOINT /*lpPoint*/) { return TRUE; }
    inline HANDLE LoadImageA(HINSTANCE, LPCSTR, UINT, int, int, UINT) { return (HANDLE)(intptr_t)1; }
    #ifndef LoadImage
        #define LoadImage LoadImageA
    #endif
    inline DWORD GetLastError() { return 0; }
    inline void SetLastError(DWORD) {}
    inline HWND FindWindowA(LPCSTR, LPCSTR) { return nullptr; }
    #ifndef FindWindow
        #define FindWindow FindWindowA
    #endif

    struct STICKYKEYS {
        DWORD cbSize;
        DWORD dwFlags;
    };
    #define SPI_GETSTICKYKEYS   0x003A
    #define SPI_SETSTICKYKEYS   0x003B
    #define SKF_AVAILABLE       0x00000002
    #define SKF_HOTKEYACTIVE    0x00000004

    inline BOOL SystemParametersInfoA(UINT, UINT, PVOID, UINT) { return TRUE; }
    #ifndef SystemParametersInfo
        #define SystemParametersInfo SystemParametersInfoA
    #endif

    inline HWND SetCapture(HWND hWnd) {
        HWND prev = GetPlatformWindowState().capturedWnd;
        GetPlatformWindowState().capturedWnd = hWnd;
        return prev;
    }
    inline BOOL ReleaseCapture() {
        GetPlatformWindowState().capturedWnd = (HWND)0;
        return TRUE;
    }
    inline HWND GetCapture() {
        return GetPlatformWindowState().capturedWnd;
    }

    typedef struct _devicemodeA {
        BYTE  dmDeviceName[32];
        WORD  dmSpecVersion;
        WORD  dmDriverVersion;
        WORD  dmSize;
        WORD  dmDriverExtra;
        DWORD dmFields;
        short dmOrientation;
        short dmPaperSize;
        short dmPaperLength;
        short dmPaperWidth;
        short dmScale;
        short dmCopies;
        short dmDefaultSource;
        short dmPrintQuality;
        short dmColor;
        short dmDuplex;
        short dmYResolution;
        short dmTTOption;
        short dmCollate;
        BYTE  dmFormName[32];
        WORD  dmLogPixels;
        DWORD dmBitsPerPel;
        DWORD dmPelsWidth;
        DWORD dmPelsHeight;
        DWORD dmDisplayFlags;
        DWORD dmDisplayFrequency;
        DWORD dmICMMethod;
        DWORD dmICMIntent;
        DWORD dmMediaType;
        DWORD dmDitherType;
        DWORD dmReserved1;
        DWORD dmReserved2;
        DWORD dmPanningWidth;
        DWORD dmPanningHeight;
    } DEVMODEA, *PDEVMODEA, *LPDEVMODEA;
    typedef DEVMODEA DEVMODE;

    #define DM_BITSPERPEL       0x00040000L
    #define DM_PELSWIDTH        0x00080000L
    #define DM_PELSHEIGHT       0x00100000L
    #define CDS_FULLSCREEN      0x00000004
    #define DISP_CHANGE_SUCCESSFUL 0
    #define DISP_CHANGE_RESTART    1
    #define DISP_CHANGE_FAILED    -1

    inline LONG ChangeDisplaySettingsA(DEVMODEA*, DWORD) { return DISP_CHANGE_SUCCESSFUL; }
    #ifndef ChangeDisplaySettings
        #define ChangeDisplaySettings ChangeDisplaySettingsA
    #endif

    // GDI & Font Definitions
    #define BI_RGB             0L
    #define DIB_RGB_COLORS     0
    #define TRANSPARENT        1
    #define OPAQUE             2
    #define LF_FACESIZE        32

    #define DEFAULT_CHARSET    1
    #define SHIFTJIS_CHARSET   128
    #define HANGUL_CHARSET     129
    #define GB2312_CHARSET     134
    #define CHINESEBIG5_CHARSET 136
    #define GREEK_CHARSET      161
    #define TURKISH_CHARSET    162
    #define VIETNAMESE_CHARSET 163
    #define HEBREW_CHARSET     177
    #define ARABIC_CHARSET     178
    #define BALTIC_CHARSET     186
    #define RUSSIAN_CHARSET    204
    #define THAI_CHARSET       222
    #define EASTEUROPE_CHARSET 238

    #define FW_NORMAL          400
    #define FW_BOLD            700
    #define OUT_DEFAULT_PRECIS 0
    #define CLIP_DEFAULT_PRECIS 0
    #define ANTIALIASED_QUALITY 4
    #define DEFAULT_PITCH      0

    #ifndef RGB
        #define RGB(r,g,b) ((DWORD)(((BYTE)(r)|((WORD)((BYTE)(g))<<8))|(((DWORD)(BYTE)(b))<<16)))
    #endif

    typedef struct tagBITMAPINFOHEADER {
        DWORD biSize;
        LONG  biWidth;
        LONG  biHeight;
        WORD  biPlanes;
        WORD  biBitCount;
        DWORD biCompression;
        DWORD biSizeImage;
        LONG  biXPelsPerMeter;
        LONG  biYPelsPerMeter;
        DWORD biClrUsed;
        DWORD biClrImportant;
    } BITMAPINFOHEADER, *LPBITMAPINFOHEADER, *PBITMAPINFOHEADER;

    typedef struct tagRGBQUAD {
        BYTE rgbBlue;
        BYTE rgbGreen;
        BYTE rgbRed;
        BYTE rgbReserved;
    } RGBQUAD;

    typedef struct tagBITMAPINFO {
        BITMAPINFOHEADER bmiHeader;
        RGBQUAD          bmiColors[1];
    } BITMAPINFO, *LPBITMAPINFO, *PBITMAPINFO;

    typedef struct tagLOGFONTA {
        LONG lfHeight;
        LONG lfWidth;
        LONG lfEscapement;
        LONG lfOrientation;
        LONG lfWeight;
        BYTE lfItalic;
        BYTE lfUnderline;
        BYTE lfStrikeOut;
        BYTE lfCharSet;
        BYTE lfOutPrecision;
        BYTE lfClipPrecision;
        BYTE lfQuality;
        BYTE lfPitchAndFamily;
        CHAR lfFaceName[LF_FACESIZE];
    } LOGFONTA, LOGFONT, *PLOGFONT, *LPLOGFONT;

    typedef struct tagTEXTMETRICA {
        LONG tmHeight;
        LONG tmAscent;
        LONG tmDescent;
        LONG tmInternalLeading;
        LONG tmExternalLeading;
        LONG tmAveCharWidth;
        LONG tmMaxCharWidth;
        LONG tmWeight;
        LONG tmOverhang;
        LONG tmDigitizedAspectX;
        LONG tmDigitizedAspectY;
        BYTE tmFirstChar;
        BYTE tmLastChar;
        BYTE tmDefaultChar;
        BYTE tmBreakChar;
        BYTE tmItalic;
        BYTE tmUnderlined;
        BYTE tmStruckOut;
        BYTE tmPitchAndFamily;
        BYTE tmCharSet;
    } TEXTMETRICA, TEXTMETRIC, *PTEXTMETRIC, *LPTEXTMETRIC;

    typedef struct _ABCFLOAT {
        FLOAT abcfA;
        FLOAT abcfB;
        FLOAT abcfC;
    } ABCFLOAT, *PABCFLOAT, *LPABCFLOAT;

    typedef int (CALLBACK* FONTENUMPROCA)(const LOGFONTA*, const TEXTMETRICA*, DWORD, LPARAM);
    typedef FONTENUMPROCA FONTENUMPROC;

    inline HDC  GetDC(HWND) { return (HDC)1; }
    inline int  ReleaseDC(HWND, HDC) { return 1; }
    inline HDC  CreateCompatibleDC(HDC) { return (HDC)1; }
    inline BOOL DeleteDC(HDC) { return TRUE; }
    inline HBITMAP CreateDIBSection(HDC, const BITMAPINFO* pbmi, UINT, void** ppvBits, HANDLE, DWORD) {
        if (!pbmi || !ppvBits) return nullptr;
        int w = pbmi->bmiHeader.biWidth > 0 ? pbmi->bmiHeader.biWidth : -pbmi->bmiHeader.biWidth;
        int h = pbmi->bmiHeader.biHeight > 0 ? pbmi->bmiHeader.biHeight : -pbmi->bmiHeader.biHeight;
        if (w <= 0) w = 1;
        if (h <= 0) h = 1;
        void* buf = std::calloc((size_t)w * (size_t)h, 4);
        *ppvBits = buf;
        return (HBITMAP)buf;
    }
    inline BOOL DeleteObject(HGDIOBJ hObj) {
        // Note: DIB section buffers are freed by CGraphicDib or DeleteObject if non-sentinel
        if (hObj && hObj != (HGDIOBJ)1) {
            std::free(hObj);
        }
        return TRUE;
    }
    inline HGDIOBJ SelectObject(HDC, HGDIOBJ h) { return h; }
    inline DWORD SetTextColor(HDC, DWORD c) { return c; }
    inline DWORD SetBkColor(HDC, DWORD c) { return c; }
    inline int   SetBkMode(HDC, int m) { return m; }
    inline BOOL  TextOutA(HDC, int, int, LPCSTR, int) { return TRUE; }
    inline BOOL  TextOutW(HDC, int, int, LPCWSTR, int) { return TRUE; }
    #define TextOut TextOutA
    inline int   SetDIBitsToDevice(HDC, int, int, DWORD, DWORD, int, int, UINT, UINT, const void*, const BITMAPINFO*, UINT) { return 0; }
    inline HFONT CreateFontIndirectA(const LOGFONTA*) { return (HFONT)1; }
    #define CreateFontIndirect CreateFontIndirectA
    inline BOOL  GetTextExtentPoint32A(HDC, LPCSTR lpString, int c, LPSIZE lpSize) {
        if (lpSize) { lpSize->cx = c * 8; lpSize->cy = 14; }
        (void)lpString;
        return TRUE;
    }
    inline BOOL  GetTextExtentPoint32W(HDC, LPCWSTR lpString, int c, LPSIZE lpSize) {
        if (lpSize) { lpSize->cx = c * 8; lpSize->cy = 14; }
        (void)lpString;
        return TRUE;
    }
    #define GetTextExtentPoint32 GetTextExtentPoint32A
    inline BOOL  GetCharABCWidthsFloatW(HDC, UINT, UINT, LPABCFLOAT lpABCF) {
        if (lpABCF) { lpABCF->abcfA = 0.0f; lpABCF->abcfB = 8.0f; lpABCF->abcfC = 0.0f; }
        return TRUE;
    }
    inline int   EnumFontFamiliesExA(HDC, LPLOGFONT, FONTENUMPROCA, LPARAM, DWORD) { return 0; }
    #define EnumFontFamiliesEx EnumFontFamiliesExA

    // Clipboard & CodePage Helpers
    #define CF_TEXT 1
    #define CP_ACP  0
    #define CP_UTF8 65001

    inline BOOL   OpenClipboard(HWND) { return FALSE; }
    inline BOOL   CloseClipboard() { return TRUE; }
    inline HANDLE GetClipboardData(UINT) { return nullptr; }

    // Language ID Constants (used by CIME::GetCodePageFromLang)
    #define PRIMARYLANGID(lgid)    ((WORD)(lgid) & 0x3ff)
    #define SUBLANGID(lgid)        ((WORD)(lgid) >> 10)
    #define MAKELANGID(p, s)       ((((WORD)(s)) << 10) | (WORD)(p))
    #define MAKELCID(lgid, srtid)  ((DWORD)((((DWORD)((WORD)(srtid))) << 16) | ((DWORD)((WORD)(lgid)))))
    #define SORT_DEFAULT           0x0

    #define LANG_ARABIC            0x01
    #define LANG_BULGARIAN         0x02
    #define LANG_CHINESE           0x04
    #define LANG_CZECH             0x05
    #define LANG_GERMAN            0x07
    #define LANG_GREEK             0x08
    #define LANG_ENGLISH           0x09
    #define LANG_HUNGARIAN         0x0e
    #define LANG_JAPANESE          0x11
    #define LANG_KOREAN            0x12
    #define LANG_POLISH            0x15
    #define LANG_ROMANIAN          0x18
    #define LANG_RUSSIAN           0x19
    #define LANG_CROATIAN          0x1a
    #define LANG_SLOVAK            0x1b
    #define LANG_THAI              0x1e
    #define LANG_TURKISH           0x1f
    #define LANG_UKRAINIAN         0x22
    #define LANG_BELARUSIAN        0x23
    #define LANG_SLOVENIAN         0x24
    #define LANG_ESTONIAN          0x25
    #define LANG_LATVIAN           0x26
    #define LANG_LITHUANIAN        0x27
    #define LANG_VIETNAMESE        0x2a
    #define LANG_MACEDONIAN        0x2f
    #define LANG_HEBREW            0x0d

    #define SUBLANG_ENGLISH_US          0x01
    #define SUBLANG_CHINESE_TRADITIONAL 0x01
    #define SUBLANG_CHINESE_SIMPLIFIED  0x02
    #define SUBLANG_CHINESE_HONGKONG    0x03
    #define SUBLANG_CHINESE_SINGAPORE   0x04
    #define SUBLANG_CHINESE_MACAU       0x05

    // MultiByteToWideChar / WideCharToMultiByte declarations (implemented in EterLocale/StringCodec.cpp)
    int MultiByteToWideChar(UINT CodePage, DWORD dwFlags, LPCSTR lpMultiByteStr, int cbMultiByte, LPWSTR lpWideCharStr, int cchWideChar);
    int WideCharToMultiByte(UINT CodePage, DWORD dwFlags, LPCWSTR lpWideCharStr, int cchWideChar, LPSTR lpMultiByteStr, int cbMultiByte, LPCSTR lpDefaultChar, LPBOOL lpUsedDefaultChar);

    #ifndef CONST
        #define CONST const
    #endif

    inline DWORD GetTickCount() {
        struct timespec ts;
        clock_gettime(CLOCK_MONOTONIC, &ts);
        return (DWORD)((uint64_t)ts.tv_sec * 1000ULL + (uint64_t)ts.tv_nsec / 1000000ULL);
    }

    inline BOOL QueryPerformanceCounter(LARGE_INTEGER* lpPerformanceCount) {
        if (!lpPerformanceCount) return FALSE;
        struct timespec ts;
        clock_gettime(CLOCK_MONOTONIC, &ts);
        lpPerformanceCount->QuadPart = (LONGLONG)ts.tv_sec * 1000000000LL + (LONGLONG)ts.tv_nsec;
        return TRUE;
    }

    inline BOOL QueryPerformanceFrequency(LARGE_INTEGER* lpFrequency) {
        if (!lpFrequency) return FALSE;
        lpFrequency->QuadPart = 1000000000LL;
        return TRUE;
    }

    #ifndef GetCurrentTime
        #define GetCurrentTime() GetTickCount()
    #endif

    #ifndef timeGetTime
        inline DWORD timeGetTime() { return GetTickCount(); }
    #endif

    inline BOOL FreeLibrary(HMODULE) { return TRUE; }

    #define LOCALE_SABBREVLANGNAME      0x00000003
    #define LOCALE_IDEFAULTANSICODEPAGE 0x00001004

    inline HKL GetKeyboardLayout(DWORD /*idThread*/) { return (HKL)(uintptr_t)0x04090409; }
    inline BOOL GetKeyboardLayoutNameA(LPSTR pwszKLID) {
        if (pwszKLID) std::strcpy(pwszKLID, "00000409");
        return TRUE;
    }
    #ifndef GetKeyboardLayoutName
        #define GetKeyboardLayoutName GetKeyboardLayoutNameA
    #endif

    inline int memcpy_s(void* dest, size_t destSize, const void* src, size_t count) {
        if (!dest || !src || destSize < count) return -1;
        std::memcpy(dest, src, count);
        return 0;
    }

    inline LPSTR CharNextExA(WORD /*CodePage*/, LPCSTR lpCurrentChar, DWORD /*dwFlags*/) {
        if (!lpCurrentChar || !*lpCurrentChar) return const_cast<LPSTR>(lpCurrentChar);
        return const_cast<LPSTR>(lpCurrentChar + 1);
    }

    inline LPSTR CharPrevExA(WORD /*CodePage*/, LPCSTR lpStart, LPCSTR lpCurrentChar, DWORD /*dwFlags*/) {
        if (!lpStart || !lpCurrentChar || lpCurrentChar <= lpStart) return const_cast<LPSTR>(lpStart);
        return const_cast<LPSTR>(lpCurrentChar - 1);
    }

    #ifdef __cplusplus
        #include <algorithm>
        #include <type_traits>
        using std::min;
        using std::max;
        template <typename T, typename U, typename std::enable_if<!std::is_same<T, U>::value && (std::is_arithmetic<T>::value || std::is_enum<T>::value) && (std::is_arithmetic<U>::value || std::is_enum<U>::value), int>::type = 0>
        inline typename std::common_type<T, U>::type min(T a, U b) {
            using R = typename std::common_type<T, U>::type;
            return (static_cast<R>(a) < static_cast<R>(b)) ? static_cast<R>(a) : static_cast<R>(b);
        }
        template <typename T, typename U, typename std::enable_if<!std::is_same<T, U>::value && (std::is_arithmetic<T>::value || std::is_enum<T>::value) && (std::is_arithmetic<U>::value || std::is_enum<U>::value), int>::type = 0>
        inline typename std::common_type<T, U>::type max(T a, U b) {
            using R = typename std::common_type<T, U>::type;
            return (static_cast<R>(a) > static_cast<R>(b)) ? static_cast<R>(a) : static_cast<R>(b);
        }
    #endif

    // Directory Creation Helper
    inline BOOL CreateDirectoryA(LPCSTR lpPathName, LPVOID lpSecurityAttributes) {
        (void)lpSecurityAttributes;
        return mkdir(lpPathName, 0755) == 0;
    }
    #ifndef CreateDirectory
        #define CreateDirectory CreateDirectoryA
    #endif

    // Timer & Multimedia Helpers
    struct TIMECAPS {
        UINT wPeriodMin;
        UINT wPeriodMax;
    };
    #define TIMERR_NOERROR 0
    inline UINT timeGetDevCaps(TIMECAPS* ptc, UINT) {
        if (ptc) { ptc->wPeriodMin = 1; ptc->wPeriodMax = 1000; }
        return TIMERR_NOERROR;
    }
    inline UINT timeBeginPeriod(UINT) { return TIMERR_NOERROR; }
    inline UINT timeEndPeriod(UINT) { return TIMERR_NOERROR; }

    // COM Stubs
    inline HRESULT CoInitialize(void*) { return S_OK; }
    inline void CoUninitialize() {}

    #ifndef FAR
        #define FAR
    #endif

    // Cursor and File Attribute Helpers
    #ifndef IMAGE_CURSOR
        #define IMAGE_CURSOR 2
    #endif
    #ifndef LR_VGACOLOR
        #define LR_VGACOLOR 0x0001
    #endif
    #ifndef FILE_ATTRIBUTE_NORMAL
        #define FILE_ATTRIBUTE_NORMAL 0x00000080
    #endif
    #ifndef FILE_ATTRIBUTE_READONLY
        #define FILE_ATTRIBUTE_READONLY 0x00000001
    #endif

    inline BOOL DestroyCursor(HCURSOR) { return TRUE; }
    // Windows keeps a display-counter whose sign reports visibility. The
    // client hides the cursor with
    //     do { i = ShowCursor(FALSE); } while (i >= 0);
    // so a constant 0 spins forever (the logo-close path hangs before the
    // login window opens). Track the counter with the same sign convention.
    inline int ShowCursor(BOOL bShow) {
        static int s_iShowCount = 0;
        if (bShow) {
            if (s_iShowCount < 0)
                ++s_iShowCount;
        } else {
            if (s_iShowCount > -1)
                --s_iShowCount;
        }
        return s_iShowCount;
    }
    inline HCURSOR SetCursor(HCURSOR) { return nullptr; }
    inline BOOL SetCursorPos(int x, int y) {
        Platform_WarpMouse(x, y);
        Platform_SetCursorPos(x, y);
        return TRUE;
    }
    inline BOOL SetFileAttributes(LPCSTR, DWORD) { return TRUE; }

    // Windows Virtual Key Definitions
    #ifndef VK_LBUTTON
    #define VK_LBUTTON        0x01
    #define VK_RBUTTON        0x02
    #define VK_CANCEL         0x03
    #define VK_MBUTTON        0x04
    #define VK_BACK           0x08
    #define VK_TAB            0x09
    #define VK_CLEAR          0x0C
    #define VK_RETURN         0x0D
    #define VK_SHIFT          0x10
    #define VK_CONTROL        0x11
    #define VK_MENU           0x12
    #define VK_PAUSE          0x13
    #define VK_CAPITAL        0x14
    #define VK_ESCAPE         0x1B
    #define VK_SPACE          0x20
    #define VK_PRIOR          0x21
    #define VK_NEXT           0x22
    #define VK_END            0x23
    #define VK_HOME           0x24
    #define VK_LEFT           0x25
    #define VK_UP             0x26
    #define VK_RIGHT          0x27
    #define VK_DOWN           0x28
    #define VK_SELECT         0x29
    #define VK_PRINT          0x2A
    #define VK_EXECUTE        0x2B
    #define VK_SNAPSHOT       0x2C
    #define VK_INSERT         0x2D
    #define VK_DELETE         0x2E
    #define VK_HELP           0x2F
    #define VK_LWIN           0x5B
    #define VK_RWIN           0x5C
    #define VK_APPS           0x5D
    #define VK_NUMPAD0        0x60
    #define VK_NUMPAD1        0x61
    #define VK_NUMPAD2        0x62
    #define VK_NUMPAD3        0x63
    #define VK_NUMPAD4        0x64
    #define VK_NUMPAD5        0x65
    #define VK_NUMPAD6        0x66
    #define VK_NUMPAD7        0x67
    #define VK_NUMPAD8        0x68
    #define VK_NUMPAD9        0x69
    #define VK_MULTIPLY       0x6A
    #define VK_ADD            0x6B
    #define VK_SEPARATOR      0x6C
    #define VK_SUBTRACT       0x6D
    #define VK_DECIMAL        0x6E
    #define VK_DIVIDE         0x6F
    #define VK_F1             0x70
    #define VK_F2             0x71
    #define VK_F3             0x72
    #define VK_F4             0x73
    #define VK_F5             0x74
    #define VK_F6             0x75
    #define VK_F7             0x76
    #define VK_F8             0x77
    #define VK_F9             0x78
    #define VK_F10            0x79
    #define VK_F11            0x7A
    #define VK_F12            0x7B
    #define VK_SCROLL         0x91
    #define VK_LSHIFT         0xA0
    #define VK_RSHIFT         0xA1
    #define VK_LCONTROL       0xA2
    #define VK_RCONTROL       0xA3
    #define VK_LMENU          0xA4
    #define VK_RMENU          0xA5
    #endif

#endif

// DirectInput keyboard scan codes (the game input ABI).
// Defined on both targets so the SDL2 input path does not depend on
// <dinput.h>; dinput.h keeps its own definitions when it is present,
// which the #ifndef guard below respects.
// DirectInput Keyboard Scan Codes
#ifndef DIK_ESCAPE
#define DIK_ESCAPE          0x01
#define DIK_1               0x02
#define DIK_2               0x03
#define DIK_3               0x04
#define DIK_4               0x05
#define DIK_5               0x06
#define DIK_6               0x07
#define DIK_7               0x08
#define DIK_8               0x09
#define DIK_9               0x0A
#define DIK_0               0x0B
#define DIK_MINUS           0x0C
#define DIK_EQUALS          0x0D
#define DIK_BACK            0x0E
#define DIK_TAB             0x0F
#define DIK_Q               0x10
#define DIK_W               0x11
#define DIK_E               0x12
#define DIK_R               0x13
#define DIK_T               0x14
#define DIK_Y               0x15
#define DIK_U               0x16
#define DIK_I               0x17
#define DIK_O               0x18
#define DIK_P               0x19
#define DIK_LBRACKET        0x1A
#define DIK_RBRACKET        0x1B
#define DIK_RETURN          0x1C
#define DIK_LCONTROL        0x1D
#define DIK_A               0x1E
#define DIK_S               0x1F
#define DIK_D               0x20
#define DIK_F               0x21
#define DIK_G               0x22
#define DIK_H               0x23
#define DIK_J               0x24
#define DIK_K               0x25
#define DIK_L               0x26
#define DIK_SEMICOLON       0x27
#define DIK_APOSTROPHE      0x28
#define DIK_GRAVE           0x29
#define DIK_LSHIFT          0x2A
#define DIK_BACKSLASH       0x2B
#define DIK_Z               0x2C
#define DIK_X               0x2D
#define DIK_C               0x2E
#define DIK_V               0x2F
#define DIK_B               0x30
#define DIK_N               0x31
#define DIK_M               0x32
#define DIK_COMMA           0x33
#define DIK_PERIOD          0x34
#define DIK_SLASH           0x35
#define DIK_RSHIFT          0x36
#define DIK_MULTIPLY        0x37
#define DIK_LMENU           0x38
#define DIK_SPACE           0x39
#define DIK_CAPITAL         0x3A
#define DIK_F1              0x3B
#define DIK_F2              0x3C
#define DIK_F3              0x3D
#define DIK_F4              0x3E
#define DIK_F5              0x3F
#define DIK_F6              0x40
#define DIK_F7              0x41
#define DIK_F8              0x42
#define DIK_F9              0x43
#define DIK_F10             0x44
#define DIK_NUMLOCK         0x45
#define DIK_SCROLL          0x46
#define DIK_NUMPAD7         0x47
#define DIK_NUMPAD8         0x48
#define DIK_NUMPAD9         0x49
#define DIK_SUBTRACT        0x4A
#define DIK_NUMPAD4         0x4B
#define DIK_NUMPAD5         0x4C
#define DIK_NUMPAD6         0x4D
#define DIK_ADD             0x4E
#define DIK_NUMPAD1         0x4F
#define DIK_NUMPAD2         0x50
#define DIK_NUMPAD3         0x51
#define DIK_NUMPAD0         0x52
#define DIK_DECIMAL         0x53
#define DIK_OEM_102         0x56
#define DIK_F11             0x57
#define DIK_F12             0x58
#define DIK_F13             0x64
#define DIK_F14             0x65
#define DIK_F15             0x66
#define DIK_KANA            0x70
#define DIK_ABNT_C1         0x73
#define DIK_CONVERT         0x79
#define DIK_NOCONVERT       0x7B
#define DIK_YEN             0x7D
#define DIK_ABNT_C2         0x7E
#define DIK_NUMPADEQUALS    0x8D
#define DIK_PREVTRACK       0x90
#define DIK_AT              0x91
#define DIK_COLON           0x92
#define DIK_UNDERLINE       0x93
#define DIK_KANJI           0x94
#define DIK_STOP            0x95
#define DIK_AX              0x96
#define DIK_UNLABELED       0x97
#define DIK_NEXTTRACK       0x99
#define DIK_NUMPADENTER     0x9C
#define DIK_RCONTROL        0x9D
#define DIK_MUTE            0xA0
#define DIK_CALCULATOR      0xA1
#define DIK_PLAYPAUSE       0xA2
#define DIK_MEDIASTOP       0xA4
#define DIK_VOLUMEDOWN      0xAE
#define DIK_VOLUMEUP        0xB0
#define DIK_WEBHOME         0xB2
#define DIK_NUMPADCOMMA     0xB3
#define DIK_DIVIDE          0xB5
#define DIK_SYSRQ           0xB7
#define DIK_RMENU           0xB8
#define DIK_PAUSE           0xC5
#define DIK_HOME            0xC7
#define DIK_UP              0xC8
#define DIK_PRIOR           0xC9
#define DIK_LEFT            0xCB
#define DIK_RIGHT           0xCD
#define DIK_END             0xCF
#define DIK_DOWN            0xD0
#define DIK_NEXT            0xD1
#define DIK_INSERT          0xD2
#define DIK_DELETE          0xD3
#define DIK_LWIN            0xDB
#define DIK_RWIN            0xDC
#define DIK_APPS            0xDD
#define DIK_POWER           0xDE
#define DIK_SLEEP           0xDF
#define DIK_WAKE            0xE3
#define DIK_WEBSEARCH       0xE5
#define DIK_WEBFAVORITES    0xE6
#define DIK_WEBREFRESH      0xE7
#define DIK_WEBSTOP         0xE8
#define DIK_WEBFORWARD      0xE9
#define DIK_WEBBACK         0xEA
#define DIK_MYCOMPUTER      0xEB
#define DIK_MAIL            0xEC
#define DIK_MEDIASELECT     0xED
#endif
