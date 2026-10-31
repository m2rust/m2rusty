#include <Neoeter/Python/Host/StdAfx.h>
#include <Neoeter/IO/EterPackManager.h>

#include <Neoeter/Python/Host/PythonLauncher.h>
#include <chrono>
#include <ctime>
#include <cmath>

#if !defined(_WIN32)
#include <dlfcn.h>
#include <cstdlib>
#include <string>
#include <vector>
#include <sys/stat.h>
#endif

static PyObject* py_time_time(PyObject* self, PyObject* args)
{
	auto now = std::chrono::system_clock::now().time_since_epoch();
	double s = std::chrono::duration<double>(now).count();
	return PyFloat_FromDouble(s);
}

static PyObject* py_time_clock(PyObject* self, PyObject* args)
{
	static auto start = std::chrono::steady_clock::now();
	auto now = std::chrono::steady_clock::now();
	double s = std::chrono::duration<double>(now - start).count();
	return PyFloat_FromDouble(s);
}

static PyObject* py_time_sleep(PyObject* self, PyObject* args)
{
	double secs;
	if (!PyArg_ParseTuple(args, "d", &secs))
		return NULL;
#if defined(_WIN32)
	// nanosleep is POSIX; the Windows build uses the Win32 equivalent (T1.9).
	Sleep((DWORD)(secs * 1000.0));
#else
	struct timespec ts;
	ts.tv_sec = (time_t)secs;
	ts.tv_nsec = (long)((secs - ts.tv_sec) * 1e9);
	nanosleep(&ts, NULL);
#endif
	Py_RETURN_NONE;
}

static PyObject* py_time_localtime(PyObject* self, PyObject* args)
{
	time_t t;
	double dt;
	if (args && PyTuple_Size(args) > 0) {
		if (!PyArg_ParseTuple(args, "d", &dt))
			return NULL;
		t = (time_t)dt;
	} else {
		t = ::time(NULL);
	}
	struct tm* tm_info = localtime(&t);
	if (!tm_info)
		return PyErr_SetFromErrno(PyExc_ValueError);
	return Py_BuildValue("(iiiiiiiii)",
		tm_info->tm_year + 1900,
		tm_info->tm_mon + 1,
		tm_info->tm_mday,
		tm_info->tm_hour,
		tm_info->tm_min,
		tm_info->tm_sec,
		(tm_info->tm_wday + 6) % 7,
		tm_info->tm_yday + 1,
		tm_info->tm_isdst
	);
}

static PyObject* py_time_strftime(PyObject* self, PyObject* args)
{
	const char* fmt;
	PyObject* tup = NULL;
	if (!PyArg_ParseTuple(args, "s|O", &fmt, &tup))
		return NULL;

	time_t t = ::time(NULL);
	struct tm* tm_info = localtime(&t);
	struct tm custom_tm;
	if (tup && PyTuple_Check(tup) && PyTuple_Size(tup) >= 9) {
		custom_tm.tm_year = PyInt_AsLong(PyTuple_GetItem(tup, 0)) - 1900;
		custom_tm.tm_mon  = PyInt_AsLong(PyTuple_GetItem(tup, 1)) - 1;
		custom_tm.tm_mday = PyInt_AsLong(PyTuple_GetItem(tup, 2));
		custom_tm.tm_hour = PyInt_AsLong(PyTuple_GetItem(tup, 3));
		custom_tm.tm_min  = PyInt_AsLong(PyTuple_GetItem(tup, 4));
		custom_tm.tm_sec  = PyInt_AsLong(PyTuple_GetItem(tup, 5));
		custom_tm.tm_wday = PyInt_AsLong(PyTuple_GetItem(tup, 6));
		custom_tm.tm_yday = PyInt_AsLong(PyTuple_GetItem(tup, 7)) - 1;
		custom_tm.tm_isdst= PyInt_AsLong(PyTuple_GetItem(tup, 8));
		tm_info = &custom_tm;
	}
	char buf[256];
	size_t len = strftime(buf, sizeof(buf), fmt, tm_info);
	return PyString_FromStringAndSize(buf, len);
}

static PyMethodDef s_time_methods[] = {
	{ "time", py_time_time, METH_NOARGS, "time() -> floating point number" },
	{ "clock", py_time_clock, METH_NOARGS, "clock() -> floating point number" },
	{ "sleep", py_time_sleep, METH_VARARGS, "sleep(seconds)" },
	{ "localtime", py_time_localtime, METH_VARARGS, "localtime([seconds]) -> tuple" },
	{ "strftime", py_time_strftime, METH_VARARGS, "strftime(format[, tuple]) -> string" },
	{ NULL, NULL, 0, NULL }
};

