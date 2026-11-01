#include <Neoeter/Render/StdAfx.h>
#include <Neoeter/Render/NetAddress.h>

#ifndef VC_EXTRALEAN

bool CNetworkAddress::GetHostName(char *szName, int size)
{
    return gethostname(szName, size) != SOCKET_ERROR;
}

CNetworkAddress::CNetworkAddress()
{
    Clear();
}

CNetworkAddress::operator const SOCKADDR_IN &() const
{
    return m_sockAddrIn;
}

void CNetworkAddress::Clear()
{
    memset(&m_sockAddrIn, 0, sizeof(m_sockAddrIn));
    m_sockAddrIn.sin_family = AF_INET;
}

bool CNetworkAddress::IsIP(const char *c_szAddr)
{
    return !(c_szAddr[0] < '0' || c_szAddr[0] > '9');
}

bool CNetworkAddress::Set(const char *c_szAddr, int port)
{
    if (IsIP(c_szAddr))
    {
        SetIP(c_szAddr);
    }
    else
    {
        if (!SetDNS(c_szAddr))
            return false;
    }

    SetPort(port);
    return true;
}

void CNetworkAddress::SetLocalIP()
{
    SetIP(INADDR_ANY);
}

void CNetworkAddress::SetIP(DWORD ip)
{
    m_sockAddrIn.sin_addr.s_addr = htonl(ip);
}

void CNetworkAddress::SetIP(const char *c_szIP)
{
    m_sockAddrIn.sin_addr.s_addr = inet_addr(c_szIP);
}

bool CNetworkAddress::SetDNS(const char *c_szDNS)
{
    HOSTENT *pHostent = gethostbyname(c_szDNS);
    if (!pHostent)
        return false;
    memcpy(&m_sockAddrIn.sin_addr, pHostent->h_addr, sizeof(m_sockAddrIn.sin_addr));
    return true;
}

void CNetworkAddress::SetPort(int port)
{
    m_sockAddrIn.sin_port = htons(port);
}

int CNetworkAddress::GetSize()
{
    return sizeof(m_sockAddrIn);
}

DWORD CNetworkAddress::GetIP() const
{
    return ntohl(m_sockAddrIn.sin_addr.s_addr);
}

void CNetworkAddress::GetIP(char *szIP, int len) const
{
    BYTE IPs[4];
    *(reinterpret_cast<DWORD *>(IPs)) = m_sockAddrIn.sin_addr.s_addr;

    _snprintf(szIP, len, "%d.%d.%d.%d", IPs[0], IPs[1], IPs[2], IPs[3]);
}

int CNetworkAddress::GetPort() const
{
    return ntohs(m_sockAddrIn.sin_port);
}

#endif
