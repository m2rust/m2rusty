#include <Neoeter/Platform/Base/StdAfx.h>
#include <Neoeter/Platform/Base/error.h>

#if defined(_WIN32)

#include <stdio.h>
#include <time.h>
#include <winsock.h>
#include <imagehlp.h>

FILE * fException;

BOOL CALLBACK EnumerateLoadedModulesProc(PCSTR ModuleName, ULONG ModuleBase, ULONG ModuleSize, PVOID UserContext)
{
	DWORD offset = *((DWORD*)UserContext);
	
	if (offset >= ModuleBase && offset <= ModuleBase + ModuleSize)
	{
		fprintf(fException, "%s", ModuleName);
		return FALSE;
	}
	else
		return TRUE;
}

#if defined(_WIN64)
// 64-bit builds get the DWORD64 variant of the module walk (D10).
BOOL CALLBACK EnumerateLoadedModulesProc64(PCSTR ModuleName, DWORD64 ModuleBase, DWORD64 ModuleSize, PVOID UserContext)
{
	DWORD64 offset = *((DWORD64*)UserContext);

	if (offset >= ModuleBase && offset <= ModuleBase + ModuleSize)
	{
		fprintf(fException, "%s", ModuleName);
		return FALSE;
	}
	return TRUE;
}
#endif

LONG __stdcall EterExceptionFilter(_EXCEPTION_POINTERS* pExceptionInfo)
{
	HANDLE		hProcess	= GetCurrentProcess();
	HANDLE		hThread		= GetCurrentThread();
	
	fException = fopen("ErrorLog.txt", "wt");
	if (fException)
	{
		char module_name[256];
		time_t module_time;
		
		HMODULE hModule = GetModuleHandle(NULL);
		
		GetModuleFileName(hModule, module_name, sizeof(module_name));
		module_time = (time_t)GetTimestampForLoadedLibrary(hModule);
		
		fprintf(fException, "Module Name: %s\n", module_name);
		fprintf(fException, "Time Stamp: %lld - %s\n", (long long)module_time, ctime(&module_time));
		fprintf(fException, "\n");
		fprintf(fException, "Exception Type: 0x%08x\n", pExceptionInfo->ExceptionRecord->ExceptionCode);
		fprintf(fException, "\n");
		
		CONTEXT&	context		= *pExceptionInfo->ContextRecord;
		
#if defined(_WIN64)
		fprintf(fException, "rax: 0x%016llx\trbx: 0x%016llx\n", (unsigned long long)context.Rax, (unsigned long long)context.Rbx);
		fprintf(fException, "rcx: 0x%016llx\trdx: 0x%016llx\n", (unsigned long long)context.Rcx, (unsigned long long)context.Rdx);
		fprintf(fException, "rsi: 0x%016llx\trdi: 0x%016llx\n", (unsigned long long)context.Rsi, (unsigned long long)context.Rdi);
		fprintf(fException, "rbp: 0x%016llx\trsp: 0x%016llx\n", (unsigned long long)context.Rbp, (unsigned long long)context.Rsp);
		fprintf(fException, "\n");

		STACKFRAME64 stackFrame = {0,};
		stackFrame.AddrPC.Offset	= context.Rip;
		stackFrame.AddrPC.Mode		= AddrModeFlat;
		stackFrame.AddrStack.Offset	= context.Rsp;
		stackFrame.AddrStack.Mode	= AddrModeFlat;
		stackFrame.AddrFrame.Offset	= context.Rbp;
		stackFrame.AddrFrame.Mode	= AddrModeFlat;

		for (int i=0; i < 512 && stackFrame.AddrPC.Offset; ++i)
		{
			if (StackWalk64(IMAGE_FILE_MACHINE_AMD64, hProcess, hThread, &stackFrame, &context,
			                NULL, SymFunctionTableAccess64, SymGetModuleBase64, NULL) != FALSE)
			{
				fprintf(fException, "0x%016llx\t", (unsigned long long)stackFrame.AddrPC.Offset);
				EnumerateLoadedModules64(hProcess, (PENUMLOADED_MODULES_CALLBACK64) EnumerateLoadedModulesProc64, &stackFrame.AddrPC.Offset);
				fprintf(fException, "\n");
			}
			else
			{
				break;
			}
		}
#else
		fprintf(fException, "eax: 0x%08x\tebx: 0x%08x\n", context.Eax, context.Ebx);
		fprintf(fException, "ecx: 0x%08x\tedx: 0x%08x\n", context.Ecx, context.Edx);
		fprintf(fException, "esi: 0x%08x\tedi: 0x%08x\n", context.Esi, context.Edi);
		fprintf(fException, "ebp: 0x%08x\tesp: 0x%08x\n", context.Ebp, context.Esp);
		fprintf(fException, "\n");
		
		STACKFRAME stackFrame = {0,};
		stackFrame.AddrPC.Offset	= context.Eip;
		stackFrame.AddrPC.Mode		= AddrModeFlat;
		stackFrame.AddrStack.Offset	= context.Esp;
		stackFrame.AddrStack.Mode	= AddrModeFlat;
		stackFrame.AddrFrame.Offset	= context.Ebp;
		stackFrame.AddrFrame.Mode	= AddrModeFlat;
		
		for (int i=0; i < 512 && stackFrame.AddrPC.Offset; ++i)
		{
			if (StackWalk(IMAGE_FILE_MACHINE_I386, hProcess, hThread, &stackFrame, &context, NULL, NULL, NULL, NULL) != FALSE)
			{
				fprintf(fException, "0x%08x\t", stackFrame.AddrPC.Offset);
				EnumerateLoadedModules(hProcess, (PENUMLOADED_MODULES_CALLBACK) EnumerateLoadedModulesProc, &stackFrame.AddrPC.Offset);
				fprintf(fException, "\n");
			}
			else
			{
				break;
			}
		}
#endif 
		
		fprintf(fException, "\n");

		fflush(fException);

		fclose(fException);
		fException = NULL;
		
		MessageBox(NULL, "A fatal error was encountered while running the game.\nAn error log was saved to ErrorLog.txt.\nPlease consider reporting this issue.", "Error!", MB_OK);
	}
	
	return EXCEPTION_EXECUTE_HANDLER;
}

void SetEterExceptionHandler()
{
	SetUnhandledExceptionFilter(EterExceptionFilter);
}

#else

#include <csignal>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <unistd.h>
#include <execinfo.h>

static void PosixSignalHandler(int sig)
{
	FILE* f = fopen("ErrorLog.txt", "wt");
	if (f)
	{
		fprintf(f, "Fatal Signal: %d\n", sig);
		void* buffer[64];
		int nptrs = backtrace(buffer, 64);
		char** strings = backtrace_symbols(buffer, nptrs);
		if (strings)
		{
			for (int i = 0; i < nptrs; ++i)
				fprintf(f, "%s\n", strings[i]);
			free(strings);
		}
		fclose(f);
	}
	_exit(1);
}

void SetEterExceptionHandler()
{
	struct sigaction sa;
	memset(&sa, 0, sizeof(sa));
	sa.sa_handler = PosixSignalHandler;
	sigaction(SIGSEGV, &sa, NULL);
	sigaction(SIGABRT, &sa, NULL);
	sigaction(SIGFPE, &sa, NULL);
	sigaction(SIGILL, &sa, NULL);
}

#endif
