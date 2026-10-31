#include <Neoeter/Platform/Locale/StdAfx.h>
#include <Neoeter/Platform/Locale/StringCodec.h>
#include <Neoeter/Platform/Locale/StringCodec_Vietnamese.h>

#if !defined(_WIN32)
#include <iconv.h>
#include <cerrno>

static const char* GetIconvEncoding(UINT codePage)
{
    switch (codePage)
    {
        case 0:         // CP_ACP
        case 65001:     // CP_UTF8
            return "UTF-8";
        case 949:       // Korean EUC-KR / CP949
            return "CP949";
        case 932:       // Japanese Shift-JIS
            return "CP932";
        case 936:       // Simplified Chinese
            return "CP936";
        case 950:       // Traditional Chinese
            return "CP950";
        case 1250:      // Central / East Europe
            return "CP1250";
        case 1251:      // Cyrillic
            return "CP1251";
        case 1252:      // Latin-1 / Western Europe
            return "CP1252";
        case 1253:      // Greek
            return "CP1253";
        case 1254:      // Turkish
            return "CP1254";
        case 1255:      // Hebrew
            return "CP1255";
        case 1256:      // Arabic
            return "CP1256";
        case 1257:      // Baltic
            return "CP1257";
        case 1258:      // Vietnamese
            return "CP1258";
        case 874:       // Thai
            return "CP874";
        default:
            return "UTF-8";
    }
}

int MultiByteToWideChar(
    UINT CodePage,
    DWORD dwFlags,
    LPCSTR lpMultiByteStr,
    int cbMultiByte,
    LPWSTR lpWideCharStr,
    int cchWideChar
)
{
    (void)dwFlags;
    if (!lpMultiByteStr)
        return 0;

    if (cbMultiByte < 0)
    {
        cbMultiByte = static_cast<int>(strlen(lpMultiByteStr)) + 1;
    }
    else if (cbMultiByte == 0)
    {
        return 0;
    }

    const char* enc = GetIconvEncoding(CodePage);
    iconv_t cd = iconv_open("WCHAR_T", enc);
    if (cd == (iconv_t)-1)
    {
        cd = iconv_open("WCHAR_T", "UTF-8");
        if (cd == (iconv_t)-1)
            return 0;
    }

    char* inBuf = const_cast<char*>(lpMultiByteStr);
    size_t inBytesLeft = static_cast<size_t>(cbMultiByte);

    if (cchWideChar == 0)
    {
        wchar_t tmpBuf[256];
        int totalWChars = 0;

        while (inBytesLeft > 0)
        {
            char* outBuf = reinterpret_cast<char*>(tmpBuf);
            size_t outBytesLeft = sizeof(tmpBuf);

            size_t res = iconv(cd, &inBuf, &inBytesLeft, &outBuf, &outBytesLeft);
            totalWChars += static_cast<int>((sizeof(tmpBuf) - outBytesLeft) / sizeof(wchar_t));

            if (res == (size_t)-1)
            {
                if (errno == E2BIG)
                {
                    continue;
                }
                else
                {
                    inBuf++;
                    inBytesLeft--;
                }
            }
        }

        iconv_close(cd);
        return totalWChars;
    }
    else
    {
        char* outBuf = reinterpret_cast<char*>(lpWideCharStr);
        size_t outBytesLeft = static_cast<size_t>(cchWideChar) * sizeof(wchar_t);
        size_t initialOutBytes = outBytesLeft;

        while (inBytesLeft > 0 && outBytesLeft >= sizeof(wchar_t))
        {
            size_t res = iconv(cd, &inBuf, &inBytesLeft, &outBuf, &outBytesLeft);
            if (res == (size_t)-1)
            {
                if (errno == E2BIG)
                {
                    break;
                }
                else
                {
                    inBuf++;
                    inBytesLeft--;
                }
            }
        }

        int written = static_cast<int>((initialOutBytes - outBytesLeft) / sizeof(wchar_t));
        iconv_close(cd);
        return written;
    }
}

