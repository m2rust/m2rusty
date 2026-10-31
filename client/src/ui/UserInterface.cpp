#include <Client/UI/StdAfx.h>
#include <Client/UI/PythonApplication.h>
#include <Client/UI/ProcessScanner.h>
#include <Client/UI/PythonExceptionSender.h>
#include <Client/UI/resource.h>
#include <Client/UI/Version.h>

#ifdef _DEBUG
#include <crtdbg.h>
#endif

#include <Neoeter/IO/EterPackManager.h>
#include <Neoeter/Render/Util.h>
#include <Client/WebBrowser/CWebBrowser.h>
#include <Neoeter/Platform/Base/CPostIt.h>

// Force link ImageLoaderCompat.cpp from libEterBase.a
extern bool Platform_GetImageInfo(const void* data, size_t size, uint32_t* outW, uint32_t* outH, uint32_t* outFormat);
extern bool Platform_DecodeImage(const void* data, size_t size, uint32_t* outW, uint32_t* outH, std::vector<uint8_t>& outPixels);
void* g_pForceLinkImageLoader[] = {
    (void*)&Platform_GetImageInfo,
    (void*)&Platform_DecodeImage
};

#ifndef OPENMT2_PLATFORM_LINUX
// T4.1: the DirectX SDK, Miles (mss32) and the SpeedTree import library are
// gone. The Windows libraries the client actually needs (opengl32, winmm,
// imagehlp, ws2_32) are linked by CMake, not by #pragma.
extern "C" {
#if defined(_MSC_VER)
	// _fltused is provided by the MSVC CRT; MinGW has no such symbol (T1.9).
	extern int32_t _fltused;
	volatile int32_t _AVOID_FLOATING_POINT_LIBRARY_BUG = _fltused;
#endif
	__declspec(dllexport) uint32_t NvOptimusEnablement = 0x00000001;
	__declspec(dllexport) int32_t AmdPowerXpressRequestHighPerformance = 1;
};
#endif
#include <stdlib.h>
bool __IS_TEST_SERVER_MODE__=false;

extern bool SetDefaultCodePage(DWORD codePage);

#ifdef USE_OPENID
extern int openid_test;
#endif

static const char* sc_apszPythonLibraryFilenames[] =

{

#if PY_MINOR_VERSION == 7
	"abc.pyc",
	"bisect.pyc",
	"codecs.pyc",
	"collections.pyc",
	"copy.pyc",
	"copy_reg.pyc",
	"fnmatch.pyc",
	"functools.pyc",
	"genericpath.pyc",
	"heapq.pyc",
	"hashlib.pyc", // new
	"keyword.pyc",
	"linecache.pyc",
	"locale.pyc",
	"ntpath.pyc",
	"os.pyc",
	// removed pyexpat
	"random.pyc",
	"re.pyc",
	"shutil.pyc",
	"site.pyc",
	"sre_compile.pyc",
	"sre_constants.pyc",
	"sre_parse.pyc",
	"stat.pyc",
	"string.pyc",
	"sysconfig.pyc",
	"traceback.pyc",
	"types.pyc",
	"UserDict.pyc",
	"warnings.pyc",
	"weakref.pyc",
	"_abcoll.pyc",
	"_weakrefset.pyc",
	"__future__.pyc",
	"encodings/aliases.pyc",
	"encodings/ascii.pyc",
	"encodings/cp1250.pyc",
	"encodings/cp1251.pyc",
	"encodings/cp1252.pyc",
	"encodings/cp949.pyc",
	"encodings/utf_8.pyc",
	"encodings/cp850.pyc", //new
	"encodings/__init__.pyc",
	// removed xml
#else
	"UserDict.pyc",
	"__future__.pyc",
	"copy_reg.pyc",
	"linecache.pyc",
	"ntpath.pyc",
	"os.pyc",
	"site.pyc",
	"stat.pyc",
	"string.pyc",
	"traceback.pyc",
	"types.pyc",
#endif

	"\n",

};

char gs_szErrorString[512] = "";

void ApplicationSetErrorString(const char* szErrorString)
{
	strcpy(gs_szErrorString, szErrorString);
}

