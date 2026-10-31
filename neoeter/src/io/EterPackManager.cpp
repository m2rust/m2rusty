#include <Neoeter/IO/StdAfx.h>

#include <fstream>
#include <iterator>
#include <chrono>
#include <cstdio>
#include <cassert>
#include <filesystem>
#include <algorithm>

#if defined(_WIN32)
#include <io.h>   // _access
#endif

namespace fs = std::filesystem;

static std::string FindCaseInsensitiveFile(const std::string& path)
{
	if (_access(path.c_str(), 0) == 0)
		return path;

	std::error_code ec;
	fs::path p(path);
	fs::path parent = p.parent_path();
	if (parent.empty()) parent = ".";
	std::string filename = p.filename().string();
	std::string lowerFilename = filename;
	std::transform(lowerFilename.begin(), lowerFilename.end(), lowerFilename.begin(), ::tolower);

	if (fs::is_directory(parent, ec)) {
		for (const auto& entry : fs::directory_iterator(parent, ec)) {
			std::string entryName = entry.path().filename().string();
			std::string entryLower = entryName;
			std::transform(entryLower.begin(), entryLower.end(), entryLower.begin(), ::tolower);
			if (entryLower == lowerFilename) {
				return (parent / entryName).string();
			}
		}
	}
	return "";
}

// Every sibling directory whose name matches `path` ignoring case. Assets
// extracted from Windows packs can end up split across e.g. ETC/ and etc/,
// which Windows treats as one folder.
static std::vector<std::string> FindCaseInsensitiveDirs(const std::string& path)
{
	std::vector<std::string> result;

	std::error_code ec;
	fs::path p(path);
	fs::path parent = p.parent_path();
	if (parent.empty()) parent = ".";
	std::string lowerFilename = p.filename().string();
	std::transform(lowerFilename.begin(), lowerFilename.end(), lowerFilename.begin(), ::tolower);

	if (fs::is_directory(parent, ec)) {
		for (const auto& entry : fs::directory_iterator(parent, ec)) {
			if (!entry.is_directory(ec))
				continue;
			std::string entryName = entry.path().filename().string();
			std::string entryLower = entryName;
			std::transform(entryLower.begin(), entryLower.end(), entryLower.begin(), ::tolower);
			if (entryLower == lowerFilename)
				result.push_back((parent / entryName).string());
		}
	}

	// Register the exact-case folder last so its files win on conflicts.
	std::stable_partition(result.begin(), result.end(),
		[&](const std::string& dir) { return fs::path(dir).filename() != p.filename(); });
	return result;
}

#include <Neoeter/IO/EterPackManager.h>

#if __has_include(<Neoeter/Platform/Base/Debug.h>)
    #include <Neoeter/Platform/Base/Debug.h>
    #include <Neoeter/Platform/Base/CRC32.h>
    #include <Neoeter/Platform/Base/Timer.h>
    #ifndef timeGetTime
        #define timeGetTime ELTimer_GetMSec
    #endif
#else
    #define TraceError(...) do { fprintf(stderr, __VA_ARGS__); fprintf(stderr, "\n"); } while(0)
    #define Tracef(...) do { fprintf(stdout, __VA_ARGS__); } while(0)
    // Platform.h (pulled in by StdAfx.h) already provides timeGetTime().
#endif

#include <Neoeter/IO/Folder.h>
#include <Neoeter/IO/ZIP.h>

#define PATH_ABSOLUTE_YMIRWORK1	"d:/ymir work/"
#define PATH_ABSOLUTE_YMIRWORK2	"d:\\ymir work\\"

static std::string NormalizeSeparators(const std::string& path)
{
	std::string res = path;
	for (char& ch : res)
	{
		if (ch == '\\')
			ch = '/';
	}
	return res;
}

// StringPath std::string 버전
std::string CEterPackManager::ConvertFileName(std::string fileName)
{
	std::string convertedFileName = fileName;
	stl_lowers(convertedFileName);

	for (size_t i = 0; i < convertedFileName.length(); i++)
	{
		if (convertedFileName[i] == '\\')
			convertedFileName[i] = '/';
	}

	return convertedFileName;
}

bool CEterPackManager::CompareName(const char * c_szDirectoryName, DWORD /*dwLength*/, const char * c_szFileName)
{
	const char * c_pszSrc = c_szDirectoryName;
	const char * c_pszCmp = c_szFileName;

	while (*c_pszSrc)
	{
		if (*(c_pszSrc++) != *(c_pszCmp++))
			return false;

		if (!*c_pszCmp)
			return false;
	}

	return true;
}

struct TimeChecker
{
	TimeChecker(const char* name) : name(name)
	{
		baseTime = timeGetTime();
	}
	~TimeChecker()
	{
		printf("load %s (%u)\n", name, timeGetTime() - baseTime);
	}

	const char* name;
	DWORD baseTime;
};

bool CEterPackManager::Get(const std::string& fileName, TPackDataPtr& dataPtr)
{
	if (m_iSearchMode == SEARCH_FILE_FIRST)
	{
		if (GetFromFile(fileName, dataPtr))
			return true;

		return GetFromPack(fileName, dataPtr);
	}
	
	if (GetFromPack(fileName, dataPtr))
		return true;

	return GetFromFile(fileName, dataPtr);
}


bool CEterPackManager::Get(const std::string& fileName, std::stringstream& dataStream)
{
	CEterPackManager::TPackDataPtr data;
	if (!Get(fileName, data))
		return false;

	// Copy the data from the pack into the file sstream
	dataStream.str("");
	dataStream.clear();
	std::copy(data->begin(), data->end(), std::ostream_iterator<char>(dataStream));
	
	return true;
}