int WideCharToMultiByte(
    UINT CodePage,
    DWORD dwFlags,
    LPCWSTR lpWideCharStr,
    int cchWideChar,
    LPSTR lpMultiByteStr,
    int cbMultiByte,
    LPCSTR lpDefaultChar,
    LPBOOL lpUsedDefaultChar
)
{
    (void)dwFlags;
    (void)lpDefaultChar;
    if (lpUsedDefaultChar)
        *lpUsedDefaultChar = FALSE;

    if (!lpWideCharStr)
        return 0;

    if (cchWideChar < 0)
    {
        cchWideChar = static_cast<int>(wcslen(lpWideCharStr)) + 1;
    }
    else if (cchWideChar == 0)
    {
        return 0;
    }

    const char* enc = GetIconvEncoding(CodePage);
    iconv_t cd = iconv_open(enc, "WCHAR_T");
    if (cd == (iconv_t)-1)
    {
        cd = iconv_open("UTF-8", "WCHAR_T");
        if (cd == (iconv_t)-1)
            return 0;
    }

    char* inBuf = reinterpret_cast<char*>(const_cast<wchar_t*>(lpWideCharStr));
    size_t inBytesLeft = static_cast<size_t>(cchWideChar) * sizeof(wchar_t);

    if (cbMultiByte == 0)
    {
        char tmpBuf[512];
        int totalBytes = 0;

        while (inBytesLeft >= sizeof(wchar_t))
        {
            char* outBuf = tmpBuf;
            size_t outBytesLeft = sizeof(tmpBuf);

            size_t res = iconv(cd, &inBuf, &inBytesLeft, &outBuf, &outBytesLeft);
            totalBytes += static_cast<int>(sizeof(tmpBuf) - outBytesLeft);

            if (res == (size_t)-1)
            {
                if (errno == E2BIG)
                {
                    continue;
                }
                else
                {
                    inBuf += sizeof(wchar_t);
                    inBytesLeft -= sizeof(wchar_t);
                }
            }
        }

        iconv_close(cd);
        return totalBytes;
    }
    else
    {
        char* outBuf = lpMultiByteStr;
        size_t outBytesLeft = static_cast<size_t>(cbMultiByte);
        size_t initialOutBytes = outBytesLeft;

        while (inBytesLeft >= sizeof(wchar_t) && outBytesLeft > 0)
        {
            size_t res = iconv(cd, &inBuf, &inBytesLeft, &outBuf, &outBytesLeft);
            if (res == (size_t)-1)
            {
                if (errno == E2BIG)
                {
                    break;
                }
                else
                {
                    inBuf += sizeof(wchar_t);
                    inBytesLeft -= sizeof(wchar_t);
                }
            }
        }

        int written = static_cast<int>(initialOutBytes - outBytesLeft);
        iconv_close(cd);
        return written;
    }
}
#endif

int Ymir_WideCharToMultiByte(
  UINT CodePage, 
  DWORD dwFlags, 
  LPCWSTR lpWideCharStr, 
  int cchWideChar, 
  LPSTR lpMultiByteStr, 
  int cbMultiByte, 
  LPCSTR lpDefaultChar, 
  LPBOOL lpUsedDefaultChar 
)
{
	if (CodePage == CP_1258)
	{
		return EL_String_Encode_Vietnamese(lpWideCharStr, cchWideChar, lpMultiByteStr, cbMultiByte);
	}
	else
	{
		return WideCharToMultiByte(CodePage, dwFlags, lpWideCharStr, cchWideChar, lpMultiByteStr, cbMultiByte, lpDefaultChar, lpUsedDefaultChar);
	}
}

int Ymir_MultiByteToWideChar(
  UINT CodePage, 
  DWORD dwFlags, 
  LPCSTR lpMultiByteStr, 
  int cbMultiByte, 
  LPWSTR lpWideCharStr, 
  int cchWideChar 
)
{
	if (CodePage == CP_1258)
	{
		return EL_String_Decode_Vietnamese(lpMultiByteStr, cbMultiByte, lpWideCharStr, cchWideChar);
	}
	else
	{
		return MultiByteToWideChar(CodePage, dwFlags, lpMultiByteStr, cbMultiByte, lpWideCharStr, cchWideChar);
	}
}