static void init_embedded_time()
{
	Py_InitModule("time", s_time_methods);
}

static PyObject* py_math_sin(PyObject* self, PyObject* args)
{
	double x;
	if (!PyArg_ParseTuple(args, "d:sin", &x))
		return NULL;
	return PyFloat_FromDouble(std::sin(x));
}

static PyObject* py_math_cos(PyObject* self, PyObject* args)
{
	double x;
	if (!PyArg_ParseTuple(args, "d:cos", &x))
		return NULL;
	return PyFloat_FromDouble(std::cos(x));
}

static PyObject* py_math_tan(PyObject* self, PyObject* args)
{
	double x;
	if (!PyArg_ParseTuple(args, "d:tan", &x))
		return NULL;
	return PyFloat_FromDouble(std::tan(x));
}

static PyObject* py_math_asin(PyObject* self, PyObject* args)
{
	double x;
	if (!PyArg_ParseTuple(args, "d:asin", &x))
		return NULL;
	return PyFloat_FromDouble(std::asin(x));
}

static PyObject* py_math_acos(PyObject* self, PyObject* args)
{
	double x;
	if (!PyArg_ParseTuple(args, "d:acos", &x))
		return NULL;
	return PyFloat_FromDouble(std::acos(x));
}

static PyObject* py_math_atan(PyObject* self, PyObject* args)
{
	double x;
	if (!PyArg_ParseTuple(args, "d:atan", &x))
		return NULL;
	return PyFloat_FromDouble(std::atan(x));
}

static PyObject* py_math_atan2(PyObject* self, PyObject* args)
{
	double y, x;
	if (!PyArg_ParseTuple(args, "dd:atan2", &y, &x))
		return NULL;
	return PyFloat_FromDouble(std::atan2(y, x));
}

static PyObject* py_math_sqrt(PyObject* self, PyObject* args)
{
	double x;
	if (!PyArg_ParseTuple(args, "d:sqrt", &x))
		return NULL;
	if (x < 0.0) {
		PyErr_SetString(PyExc_ValueError, "math domain error");
		return NULL;
	}
	return PyFloat_FromDouble(std::sqrt(x));
}

static PyObject* py_math_ceil(PyObject* self, PyObject* args)
{
	double x;
	if (!PyArg_ParseTuple(args, "d:ceil", &x))
		return NULL;
	return PyFloat_FromDouble(std::ceil(x));
}

static PyObject* py_math_floor(PyObject* self, PyObject* args)
{
	double x;
	if (!PyArg_ParseTuple(args, "d:floor", &x))
		return NULL;
	return PyFloat_FromDouble(std::floor(x));
}

static PyObject* py_math_fabs(PyObject* self, PyObject* args)
{
	double x;
	if (!PyArg_ParseTuple(args, "d:fabs", &x))
		return NULL;
	return PyFloat_FromDouble(std::fabs(x));
}

static PyObject* py_math_hypot(PyObject* self, PyObject* args)
{
	double x, y;
	if (!PyArg_ParseTuple(args, "dd:hypot", &x, &y))
		return NULL;
	return PyFloat_FromDouble(std::hypot(x, y));
}

static PyObject* py_math_pow(PyObject* self, PyObject* args)
{
	double x, y;
	if (!PyArg_ParseTuple(args, "dd:pow", &x, &y))
		return NULL;
	return PyFloat_FromDouble(std::pow(x, y));
}

static PyObject* py_math_exp(PyObject* self, PyObject* args)
{
	double x;
	if (!PyArg_ParseTuple(args, "d:exp", &x))
		return NULL;
	return PyFloat_FromDouble(std::exp(x));
}

static PyObject* py_math_log(PyObject* self, PyObject* args)
{
	double x;
	PyObject* baseObj = NULL;
	if (!PyArg_ParseTuple(args, "d|O:log", &x, &baseObj))
		return NULL;
	if (x <= 0.0) {
		PyErr_SetString(PyExc_ValueError, "math domain error");
		return NULL;
	}
	if (baseObj && baseObj != Py_None) {
		double base = PyFloat_AsDouble(baseObj);
		if (PyErr_Occurred())
			return NULL;
		if (base <= 0.0 || base == 1.0) {
			PyErr_SetString(PyExc_ValueError, "math domain error");
			return NULL;
		}
		return PyFloat_FromDouble(std::log(x) / std::log(base));
	}
	return PyFloat_FromDouble(std::log(x));
}

