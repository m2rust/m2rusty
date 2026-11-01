#ifndef __INC_YMIR_ETERLIB_FILELOADERTHREAD_H__
#define __INC_YMIR_ETERLIB_FILELOADERTHREAD_H__

#include <deque>
#include <memory>
#include <vector>
#include "Thread.h"
#include "Mutex.h"

class CFileLoaderThread
{
  public:
    using TData = struct SData
    {
        std::string fileName;
        std::shared_ptr<std::vector<char>> data;
    };

    CFileLoaderThread();
    ~CFileLoaderThread();

    int Create(void *arg);

    void Request(const std::string &c_rstFileName);
    bool Fetch(TData &data);
    void Shutdown();

  protected:
    static UINT CALLBACK EntryPoint(void *pThis);
    UINT Run(void *arg);

    void *Arg() const
    {
        return m_pArg;
    }
    void Arg(void *arg)
    {
        m_pArg = arg;
    }

    HANDLE m_hThread;

  private:
    void *m_pArg;
    unsigned m_uThreadID = 0;

  protected:
    UINT Setup();
    UINT Execute(void *pvArg);
    void Destroy();
    void Process();

  private:
    std::deque<TData> m_pRequestDeque;
    Mutex m_RequestMutex;

    std::deque<TData> m_pCompleteDeque;
    Mutex m_CompleteMutex;

    HANDLE m_hSemaphore = nullptr;
    int m_iRestSemCount = 0;
    bool m_bShutdowned = false;
};

#endif
