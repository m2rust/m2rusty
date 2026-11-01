#include <Neoeter/Render/StdAfx.h>
#include <Neoeter/Render/NetDatagramSender.h>

BOOL CNetDatagramSender::SetSocket(const char *c_szIP, WORD wPortIndex)
{
    return SetSocket(inet_addr(c_szIP), wPortIndex);
}

BOOL CNetDatagramSender::SetSocket(DWORD dwAddress, WORD wPortIndex)
{
    m_isSocket = TRUE;

    m_dwAddress = dwAddress;
    m_wPortIndex = wPortIndex;

    m_Socket = socket(AF_INET, SOCK_DGRAM, 0);

    memset(&m_SockAddr, 0, sizeof(SOCKADDR_IN));
    m_SockAddr.sin_family = AF_INET;
    m_SockAddr.sin_addr.s_addr = dwAddress;
    m_SockAddr.sin_port = htons(wPortIndex);

    return TRUE;
}

BOOL CNetDatagramSender::Send(const void *pBuffer, int iSize)
{
    assert(isSocket());

    int const iSendingLength = sendto(m_Socket, static_cast<const char *>(pBuffer), iSize, 0,
                                      reinterpret_cast<PSOCKADDR>(&m_SockAddr), sizeof(SOCKADDR_IN));
    if (iSendingLength < 0)
    {
        Tracef("Failed sending packet\n");
        return FALSE;
    }

    return TRUE;
}

BOOL CNetDatagramSender::isSocket() const
{
    return m_isSocket;
}

CNetDatagramSender::CNetDatagramSender() : m_isSocket(FALSE), m_Socket(0), m_dwAddress(0), m_wPortIndex(1000)
{

    memset(&m_SockAddr, 0, sizeof(SOCKADDR_IN));
}

CNetDatagramSender::~CNetDatagramSender() = default;
