#ifndef __INC_ETERPACK_INLINE_H__
#define __INC_ETERPACK_INLINE_H__

#include <Platform/Platform.h>
#include <cstring>
#include <cctype>

inline void inlinePathCreate(const char* path)
{
	char dir[256];
	const char* p;
	const char* k;
	
	p = (strlen(path) > 3 && path[1] == ':') ? (path + 3) : path;
	
	while (NULL != (k = strchr(p, '/')))
	{
		size_t len = static_cast<size_t>(k - path);
		if (len < sizeof(dir))
		{
			memset(dir, 0, sizeof(dir));
			strncpy(dir, path, len);
			CreateDirectory(dir, NULL);
		}
		p = k + 1;
	}
}

inline void inlineConvertPackFilename(char* name)
{
	char * p = name;

	while (*p)
	{
		if (*p == '\\')
			*p = '/';
		else
			*p = static_cast<char>(tolower(static_cast<unsigned char>(*p)));
		p++;
	}
}

#endif