struct FinderLock
{
	FinderLock(CRITICAL_SECTION& cs) : p_cs(&cs)
	{
		EnterCriticalSection(p_cs);
	}

	~FinderLock()
	{
		LeaveCriticalSection(p_cs);
	}

	CRITICAL_SECTION* p_cs;
};

bool CEterPackManager::GetFromPack(const std::string& fileName, TPackDataPtr& dataPtr)
{
	FinderLock lock(m_csFinder);

	std::string strFileName = ConvertFileName(fileName);

	auto pkFileItem = m_FileMap.find(strFileName);

	if (pkFileItem == m_FileMap.end()) {
#ifdef _DEBUG
		TraceError("CANNOT_FIND_PACK_FILE [%s]", strFileName.c_str());
#endif

		return false;
	}

	auto data = std::make_shared<std::vector<char>>();
	if (!pkFileItem->second->getFile(strFileName, data))
		return false;

	// Set dataPtr to the retrieved data pointer
	dataPtr = data;

	return true;
}

bool CEterPackManager::GetFromFile(const std::string& fileName, TPackDataPtr& dataPtr)
{
	std::string normalizedPath = NormalizeSeparators(fileName);

	// Try to open the file with exact case first, then normalized lowercase
	std::ifstream file(normalizedPath, std::ios::binary);
	if (!file.is_open())
	{
		std::string lowerPath = ConvertFileName(fileName);
		file.open(lowerPath, std::ios::binary);
		if (!file.is_open())
			return false;
	}

	// Read the file's contents
	dataPtr = std::make_shared<std::vector<char>>(
		std::istreambuf_iterator<char>(file),
		std::istreambuf_iterator<char>()
	);
	return true;
}

bool CEterPackManager::isExistInPack(const char * c_szFileName)
{
	std::string strFileName = ConvertFileName(c_szFileName);

	auto pkFileItem = m_FileMap.find(strFileName);

	if (pkFileItem == m_FileMap.end())
		return false;

	if (!pkFileItem->second)
		return false;

	return pkFileItem->second->fileExists(strFileName.c_str());
}

bool CEterPackManager::isExist(const char * c_szFileName)
{
	std::string normPath = NormalizeSeparators(c_szFileName);
	std::string lowerPath = ConvertFileName(c_szFileName);

	if (m_iSearchMode == SEARCH_PACK_FIRST)
	{
		if (isExistInPack(c_szFileName))
			return true;

		return (_access(normPath.c_str(), 0) == 0) || (_access(lowerPath.c_str(), 0) == 0);
	}

	if ((_access(normPath.c_str(), 0) == 0) || (_access(lowerPath.c_str(), 0) == 0))
		return true;

	return isExistInPack(c_szFileName);
}

bool CEterPackManager::PackExists(const std::string& name, const std::string& container)
{
	std::string normName = NormalizeSeparators(name);
	if (container == "FOLDER") {
		std::string realFolder = FindCaseInsensitiveFile(normName);
		return !realFolder.empty();
	}
	else if (container == "ZIP") {
		std::string realZip = FindCaseInsensitiveFile(normName + ".zip");
		return !realZip.empty();
	}

	throw std::runtime_error("Unexpected container type: " + container + "!");
}

bool CEterPackManager::RegisterPack(const std::string& name, const std::string& container)
{
	auto it = m_PackMap.find(name);

	if (it != m_PackMap.end())
		return true;

	try {
		std::string normName = NormalizeSeparators(name);
		std::shared_ptr<FileProvider> pack;

		// Determine requested container type
		if (container == "FOLDER") {
			std::vector<std::string> realFolders = FindCaseInsensitiveDirs(normName);
			if (realFolders.empty()) realFolders.push_back(normName);

			for (auto const& realFolder : realFolders) {
				pack = std::make_shared<Folder>(realFolder);
				for (auto const& fileName : pack->listFiles())
					m_FileMap[fileName] = pack;
			}

			m_PackMap.insert({ name, pack });
			return true;
		}
		else if (container == "ZIP") {
			std::string realZip = FindCaseInsensitiveFile(normName + ".zip");
			if (realZip.empty()) realZip = normName + ".zip";
			pack = std::make_shared<ZIP>(realZip);
		}
		else
			throw std::runtime_error("Unexpected container type: " + container + "!");

		// Load container data
		auto packFiles = pack->listFiles();

		for (auto const& fileName : packFiles)
			m_FileMap[fileName] = pack;

		m_PackMap.insert({ name, pack });

		return true;
	}
	catch (std::exception& e)
	{
#ifdef _DEBUG
		Tracef("Unable to load file provider '%s': %s\n", name.c_str(), e.what());
#else
		(void)e;
#endif

		return false;
	}
}

void CEterPackManager::SetSearchMode(bool bPackFirst)
{
	m_iSearchMode = bPackFirst ? SEARCH_PACK_FIRST : SEARCH_FILE_FIRST;
}

int CEterPackManager::GetSearchMode()
{
	return m_iSearchMode;
}

CEterPackManager::CEterPackManager() : m_iSearchMode(SEARCH_FILE_FIRST)
{
	InitializeCriticalSection(&m_csFinder);
}

CEterPackManager::~CEterPackManager()
{
	DeleteCriticalSection(&m_csFinder);
}

const CEterPackManager::TFileMap& CEterPackManager::GetFileMap() {
	return this->m_FileMap;
}
