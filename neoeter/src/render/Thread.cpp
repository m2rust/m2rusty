#include <Neoeter/Render/StdAfx.h>
#include <Neoeter/Render/Thread.h>

CThread::CThread() : m_pArg(nullptr), m_hThread(nullptr)
{
}

int CThread::Create(void *arg)
{
    Arg(arg);
    m_hThread = (HANDLE)_beginthreadex(nullptr, 0, EntryPoint, this, 0, &m_uThreadID);

    if (!m_hThread)
        return false;

    SetThreadPriority(m_hThread, THREAD_PRIORITY_NORMAL);
    return true;
}

UINT CThread::Run(void *arg)
{
    if (!Setup())
        return 0;

    return (Execute(arg));
}

/* Static */
UINT CALLBACK CThread::EntryPoint(void *pThis)
{
    auto *pThread = static_cast<CThread *>(pThis);
    return pThread->Run(pThread->Arg());
}