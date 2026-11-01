#pragma once

#include <Neoeter/Render/NetStream.h>
#include <Client/UI/MarkImage.h>

#ifdef __VTUNE__

class CGuildMarkUploader : public CNetworkStream, public CSingleton<CGuildMarkUploader>
{
  public:
    enum
    {
        ERROR_NONE,
        ERROR_CONNECT,
        ERROR_LOAD,
        ERROR_WIDTH,
        ERROR_HEIGHT,
    };

  public:
    CGuildMarkUploader()
    {
    }
    virtual ~CGuildMarkUploader()
    {
    }

    void Disconnect()
    {
    }
    bool Connect(const CNetworkAddress &c_rkNetAddr, DWORD dwHandle, DWORD dwRandomKey, DWORD dwGuildID,
                 const char *c_szFileName, UINT *peError)
    {
        return false;
    }
    bool ConnectToSendSymbol(const CNetworkAddress &c_rkNetAddr, DWORD dwHandle, DWORD dwRandomKey, DWORD dwGuildID,
                             const char *c_szFileName, UINT *peError)
    {
        return false;
    }
    void Process()
    {
    }
};

#else

class CGuildMarkUploader : public CNetworkStream, public CSingleton<CGuildMarkUploader>
{
  public:
    enum
    {
        ERROR_NONE,
        ERROR_CONNECT,
        ERROR_LOAD,
        ERROR_WIDTH,
        ERROR_HEIGHT,
    };

    enum
    {
        SEND_TYPE_MARK,
        SEND_TYPE_SYMBOL,
    };

    CGuildMarkUploader();
    ~CGuildMarkUploader() override;

    void Disconnect();
    bool Connect(const CNetworkAddress &c_rkNetAddr, DWORD dwHandle, DWORD dwRandomKey, DWORD dwGuildID,
                 const char *c_szFileName, UINT *peError);
    bool ConnectToSendSymbol(const CNetworkAddress &c_rkNetAddr, DWORD dwHandle, DWORD dwRandomKey, DWORD dwGuildID,
                             const char *c_szFileName, UINT *peError);
    bool IsCompleteUploading() const;

    void Process();

  private:
    enum
    {
        STATE_OFFLINE,
        STATE_LOGIN,
        STATE_COMPLETE,
    };

    void OnConnectFailure() override;
    void OnConnectSuccess() override;
    void OnRemoteDisconnect() override;
    void OnDisconnect() override;

    bool __Load(const char *c_szFileName, UINT *peError);
    bool __LoadSymbol(const char *c_szFileName, UINT *peError);

    bool __Save(const char *c_szFileName);

    void __Inialize();
    bool __StateProcess();

    void __OfflineState_Set();
    void __CompleteState_Set();

    void __LoginState_Set();
    bool __LoginState_Process();
    bool __LoginState_RecvPhase();
    bool __LoginState_RecvHandshake();
    bool __LoginState_RecvPing();

    bool __AnalyzePacket(UINT uHeader, UINT uPacketSize, bool (CGuildMarkUploader::*pfnDispatchPacket)());

    bool __SendMarkPacket();
    bool __SendSymbolPacket();

    UINT m_eState = 0U;

    DWORD m_dwSendType = 0U;
    DWORD m_dwHandle = 0U;
    DWORD m_dwRandomKey = 0U;
    DWORD m_dwGuildID = 0U;

    SGuildMark m_kMark{};

    DWORD m_dwSymbolBufSize = 0U;
    DWORD m_dwSymbolCRC32 = 0U;
    BYTE *m_pbySymbolBuf;
};

#endif
