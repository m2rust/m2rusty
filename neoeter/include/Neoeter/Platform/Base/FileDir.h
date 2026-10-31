#ifndef __ETER_FILE_DIR__
#define __ETER_FILE_DIR__

#if (defined(_WIN32) || defined(_WIN64)) && !defined(USE_PLATFORM_COMPAT)
#include <windows.h>
#else
#include <Platform/Platform.h>
#endif

class CDir
{
	public:
		CDir();
		virtual ~CDir();
		
		void Destroy();		
		bool Create(const char* c_szFilter, const char* c_szPath="", BOOL bCheckedExtension = FALSE);
		
	protected:
		virtual bool OnFolder(const char* c_szFilter, const char* c_szPath, const char* c_szName) = 0;
		virtual bool OnFile(const char* c_szPath, const char* c_szName) = 0;
		
	protected:
		bool IsFolder();
		
		void Initialize();
		
	protected:
		WIN32_FIND_DATA m_wfd;
		HANDLE m_hFind;
};


#endif