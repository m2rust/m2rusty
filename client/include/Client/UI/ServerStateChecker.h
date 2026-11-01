#pragma once

#include <Neoeter/Render/NetStream.h>

class CServerStateChecker : public CSingleton<CServerStateChecker>
{
  public:
    CServerStateChecker();
    ~CServerStateChecker() override;

    void Create(PyObject *poWnd);
    void AddChannel(UINT uServerIndex, const char *c_szAddr, UINT uPort);
    void Request();
    void Update();

    void Initialize();

  private:
    using TChannel = struct SChannel
    {
        UINT uServerIndex;
        const char *c_szAddr;
        UINT uPort;
    };

    PyObject *m_poWnd = nullptr;

    std::list<TChannel> m_lstChannel;

    CNetworkStream m_kStream;
};