static PyObject* py_math_log10(PyObject* self, PyObject* args)
{
	double x;
	if (!PyArg_ParseTuple(args, "d:log10", &x))
		return NULL;
	if (x <= 0.0) {
		PyErr_SetString(PyExc_ValueError, "math domain error");
		return NULL;
	}
	return PyFloat_FromDouble(std::log10(x));
}

static PyObject* py_math_degrees(PyObject* self, PyObject* args)
{
	double x;
	if (!PyArg_ParseTuple(args, "d:degrees", &x))
		return NULL;
	return PyFloat_FromDouble(x * (180.0 / 3.14159265358979323846));
}

static PyObject* py_math_radians(PyObject* self, PyObject* args)
{
	double x;
	if (!PyArg_ParseTuple(args, "d:radians", &x))
		return NULL;
	return PyFloat_FromDouble(x * (3.14159265358979323846 / 180.0));
}

static PyObject* py_math_fmod(PyObject* self, PyObject* args)
{
	double x, y;
	if (!PyArg_ParseTuple(args, "dd:fmod", &x, &y))
		return NULL;
	if (y == 0.0) {
		PyErr_SetString(PyExc_ValueError, "math domain error");
		return NULL;
	}
	return PyFloat_FromDouble(std::fmod(x, y));
}

static PyObject* py_math_modf(PyObject* self, PyObject* args)
{
	double x;
	if (!PyArg_ParseTuple(args, "d:modf", &x))
		return NULL;
	double intpart;
	double fractpart = std::modf(x, &intpart);
	return Py_BuildValue("(dd)", fractpart, intpart);
}

static PyMethodDef s_math_methods[] = {
	{ "sin", py_math_sin, METH_VARARGS, "sin(x)" },
	{ "cos", py_math_cos, METH_VARARGS, "cos(x)" },
	{ "tan", py_math_tan, METH_VARARGS, "tan(x)" },
	{ "asin", py_math_asin, METH_VARARGS, "asin(x)" },
	{ "acos", py_math_acos, METH_VARARGS, "acos(x)" },
	{ "atan", py_math_atan, METH_VARARGS, "atan(x)" },
	{ "atan2", py_math_atan2, METH_VARARGS, "atan2(y, x)" },
	{ "sqrt", py_math_sqrt, METH_VARARGS, "sqrt(x)" },
	{ "ceil", py_math_ceil, METH_VARARGS, "ceil(x)" },
	{ "floor", py_math_floor, METH_VARARGS, "floor(x)" },
	{ "fabs", py_math_fabs, METH_VARARGS, "fabs(x)" },
	{ "hypot", py_math_hypot, METH_VARARGS, "hypot(x, y)" },
	{ "pow", py_math_pow, METH_VARARGS, "pow(x, y)" },
	{ "exp", py_math_exp, METH_VARARGS, "exp(x)" },
	{ "log", py_math_log, METH_VARARGS, "log(x[, base])" },
	{ "log10", py_math_log10, METH_VARARGS, "log10(x)" },
	{ "degrees", py_math_degrees, METH_VARARGS, "degrees(x)" },
	{ "radians", py_math_radians, METH_VARARGS, "radians(x)" },
	{ "fmod", py_math_fmod, METH_VARARGS, "fmod(x, y)" },
	{ "modf", py_math_modf, METH_VARARGS, "modf(x)" },
	{ NULL, NULL, 0, NULL }
};

static void init_embedded_math()
{
	PyObject* m = Py_InitModule("math", s_math_methods);
	if (m) {
		PyModule_AddObject(m, "pi", PyFloat_FromDouble(3.14159265358979323846));
		PyModule_AddObject(m, "e", PyFloat_FromDouble(2.71828182845904523536));
	}
}

CPythonLauncher::CPythonLauncher()
{
	Py_NoSiteFlag = 1;
#if defined(OPENMT2_PLATFORM_LINUX) || defined(UNIX)
	static char s_pythonHome[] = ".";
	Py_SetPythonHome(s_pythonHome);
#endif
	Py_Initialize();
}

CPythonLauncher::~CPythonLauncher()
{
	Clear();
}

void CPythonLauncher::Clear()
{
	Py_Finalize();
}

std::string g_stTraceBuffer[512];
int	g_nCurTraceN = 0;

void Traceback()
{
	std::string str;

	for (int i = 0; i < g_nCurTraceN; ++i)
	{
		str.append(g_stTraceBuffer[i]);
		str.append("\n");
	}
	
	PyObject * exc;
	PyObject * v;
	PyObject * tb;
	const char * errStr;

	PyErr_Fetch(&exc, &v, &tb);

	if (PyString_Check(v))
	{
		errStr = PyString_AS_STRING(v);
		str.append("Error: ");
		str.append(errStr);

		Tracef("%s\n", errStr);
	}
	Py_DECREF(exc);
	Py_DECREF(v);
	Py_DECREF(tb);
	LogBoxf("Traceback:\n\n%s\n", str.c_str());
}

