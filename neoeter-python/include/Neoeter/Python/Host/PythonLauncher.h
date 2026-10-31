#pragma once
#include <memory>
#include <vector>
#include <python2.7/frameobject.h>

#include <Neoeter/Platform/Base/Singleton.h>

class CPythonLauncher : public CSingleton<CPythonLauncher>
{
	public:
		CPythonLauncher();
		virtual ~CPythonLauncher();

		void Clear();

		bool Create(const char* c_szProgramName="eter.python");
		void SetTraceFunc(int (*pFunc)(PyObject * obj, PyFrameObject * f, int what, PyObject *arg));
		bool RunLine(const char* c_szLine);
		bool RunFile(const char* c_szFileName);
		bool RunMemoryTextFile(const char* c_szFileName, std::shared_ptr<std::vector<char>> fileData);
		bool RunCompiledFile(const char* c_szFileName);
		const char* GetError();

	protected:
		PyObject* m_poModule;
		PyObject* m_poDic;
};