bool CheckPythonLibraryFilenames()
{
	for (int i = 0; *sc_apszPythonLibraryFilenames[i] != '\n'; ++i)
	{
		std::string stFilename = "lib/";
		stFilename += sc_apszPythonLibraryFilenames[i];
		for (char& c : stFilename) {
			if (c == '\\') c = '/';
		}

		if (_access(stFilename.c_str(), 0) != 0)
		{
			return false;
		}

		MoveFile(stFilename.c_str(), stFilename.c_str());
	}

	return true;
}

////////////////////////////////////////////

int Setup(LPSTR lpCmdLine); // Internal function forward

bool PackInitialize(const std::string& packFolder)
{
	if (_access(packFolder.c_str(), 0) != 0)
		return true;

	// Initialize variables
	bool bPackFirst = true;
	std::string stFolder = packFolder + "/";
	std::string stFileName;

#ifdef _DISTRIBUTE
	Tracef("Info: Pack search mode set to archive-first.\n");

	// Set Index file name to production value
	stFileName = stFolder + "Index";
#else
	if (_access((stFolder + "Index.dev").c_str(), 0) == 0)
	{
		bPackFirst = false;
		Tracef("Info: Pack search mode set to file-first.\n");

		// Set Index file name to development value
		stFileName = stFolder + "Index.dev";
	}
	else
	{
		bPackFirst = true;
		Tracef("Info: Pack search mode set to archive-first (fallback).\n");

		// Set Index file name to production value
		stFileName = stFolder + "Index";
	}
#endif

	// Set up file reader
	CMappedFile file;
	LPCVOID pvData;

	if (!file.Create(stFileName.c_str(), &pvData, 0, 0))
	{
		LogBoxf("FATAL ERROR! File not exist: %s", stFileName.c_str());
		TraceError("FATAL ERROR! File not exist: %s", stFileName.c_str());
		return true;
	}

	CMemoryTextFileLoader TextLoader;
	TextLoader.Bind(file.Size(), pvData);

	// Set up archive manager
	CTextFileLoader::SetCacheMode();
	CEterPackManager::Instance().SetSearchMode(bPackFirst);

	CSoundData::SetPackMode(); // Miles 파일 콜백을 셋팅

	// Read lines and parse tokens
	std::string strPackName, strTexCachePackName;
	CTokenVector tokens = CTokenVector();

	for (DWORD i = 0; i < TextLoader.GetLineCount() - 1; i++)
	{
		// Split line into tokens
		TextLoader.SplitLineByTab(i, &tokens);

		// Check if the read number of tokens is valid
		if (tokens.size() != 2)
		{
			TraceError("Invalid number of tokens on line %d: expected %d, got %d!", i, 2, tokens.size());
			return false;
		}

		// Assign tokens into variables
		const std::string& c_rstName = tokens[0];
		const std::string& c_rstContainer = tokens[1];

		// Load pack
		strPackName = stFolder + c_rstName;
		CEterPackManager::Instance().RegisterPack(strPackName, c_rstContainer);

		// Try to load texture cache pack if it exists
		strTexCachePackName = strPackName + "_texcache";
		if (CEterPackManager::Instance().PackExists(strTexCachePackName, c_rstContainer)) {
			CEterPackManager::Instance().RegisterPack(strTexCachePackName, c_rstContainer);
		}
	}

	return true;
}