int TraceFunc(PyObject * obj, PyFrameObject * f, int what, PyObject *arg)
{
	const char * funcname;
	char szTraceBuffer[128];

	switch (what)
	{
		case PyTrace_CALL:
			if (g_nCurTraceN >= 512)
				return 0;

			if (Py_OptimizeFlag)
				f->f_lineno = PyCode_Addr2Line(f->f_code, f->f_lasti);

			funcname = PyString_AsString(f->f_code->co_name);

			_snprintf(szTraceBuffer, sizeof(szTraceBuffer), "Call: File \"%s\", line %d, in %s", 
					  PyString_AsString(f->f_code->co_filename), 
					  f->f_lineno,
					  funcname);

			g_stTraceBuffer[g_nCurTraceN++]=szTraceBuffer;			
			break;

		case PyTrace_RETURN:
			if (g_nCurTraceN > 0)
				--g_nCurTraceN;
			break;

		case PyTrace_EXCEPTION:
			if (g_nCurTraceN >= 512)
				return 0;
			
			PyObject * exc_type, * exc_value, * exc_traceback;

			PyTuple_GetObject(arg, 0, &exc_type);
			PyTuple_GetObject(arg, 1, &exc_value);
			PyTuple_GetObject(arg, 2, &exc_traceback);

			Py_ssize_t len;
			const char * exc_str;
			PyObject_AsCharBuffer(exc_type, &exc_str, &len);
			
			_snprintf(szTraceBuffer, sizeof(szTraceBuffer), "Exception: File \"%s\", line %d, in %s", 
					  PyString_AS_STRING(f->f_code->co_filename), 
					  f->f_lineno,
					  PyString_AS_STRING(f->f_code->co_name));

			g_stTraceBuffer[g_nCurTraceN++]=szTraceBuffer;
			
			break;
	}
	return 0;
}

#if !defined(_WIN32)
static bool IsDirectory(const std::string& path)
{
	struct stat st;
	return stat(path.c_str(), &st) == 0 && S_ISDIR(st.st_mode);
}

static bool IsPythonStdlib(const std::string& dir)
{
	struct stat st;
	return !dir.empty() &&
		(stat((dir + "/os.py").c_str(), &st) == 0 || stat((dir + "/os.pyc").c_str(), &st) == 0);
}

// Py_SetPythonHome(".") hides the host's standard library, and bin/lib only
// carries the modules the Windows client needed (no posixpath, ...). Find the
// real Python 2.7 stdlib so anything missing from lib falls back to it.
static std::string FindPythonStdlib()
{
	std::vector<std::string> candidates;

	if (const char* env = getenv("OPENMT2_PYTHON_STDLIB"))
		candidates.push_back(env);
	if (const char* env = getenv("PYTHONHOME"))
		candidates.push_back(std::string(env) + "/lib/python2.7");

	// Next to the libpython that is actually loaded.
	Dl_info info;
	if (dladdr((void*)&Py_Initialize, &info) && info.dli_fname) {
		std::string lib = info.dli_fname;
		std::string::size_type slash = lib.rfind('/');
		if (slash != std::string::npos) {
			std::string libDir = lib.substr(0, slash);
			candidates.push_back(libDir + "/python2.7");
			// Debian: /usr/lib/x86_64-linux-gnu/libpython2.7.so -> /usr/lib/python2.7
			std::string::size_type parent = libDir.rfind('/');
			if (parent != std::string::npos)
				candidates.push_back(libDir.substr(0, parent) + "/python2.7");
		}
	}

#ifdef OPENMT2_PYTHON_STDLIB_BUILD
	candidates.push_back(OPENMT2_PYTHON_STDLIB_BUILD);
#endif
	candidates.push_back("/usr/lib/python2.7");
	candidates.push_back("/usr/lib64/python2.7");
	candidates.push_back("/usr/local/lib/python2.7");

	for (const std::string& dir : candidates)
		if (IsPythonStdlib(dir))
			return dir;

	return std::string();
}

static void AppendSysPath(const std::string& dir)
{
	if (!IsDirectory(dir))
		return;

	PyObject* path = PySys_GetObject((char*)"path");
	if (!path || !PyList_Check(path))
		return;

	PyObject* item = PyString_FromString(dir.c_str());
	if (item) {
		PyList_Append(path, item);
		Py_DECREF(item);
	}
}
#endif

