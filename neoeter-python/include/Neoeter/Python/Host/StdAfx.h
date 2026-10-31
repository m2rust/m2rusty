#pragma once

#include <Platform/Platform.h>
#include <Neoeter/Render/StdAfx.h>
#include <Neoeter/Model/Grn/StdAfx.h>

#include <Python.h>  // T1.9: the include dir selects the platform copy

#include <Neoeter/Python/Host/PythonUtils.h>
#include <Neoeter/Python/Host/PythonLauncher.h>
#include <Neoeter/Python/Host/PythonMarshal.h>
#include <Neoeter/Python/Host/Resource.h>

void initdbg();

// PYTHON_EXCEPTION_SENDER
class IPythonExceptionSender
{
	public:
		void Clear()
		{
			m_strExceptionString = "";
		}

		void RegisterExceptionString(const char * c_szString)
		{
			m_strExceptionString += c_szString;
		}

		virtual void Send() = 0;

	protected:
		std::string m_strExceptionString;
};

extern IPythonExceptionSender * g_pkExceptionSender;

void SetExceptionSender(IPythonExceptionSender * pkExceptionSender);
// END_OF_PYTHON_EXCEPTION_SENDER
