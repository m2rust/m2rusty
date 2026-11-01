#pragma once

#ifndef VC_EXTRALEAN

class CNetDatagramReceiver
{
  public:
    CNetDatagramReceiver();
    virtual ~CNetDatagramReceiver();

    void Clear();
    BOOL Bind(DWORD dwAddress, WORD wPortIndex);
    BOOL isBind() const;

    BOOL Process();
    BOOL Recv(void *pBuffer, int iSize);
    BOOL Peek(void *pBuffer, int iSize);

    void SetRecvBufferSize(int recvBufSize);

  protected:
    BOOL m_isBind = 0;

    DWORD m_dwPortIndex = 0U;

    SOCKET m_Socket = 0;
    SOCKADDR_IN m_SockAddr{};

    int m_recvBufCurrentPos = 0;
    int m_recvBufCurrentSize = 0;

    char *m_recvBuf;
    int m_recvBufSize;
};

#endif