bool RunMainScript(CPythonLauncher& pyLauncher, const char* lpCmdLine)
{
	initpack();
	initdbg();
	initime();
	initgrp();
	initgrpImage();
	initgrpText();
	initwndMgr();
	/////////////////////////////////////////////
	initapp();
	initsystem();
	initchr();
	initchrmgr();
	initPlayer();
	initItem();
	initNonPlayer();
	initTrade();
	initChat();
	initTextTail();
	initnet();
	initMiniMap();
	initProfiler();
	initEvent();
	initeffect();
	initfly();
	initsnd();
	initeventmgr();
	initshop();
	initskill();
	initquest();
	initBackground();
	initMessenger();
	initsafebox();
	initguild();
	initServerStateChecker();

	// RegisterDebugFlag
	{
		std::string stRegisterDebugFlag;

#ifdef _DISTRIBUTE 
		stRegisterDebugFlag ="__DEBUG__ = 0";
#else
		stRegisterDebugFlag ="__DEBUG__ = 1"; 
#endif

		if (!pyLauncher.RunLine(stRegisterDebugFlag.c_str()))
		{
			TraceError("RegisterDebugFlag Error");
			return false;
		}
	}

	// RegisterCommandLine
	{
		std::string stRegisterCmdLine;

		const char * loginMark = "-cs";
		const char * loginMark_NonEncode = "-ncs";
		const char * seperator = " ";

		std::string stCmdLine;
		const int CmdSize = 3;
		std::vector<std::string> stVec;
		SplitLine(lpCmdLine,seperator,&stVec);
		if (CmdSize == stVec.size() && stVec[0]==loginMark)
		{
			char buf[MAX_PATH];	//TODO 아래 함수 string 형태로 수정
			base64_decode(stVec[2].c_str(),buf);
			stVec[2] = buf;
			string_join(seperator,stVec,&stCmdLine);
		}
		else if (CmdSize <= stVec.size() && stVec[0]==loginMark_NonEncode)
		{
			stVec[0] = loginMark;
			string_join(" ",stVec,&stCmdLine);
		}
		else
			stCmdLine = lpCmdLine;

		stRegisterCmdLine ="__COMMAND_LINE__ = ";
		stRegisterCmdLine+='"';
		stRegisterCmdLine+=stCmdLine;
		stRegisterCmdLine+='"';

		const CHAR* c_szRegisterCmdLine=stRegisterCmdLine.c_str();
		if (!pyLauncher.RunLine(c_szRegisterCmdLine))
		{
			TraceError("RegisterCommandLine Error");
			return false;
		}
	}
	{
		std::vector<std::string> stVec;
		SplitLine(lpCmdLine," " ,&stVec);

		if (stVec.size() != 0 && "--pause-before-create-window" == stVec[0])
		{
			system("pause");
		}
		if (!pyLauncher.RunFile("system.py"))
		{
			TraceError("RunMain Error");
			return false;
		}
	}

	return true;
}

bool Main(HINSTANCE hInstance, LPSTR lpCmdLine)
{
#ifdef LOCALE_SERVICE_YMIR
	extern bool g_isScreenShotKey;
	g_isScreenShotKey = true;
#endif

	DWORD dwRandSeed=time(NULL)+DWORD((uintptr_t)GetCurrentProcess());
	srandom(dwRandSeed);
	srand(random());

	SetLogLevel(1);

#ifdef LOCALE_SERVICE_VIETNAM_MILD
	extern BOOL USE_VIETNAM_CONVERT_WEAPON_VNUM;
	USE_VIETNAM_CONVERT_WEAPON_VNUM = true;
#endif

	if (_access("perf_game_update.txt", 0)==0)
	{
		DeleteFile("perf_game_update.txt");
	}

	if (_access("newpatch.exe", 0)==0)
	{		
		system("patchupdater.exe");
		return false;
	}
	Tracef("[UserInterface] Setting up environment...\n");
	if (!Setup(lpCmdLine))
		return false;

#if defined(_DEBUG) || defined(OPENMT2_PLATFORM_LINUX)
	OpenConsoleWindow();
	OpenLogFile(true); // true == uses syserr.txt and log.txt
#else
	OpenLogFile(false); // false == uses syserr.txt only
#endif

	Tracef("[UserInterface] Initializing pack system from 'pack'...\n");
	static CLZO				lzo;
	static CEterPackManager	EterPackManager;

	if (!PackInitialize("pack"))
	{
		LogBox("Pack Initialization failed. Check log.txt file..");
		return false;
	}
	Tracef("[UserInterface] Pack system initialized successfully.\n");

	Tracef("[UserInterface] Loading global locale configuration...\n");
	if(LocaleService_LoadGlobal(hInstance))
		SetDefaultCodePage(LocaleService_GetCodePage());

	Tracef("[UserInterface] Creating and initializing CPythonApplication...\n");
	CPythonApplication * app = new CPythonApplication;

	app->Initialize(hInstance);
	Tracef("[UserInterface] CPythonApplication initialized.\n");

	bool ret=false;
	{
		Tracef("[UserInterface] Creating CPythonLauncher...\n");
		CPythonLauncher pyLauncher;
		CPythonExceptionSender pyExceptionSender;
		SetExceptionSender(&pyExceptionSender);

		if (pyLauncher.Create())
		{
			Tracef("[UserInterface] Python runtime created. Launching main script (system.py)...\n");
			ret=RunMainScript(pyLauncher, lpCmdLine);	//게임 실행중엔 함수가 끝나지 않는다.
			Tracef("[UserInterface] RunMainScript exited with code %d.\n", ret);
		}
		else
		{
			TraceError("[UserInterface] Failed to initialize Python Launcher!");
		}

		//ProcessScanner_ReleaseQuitEvent();
		
		//게임 종료시.
		Tracef("[UserInterface] Cleaning up application...\n");
		app->Clear();

		timeEndPeriod(1);
		pyLauncher.Clear();
	}

	app->Destroy();
	delete app;
	Tracef("[UserInterface] Application cleanly terminated.\n");
	
	return ret;
}

