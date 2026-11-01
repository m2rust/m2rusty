#pragma once

class CNetworkDevice
{
  public:
    CNetworkDevice();
    virtual ~CNetworkDevice();

    void Destroy();
    bool Create();

  protected:
    void Initialize();

    bool m_isWSA = false;
};
