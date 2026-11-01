#pragma once

class CPythonExceptionSender : public IPythonExceptionSender
{
  public:
    CPythonExceptionSender();
    virtual ~CPythonExceptionSender();

    void Send() override;

  protected:
    std::set<DWORD> m_kSet_dwSendedExceptionCRC;
};