HANDLE CreateMetin2GameMutex()
{
	SECURITY_ATTRIBUTES sa;
	ZeroMemory(&sa, sizeof(SECURITY_ATTRIBUTES));
	sa.nLength				= sizeof(sa);
	sa.lpSecurityDescriptor	= NULL;
	sa.bInheritHandle		= FALSE;

	return CreateMutex(&sa, FALSE, "Metin2GameMutex");
}

void DestroyMetin2GameMutex(HANDLE hMutex)
{
	if (hMutex)
	{
		ReleaseMutex(hMutex);
		hMutex = NULL;
	}
}

void __ErrorPythonLibraryIsNotExist()
{
	LogBoxf("FATAL ERROR!! Python Library file not exist!");
}

bool __IsTimeStampOption(LPSTR lpCmdLine)
{
	const char* TIMESTAMP = "/timestamp";
	return (strncmp(lpCmdLine, TIMESTAMP, strlen(TIMESTAMP))==0);
}

void __PrintTimeStamp()
{
#ifndef _MSC_VER
#define _MSC_VER 1900
#endif
#ifdef	_DEBUG
	if (__IS_TEST_SERVER_MODE__)
		LogBoxf("METIN2 BINARY TEST DEBUG VERSION %s  ( MS C++ %d Compiled )", __TIMESTAMP__, _MSC_VER);
	else
		LogBoxf("METIN2 BINARY DEBUG VERSION %s ( MS C++ %d Compiled )", __TIMESTAMP__, _MSC_VER);
	
#else
	if (__IS_TEST_SERVER_MODE__)
		LogBoxf("METIN2 BINARY TEST VERSION %s  ( MS C++ %d Compiled )", __TIMESTAMP__, _MSC_VER);
	else
		LogBoxf("METIN2 BINARY DISTRIBUTE VERSION %s ( MS C++ %d Compiled )", __TIMESTAMP__, _MSC_VER);			
#endif			
}

bool __IsLocaleOption(LPSTR lpCmdLine)
{
	return (strcmp(lpCmdLine, "--locale") == 0);
}

bool __IsLocaleVersion(LPSTR lpCmdLine)
{
	return (strcmp(lpCmdLine, "--perforce-revision") == 0);
}

#ifdef USE_OPENID
//2012.07.16 김용욱
//일본 OpenID 지원. 인증키 인자 추가
bool __IsOpenIDAuthKeyOption(LPSTR lpCmdLine)
{
	return (strcmp(lpCmdLine, "--openid-authkey") == 0);
}

bool __IsOpenIDTestOption(LPSTR lpCmdLine) //클라이언트에서 로그인이 가능하다.
{
	return (strcmp(lpCmdLine, "--openid-test") == 0);
}
#endif /* USE_OPENID */

