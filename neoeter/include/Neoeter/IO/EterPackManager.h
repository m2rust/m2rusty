#pragma once

#if (defined(_WIN32) || defined(_WIN64)) && !defined(USE_PLATFORM_COMPAT)
    #include <windows.h>
#else
    #include <Platform/Platform.h>
#endif

#include <unordered_map>
#include <list>
#include <vector>
#include <string>
#include <memory>
#include <sstream>
#include <algorithm>
#include <cctype>

#if __has_include(<Neoeter/Platform/Base/Singleton.h>)
    #include <Neoeter/Platform/Base/Singleton.h>
#elif !defined(__INC_ETERLIB_SINGLETON_H__)
    #define __INC_ETERLIB_SINGLETON_H__
    #include <cassert>
    #include <cstdint>
    template <typename T> class CSingleton
    {
        static T* ms_singleton;
    public:
        CSingleton()
        {
            assert(!ms_singleton);
            intptr_t offset = (intptr_t)(T*)1 - (intptr_t)(CSingleton<T>*)(T*)1;
            ms_singleton = (T*)((intptr_t)this + offset);
        }
        virtual ~CSingleton()
        {
            assert(ms_singleton);
            ms_singleton = nullptr;
        }
        static T& Instance()
        {
            assert(ms_singleton);
            return (*ms_singleton);
        }
        static T* InstancePtr()
        {
            return (ms_singleton);
        }
        static T& instance()
        {
            assert(ms_singleton);
            return (*ms_singleton);
        }
    };
    template <typename T> T* CSingleton<T>::ms_singleton = nullptr;
#endif

#if __has_include(<Neoeter/Platform/Base/Stl.h>)
    #include <Neoeter/Platform/Base/Stl.h>
#else
    inline void stl_lowers(std::string& rstRet)
    {
        for (size_t i = 0; i < rstRet.length(); ++i)
            rstRet[i] = static_cast<char>(std::tolower(static_cast<unsigned char>(rstRet[i])));
    }
#endif

#include "FileProvider.h"

class CEterPackManager : public CSingleton<CEterPackManager>
{
	public:
		enum ESearchModes
		{
			SEARCH_FILE_FIRST,
			SEARCH_PACK_FIRST
		};

		typedef std::list<std::shared_ptr<FileProvider>> TEterPackList;
		typedef std::unordered_map<std::string, std::shared_ptr<FileProvider>> TEterPackMap;
		typedef std::unordered_map<std::string, std::shared_ptr<FileProvider>> TFileMap;
		typedef std::shared_ptr<std::vector<char>> TPackDataPtr;

	public:
		CEterPackManager();
		virtual ~CEterPackManager();
		
		void SetSearchMode(bool bPackFirst);
		int GetSearchMode();

		bool Get(const std::string& fileName, TPackDataPtr& dataPtr);
		bool Get(const std::string& fileName, std::stringstream& dataStream);

		bool isExist(const char * c_szFileName);
		bool isExistInPack(const char * c_szFileName);

		bool PackExists(const std::string& name, const std::string& container);
		bool RegisterPack(const std::string& name, const std::string& container);
		std::string ConvertFileName(std::string fileName);

		const TFileMap& GetFileMap();

	protected:
		bool CompareName(const char * c_szDirectoryName, DWORD iLength, const char * c_szFileName);
		bool GetFromPack(const std::string& fileName, TPackDataPtr& dataPtr);
		bool GetFromFile(const std::string& fileName, TPackDataPtr& dataPtr);

	protected:
		int						m_iSearchMode;

		TFileMap				m_FileMap;
		TEterPackMap			m_PackMap;

		CRITICAL_SECTION		m_csFinder;
};
