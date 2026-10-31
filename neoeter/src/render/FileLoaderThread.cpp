#include <Neoeter/Render/StdAfx.h>
#include <Neoeter/IO/EterPackManager.h>
#include <Neoeter/Render/FileLoaderThread.h>
#include <Neoeter/Render/ResourceManager.h>

CFileLoaderThread::CFileLoaderThread() : m_bShutdowned(false), m_pArg(NULL), m_hThread(NULL), m_uThreadID(0)
{
}

CFileLoaderThread::~CFileLoaderThread()
{
	Destroy();
}

int CFileLoaderThread::Create(void * arg)
{
	Arg(arg);
	m_hThread = (HANDLE) _beginthreadex(NULL, 0, EntryPoint, this, 0, &m_uThreadID);

	if (!m_hThread)
		return false;

	SetThreadPriority(m_hThread, THREAD_PRIORITY_NORMAL);
	return true;
}

UINT CFileLoaderThread::Run(void * arg)
{
	if (!Setup())
		return 0;

	return (Execute(arg));
}

/* Static */
UINT CALLBACK CFileLoaderThread::EntryPoint(void * pThis)
{
	CFileLoaderThread * pThread = (CFileLoaderThread *) pThis;
	return pThread->Run(pThread->Arg());
}

//////////////////////////////////////////////////////////////////////////
void CFileLoaderThread::Destroy()
{
	if (m_hSemaphore)
	{
		CloseHandle(m_hSemaphore);
		m_hSemaphore = NULL;
	}
}

UINT CFileLoaderThread::Setup()
{
	m_hSemaphore = CreateSemaphore(NULL,		// no security attributes
								   0,			// initial count
								   65535,		// maximum count
								   NULL);		// unnamed semaphore
	if (!m_hSemaphore)
		return 0;

	return 1;
}

void CFileLoaderThread::Shutdown()
{
	if (!m_hSemaphore)
		return;

	BOOL bRet;
	
	m_bShutdowned = true;

	do
	{
		bRet = ReleaseSemaphore(m_hSemaphore, 1, NULL);
	}
	while (!bRet);

	WaitForSingleObject(m_hThread, 10000);	// 쓰레드가 종료 되기를 10초 기다림
}

UINT CFileLoaderThread::Execute(void * /*pvArg*/)
{
	while (!m_bShutdowned)
	{
		DWORD dwWaitResult; 

		dwWaitResult = WaitForSingleObject(m_hSemaphore, INFINITE);

		if (m_bShutdowned)
			break;

		switch (dwWaitResult) 
		{ 
			case WAIT_OBJECT_0:
				{
					Process();
				}
				break;

			case WAIT_TIMEOUT:
				TraceError("CFileLoaderThread::Execute: Timeout occured while time-out interval is INIFITE");
				break;
		}
	}

	Destroy();
	return 1;
}

void CFileLoaderThread::Request(const std::string& c_rstFileName)	// called in main thread
{
	TData pData;
	pData.fileName = c_rstFileName;
	pData.data = nullptr;

	m_RequestMutex.Lock();
	m_pRequestDeque.push_back(pData);
	m_RequestMutex.Unlock();

	++m_iRestSemCount;

	if (!ReleaseSemaphore(m_hSemaphore, m_iRestSemCount, NULL))
		TraceError("CFileLoaderThread::Request: ReleaseSemaphore error");

	--m_iRestSemCount;
}

bool CFileLoaderThread::Fetch(TData& data)	// called in main thread
{
	m_CompleteMutex.Lock();

	if (m_pCompleteDeque.empty())
	{
		m_CompleteMutex.Unlock();
		return false;
	}

	data = m_pCompleteDeque.front();
	m_pCompleteDeque.pop_front();

	m_CompleteMutex.Unlock();
	return true;
}

void CFileLoaderThread::Process()	// called in loader thread
{
	m_RequestMutex.Lock();

	if (m_pRequestDeque.empty())
	{
		m_RequestMutex.Unlock();
		return;
	}

	auto request = m_pRequestDeque.front();
	m_pRequestDeque.pop_front();

	m_RequestMutex.Unlock();

	if (!CEterPackManager::Instance().Get(request.fileName, request.data))
		request.data = nullptr;

	m_CompleteMutex.Lock();
	m_pCompleteDeque.push_back(request);
	m_CompleteMutex.Unlock();

	Sleep(g_iLoadingDelayTime);
}
