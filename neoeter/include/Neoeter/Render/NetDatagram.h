#pragma once

class CNetworkDatagram
{
  public:
    CNetworkDatagram();
    virtual ~CNetworkDatagram();

    void Destroy();
    bool Create(UINT uPort);

    void Update();

    bool CanRecv();

    int PeekRecvFrom(UINT uBufLen, void *pvBuf, SOCKADDR_IN *pkSockAddrIn) const;
    int RecvFrom(UINT uBufLen, void *pvBuf, SOCKADDR_IN *pkSockAddrIn) const;
    int SendTo(UINT uBufLen, const void *c_pvBuf, const SOCKADDR_IN &c_rkSockAddrIn) const;

  private:
    void __Initialize();

    SOCKET m_sock = 0;

    fd_set m_fdsRecv{};
    fd_set m_fdsSend{};
};