int APIENTRY WinMain(HINSTANCE hInstance, HINSTANCE hPrevInstance, LPSTR lpCmdLine, int nCmdShow)
{
	if (strstr(lpCmdLine, "--hackshield") != 0)
		return 0;

#ifdef _DEBUG
	_CrtSetDbgFlag( _CRTDBG_ALLOC_MEM_DF | _CRTDBG_CHECK_CRT_DF | _CRTDBG_LEAK_CHECK_DF );
	//_CrtSetBreakAlloc( 110247 ); 
#endif

	ApplicationStringTable_Initialize(hInstance);

	LocaleService_LoadConfig("locale.cfg");
	SetDefaultCodePage(LocaleService_GetCodePage());	

	bool bQuit = false;
	bool bAuthKeyChecked = false;	//OpenID 버전에서 인증키가 들어왔는지 알기 위한 인자.
	int nArgc = 0;
	PCHAR* szArgv = CommandLineToArgv( lpCmdLine, &nArgc );

	for( int i=0; i < nArgc; i++ ) {
		if(szArgv[i] == 0)
			continue;
		if (__IsLocaleVersion(szArgv[i])) // #0000829: [M2EU] 버전 파일이 항상 생기지 않도록 수정 
		{
			char szModuleName[MAX_PATH];
			char szVersionPath[MAX_PATH];
			GetModuleFileName(NULL, szModuleName, sizeof(szModuleName));
			sprintf(szVersionPath, "%s.version", szModuleName);
			FILE* fp = fopen(szVersionPath, "wt");
			if (fp)
			{
				extern int METIN2_GET_VERSION();
				fprintf(fp, "r%d\n", METIN2_GET_VERSION());
				fclose(fp);
			}
			bQuit = true;
		} else if (__IsLocaleOption(szArgv[i]))
		{
			FILE* fp=fopen("locale.txt", "wt");
			fprintf(fp, "service[%s] code_page[%d]", 
				LocaleService_GetName(), LocaleService_GetCodePage());
			fclose(fp);
			bQuit = true;
		} else if (__IsTimeStampOption(szArgv[i]))
		{
			__PrintTimeStamp();
			bQuit = true;
		} else if ((strcmp(szArgv[i], "--force-set-locale") == 0))
		{
			// locale 설정엔 인자가 두 개 더 필요함 (로케일 명칭, 데이터 경로)
			if (nArgc <= i + 2)
			{
				MessageBox(NULL, "Invalid arguments", ApplicationStringTable_GetStringz(IDS_APP_NAME, "APP_NAME"), MB_ICONSTOP);
				goto Clean;
			}

			const char* localeName = szArgv[++i];
			const char* localePath = szArgv[++i];

			LocaleService_ForceSetLocale(localeName, localePath);
		}
#ifdef USE_OPENID
		else if (__IsOpenIDAuthKeyOption(szArgv[i]))	//2012.07.16 OpenID : 김용욱
		{
			// 인증키 설정엔 인자가 한 개 더 필요함 (인증키)
			if (nArgc <= i + 1)
			{
				MessageBox(NULL, "Invalid arguments", ApplicationStringTable_GetStringz(IDS_APP_NAME, "APP_NAME"), MB_ICONSTOP);
				goto Clean;
			}

			const char* authKey = szArgv[++i];

			//ongoing (2012.07.16)
			//인증키 저장하는 부분
			LocaleService_SetOpenIDAuthKey(authKey);

			bAuthKeyChecked = true;
		}
		else if (__IsOpenIDTestOption(szArgv[i]))
		{
			openid_test = 1;

		}
#endif /* USE_OPENID */
	}

#ifdef USE_OPENID
	//OpenID
	//OpenID 클라이언트의 경우인증키를 받아오지 않을 경우 (웹을 제외하고 실행 시) 클라이언트 종료.
	
	if (false == bAuthKeyChecked && !openid_test)
	{
		MessageBox(NULL, "Invalid execution", ApplicationStringTable_GetStringz(IDS_APP_NAME, "APP_NAME"), MB_ICONSTOP);
		goto Clean;
	}
#endif /* USE_OPENID */


	if(bQuit)
		goto Clean;

#if defined(NEEDED_COMMAND_ARGUMENT)
	// 옵션이 없으면 비정상 실행으로 간주, 프로그램 종료
	if (strstr(lpCmdLine, NEEDED_COMMAND_ARGUMENT) == 0) {
		MessageBox(NULL, ApplicationStringTable_GetStringz(IDS_ERR_MUST_LAUNCH_FROM_PATCHER, "ERR_MUST_LAUNCH_FROM_PATCHER"), ApplicationStringTable_GetStringz(IDS_APP_NAME, "APP_NAME"), MB_ICONSTOP);
			goto Clean;
	}
#endif

#if defined(NEEDED_COMMAND_CLIPBOARD)
	{
		CHAR szSecKey[256];
		CPostIt cPostIt( "VOLUME1" );

		if( cPostIt.Get( "SEC_KEY", szSecKey, sizeof(szSecKey) ) == FALSE ) {
			MessageBox(NULL, ApplicationStringTable_GetStringz(IDS_ERR_MUST_LAUNCH_FROM_PATCHER, "ERR_MUST_LAUNCH_FROM_PATCHER"), ApplicationStringTable_GetStringz(IDS_APP_NAME, "APP_NAME"), MB_ICONSTOP);
			goto Clean;
		}
		if( strstr(szSecKey, NEEDED_COMMAND_CLIPBOARD) == 0 ) {
			MessageBox(NULL, ApplicationStringTable_GetStringz(IDS_ERR_MUST_LAUNCH_FROM_PATCHER, "ERR_MUST_LAUNCH_FROM_PATCHER"), ApplicationStringTable_GetStringz(IDS_APP_NAME, "APP_NAME"), MB_ICONSTOP);
			goto Clean;
		}
		cPostIt.Empty();
	}
#endif

	WebBrowser_Startup(hInstance);

	if (!CheckPythonLibraryFilenames())
	{
		__ErrorPythonLibraryIsNotExist();
		goto Clean;
	}

	Main(hInstance, lpCmdLine);

	WebBrowser_Cleanup();

	::CoUninitialize();

	if(gs_szErrorString[0])
		MessageBox(NULL, gs_szErrorString, ApplicationStringTable_GetStringz(IDS_APP_NAME, "APP_NAME"), MB_ICONSTOP);

Clean:
	SAFE_FREE_GLOBAL(szArgv);

	return 0;
}