void CPythonLauncher::SetTraceFunc(int (*pFunc)(PyObject * obj, PyFrameObject * f, int what, PyObject *arg))
{
	PyEval_SetTrace(pFunc, NULL);
}

bool CPythonLauncher::Create(const char* c_szProgramName)
{
	Py_SetProgramName((char*)c_szProgramName);
#ifdef _DEBUG
	PyEval_SetTrace(TraceFunc, NULL);
#endif
	m_poModule = PyImport_AddModule((char *) "__main__");

	if (!m_poModule)
		return false;
	
	m_poDic = PyModule_GetDict(m_poModule);

    PyObject * builtins = PyImport_ImportModule("__builtin__");
	PyModule_AddIntConstant(builtins, "TRUE", 1);
	PyModule_AddIntConstant(builtins, "FALSE", 0);
    PyDict_SetItemString(m_poDic, "__builtins__", builtins);
	Py_DECREF(builtins);

	init_embedded_time();
	init_embedded_math();

	if (!RunLine("import __main__"))
		return false;
	
	if (!RunLine("import sys"))
		return false;

	RunLine("sys.path.insert(0, 'lib')");

#if !defined(_WIN32)
	// After lib, so the modules shipped with the client keep priority.
	std::string stdlib = FindPythonStdlib();
	if (!stdlib.empty()) {
		Tracef("Python standard library: %s\n", stdlib.c_str());
		AppendSysPath(stdlib);
		AppendSysPath(stdlib + "/plat-linux2");
		AppendSysPath(stdlib + "/lib-dynload");
	} else {
		TraceError("Python 2.7 standard library not found; set OPENMT2_PYTHON_STDLIB");
	}
#endif

	return true;
}

bool CPythonLauncher::RunCompiledFile(const char* c_szFileName)
{
	FILE * fp = fopen(c_szFileName, "rb");

	if (!fp)
		return false;

	PyCodeObject *co;
	PyObject *v;
	long magic;
	long PyImport_GetMagicNumber(void);

	magic = _PyMarshal_ReadLongFromFile(fp);

	if (magic != PyImport_GetMagicNumber())
	{
		PyErr_SetString(PyExc_RuntimeError, "Bad magic number in .pyc file");
		fclose(fp);
		return false;
	}

	_PyMarshal_ReadLongFromFile(fp);
	v = _PyMarshal_ReadLastObjectFromFile(fp);

	fclose(fp);

	if (!v || !PyCode_Check(v))
	{
		Py_XDECREF(v);
		PyErr_SetString(PyExc_RuntimeError, "Bad code object in .pyc file");
		return false;
	}

	co = (PyCodeObject *) v;
	v = PyEval_EvalCode(co, m_poDic, m_poDic);
/*	if (v && flags)
		flags->cf_flags |= (co->co_flags & PyCF_MASK);*/
	Py_DECREF(co);
	if (!v)
	{
		Traceback();
		return false;
	}

	Py_DECREF(v);
	if (Py_FlushLine()) 
		PyErr_Clear();

	return true;
}


bool CPythonLauncher::RunMemoryTextFile(const char* c_szFileName, std::shared_ptr<std::vector<char>> fileData)
{
	std::string stConvFileData;
	stConvFileData.reserve(fileData->size());
	stConvFileData+="exec(compile('''";

	// ConvertPythonTextFormat
	{
		for (UINT i=0; i<fileData->size(); ++i)
		{
			if ((*fileData)[i] != 13)
				stConvFileData+= (*fileData)[i];
		}
	}

	stConvFileData+= "''', ";
	stConvFileData+= "'";
	stConvFileData+= c_szFileName;
	stConvFileData+= "', ";
	stConvFileData+= "'exec'))";

	const CHAR* c_pcConvFileData=stConvFileData.c_str();
	return RunLine(c_pcConvFileData);
}

bool CPythonLauncher::RunFile(const char* c_szFileName)
{
	CEterPackManager::TPackDataPtr data;
	if (!CEterPackManager::Instance().Get(c_szFileName, data))
		return false;
		
	if (data->size() == 0)
		return false;
		
	return RunMemoryTextFile(c_szFileName, data);
}

bool CPythonLauncher::RunLine(const char* c_szSrc)
{
	PyObject * v = PyRun_String((char *) c_szSrc, Py_file_input, m_poDic, m_poDic);

	if (!v)
	{
		Traceback();
		return false;
	}

	Py_DECREF(v);
	return true;
}

const char* CPythonLauncher::GetError()
{
	PyObject* exc;
	PyObject* v;
	PyObject* tb;

	PyErr_Fetch(&exc, &v, &tb);        

	if (PyString_Check(v))
		return PyString_AS_STRING(v);
	
	return "";
}
