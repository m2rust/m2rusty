#pragma once

#include <Neoeter/Render/NetStream.h>
#include <Client/UI/MarkManager.h>

class CGuildMarkDownloader : public CNetworkStream, public CSingleton<CGuildMarkDownloader>
{
  public:
    CGuildMarkDownloader();
    ~CGuildMarkDownloader() override;

    bool Connect(const CNetworkAddress &c_rkNetAddr, DWORD dwHandle, DWORD dwRandomKey);
    bool ConnectToRecvSymbol(const CNetworkAddress &c_rkNetAddr, DWORD dwHandle, DWORD dwRandomKey,
                             const std::vector<DWORD> &c_rkVec_dwGuildID);

    void Process();

  private:
    enum
    {
        STATE_OFFLINE,
        STATE_LOGIN,
        STATE_COMPLETE,
    };

    enum
    {
        TODO_RECV_NONE,
        TODO_RECV_MARK,
        TODO_RECV_SYMBOL,
    };

    void OnConnectFailure() override;
    void OnConnectSuccess() override;
    void OnRemoteDisconnect() override;
    void OnDisconnect() override;

    void __Initialize();
    bool __StateProcess();

    UINT __GetPacketSize(UINT header);
    bool __DispatchPacket(UINT header);

    void __OfflineState_Set();
    void __CompleteState_Set();

    void __LoginState_Set();
    bool __LoginState_Process();
    bool __LoginState_RecvPhase();
    bool __LoginState_RecvHandshake();
    bool __LoginState_RecvPing();
    bool __LoginState_RecvMarkIndex();
    bool __LoginState_RecvMarkBlock();
    bool __LoginState_RecvSymbolData();
    bool __SendMarkIDXList();
    bool __SendMarkCRCList();
    bool __SendSymbolCRCList();

    DWORD m_dwHandle = 0U;
    DWORD m_dwRandomKey = 0U;
    DWORD m_dwTodo = 0U;

    std::vector<DWORD> m_kVec_dwGuildID;

    UINT m_eState = 0U;

    BYTE m_currentRequestingImageIndex = 0U;

    CGuildMarkManager *m_pkMarkMgr = nullptr;

    DWORD m_dwBlockIndex = 0U;
    DWORD m_dwBlockDataSize = 0U;
    DWORD m_dwBlockDataPos = 0U;
};