#ifdef OPENMT2_PLATFORM_LINUX
int main(int argc, char** argv)
{
	printf("====================================================\n");
	printf("[Metin2 Linux Client] Starting client (PID: %d)...\n", getpid());
	printf("====================================================\n");
	fflush(stdout);

	std::string cmdLine;
	for (int i = 1; i < argc; ++i) {
		if (i > 1) cmdLine += " ";
		cmdLine += argv[i];
	}
	int ret = WinMain(NULL, NULL, const_cast<char*>(cmdLine.c_str()), 1);

	printf("====================================================\n");
	printf("[Metin2 Linux Client] Process terminated with code: %d\n", ret);
	printf("====================================================\n");
	fflush(stdout);
	return ret;
}
#endif

static void GrannyError(granny_log_message_type Type,
						granny_log_message_origin Origin,
						char const *Error,
						void *UserData)
{
    TraceError("GRANNY: %s", Error);
}

int Setup(LPSTR lpCmdLine)
{
	/* 
	 *	타이머 정밀도를 올린다.
	 */
	TIMECAPS tc; 
	UINT wTimerRes; 

	if (timeGetDevCaps(&tc, sizeof(TIMECAPS)) != TIMERR_NOERROR) 
		return 0;

	wTimerRes = MINMAX(tc.wPeriodMin, 1, tc.wPeriodMax); 
	timeBeginPeriod(wTimerRes); 

	/*
	 *	그래니 에러 핸들링
	 */

	granny_log_callback Callback;
    Callback.Function = GrannyError;
    Callback.UserData = 0;
    GrannySetLogCallback(&Callback);
	return 1;
}
