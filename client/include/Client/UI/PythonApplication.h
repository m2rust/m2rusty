#pragma once

#include <Neoeter/Render/MSApplication.h>
#include <Neoeter/Render/Input.h>
#include <Neoeter/Render/Profiler.h>
#include <Neoeter/Render/GrpDevice.h>
#include <Neoeter/Render/NetDevice.h>
#include <Neoeter/Render/GrpLightManager.h>
#include <Neoeter/Effect/EffectManager.h>
#include <Client/Game/RaceManager.h>
#include <Client/Game/ItemManager.h>
#include <Client/Game/FlyingObjectManager.h>
#include <Client/Game/GameEventManager.h>
#include <Neoeter/Audio/SoundManager.h>

#include <Client/UI/PythonEventManager.h>
#include <Client/UI/PythonPlayer.h>
#include <Client/UI/PythonNonPlayer.h>
#include <Client/UI/PythonMiniMap.h>
#include <Client/UI/PythonIME.h>
#include <Client/UI/PythonItem.h>
#include <Client/UI/PythonShop.h>
#include <Client/UI/PythonExchange.h>
#include <Client/UI/PythonChat.h>
#include <Client/UI/PythonTextTail.h>
#include <Client/UI/PythonSkill.h>
#include <Client/UI/PythonSystem.h>
// #include "PythonNetworkDatagram.h"
#include <Client/UI/PythonNetworkStream.h>
#include <Client/UI/PythonCharacterManager.h>
#include <Client/UI/PythonQuest.h>
#include <Client/UI/PythonMessenger.h>
#include <Client/UI/PythonSafeBox.h>
#include <Client/UI/PythonGuild.h>

#include <Client/UI/GuildMarkDownloader.h>
#include <Client/UI/GuildMarkUploader.h>

#include <Client/UI/AccountConnector.h>

#include <Client/UI/ServerStateChecker.h>
#include <Client/UI/AbstractApplication.h>
#include <Client/UI/MovieMan.h>

struct IGraphBuilder;
struct IBaseFilter;
struct ISampleGrabber;
struct IMediaControl;
struct IMediaEventEx;
struct IVideoWindow;
struct IBasicVideo;

class CPythonApplication : public CMSApplication, public CInputKeyboard, public IAbstractApplication
{
  public:
    enum EDeviceState
    {
        DEVICE_STATE_FALSE,
        DEVICE_STATE_SKIP,
        DEVICE_STATE_OK,
    };

    enum ECursorMode
    {
        CURSOR_MODE_HARDWARE,
        CURSOR_MODE_SOFTWARE,
    };

    enum ECursorShape
    {
        CURSOR_SHAPE_NORMAL = 0,
        CURSOR_SHAPE_ATTACK = 1,
        CURSOR_SHAPE_TARGET = 2,
        CURSOR_SHAPE_TALK = 3,
        CURSOR_SHAPE_CANT_GO = 4,
        CURSOR_SHAPE_PICK = 5,

        CURSOR_SHAPE_DOOR = 6,
        CURSOR_SHAPE_CHAIR = 7,
        CURSOR_SHAPE_MAGIC = 8, // Magic
        CURSOR_SHAPE_BUY = 9,   // Buy
        CURSOR_SHAPE_SELL = 10, // Sell

        CURSOR_SHAPE_CAMERA_ROTATE = 11, // Camera Rotate
        CURSOR_SHAPE_HSIZE = 12,         // Horizontal Size
        CURSOR_SHAPE_VSIZE = 13,         // Vertical Size
        CURSOR_SHAPE_HVSIZE = 14,        // Horizontal & Vertical Size

        CURSOR_SHAPE_COUNT = 15,

        // 안정적인 네이밍 변환을 위한 임시 enumerate
        NORMAL = CURSOR_SHAPE_NORMAL,
        ATTACK = CURSOR_SHAPE_ATTACK,
        TARGET = CURSOR_SHAPE_TARGET,
        CAMERA_ROTATE = CURSOR_SHAPE_CAMERA_ROTATE,
        CURSOR_COUNT = CURSOR_SHAPE_COUNT,
    };

    enum EInfo
    {
        INFO_ACTOR,
        INFO_EFFECT,
        INFO_ITEM,
        INFO_TEXTTAIL,
    };

    enum ECameraControlDirection
    {
        CAMERA_TO_POSITIVE = 1,
        CAMERA_TO_NEGITIVE = -1,
        CAMERA_STOP = 0,
    };

    enum
    {
        CAMERA_MODE_NORMAL = 0,
        CAMERA_MODE_STAND = 1,
        CAMERA_MODE_BLEND = 2,

        EVENT_CAMERA_NUMBER = 101,
    };

    struct SCameraSpeed
    {
        float m_fUpDir = 0.0f;
        float m_fViewDir = 0.0f;
        float m_fCrossDir = 0.0f;

        SCameraSpeed()
        {
        }
    };

    CPythonApplication();
    ~CPythonApplication() override;

    void ShowWebPage(const char *c_szURL, const RECT &c_rcWebPage);
    void MoveWebPage(const RECT &c_rcWebPage);
    void HideWebPage();

    bool IsWebPageMode();

    void NotifyHack(const char *c_szFormat, ...);
    void GetInfo(UINT eInfo, std::string *pstInfo);
    void GetMousePosition(POINT *ppt) override;

    static CPythonApplication &Instance()
    {
        assert(ms_pInstance != nullptr);
        return *ms_pInstance;
    }

    void Loop();
    void Destroy();
    void Clear();
    void Exit();
    void Abort();

    void SetMinFog(float fMinFog);
    void SetFrameSkip(bool isEnable);
    void SkipRenderBuffering(DWORD dwSleepMSec) override;

    bool Create(PyObject *poSelf, const char *c_szName, int width, int height, int Windowed);
    bool CreateDevice(int width, int height, int Windowed, int bit = 32, int frequency = 0);

    void UpdateGame();
    void RenderGame();

    bool Process();

    void UpdateClientRect();

    bool CreateCursors();
    void DestroyCursors();

    void SafeSetCapture();
    void SafeReleaseCapture();

    BOOL SetCursorNum(int iCursorNum);
    void SetCursorVisible(BOOL bFlag, bool bLiarCursorOn = false);
    BOOL GetCursorVisible() const;
    bool GetLiarCursorOn() const;
    void SetCursorMode(int iMode);
    int GetCursorMode() const;
    int GetCursorNum() const
    {
        return m_iCursorNum;
    }

    void SetMouseHandler(PyObject *poMouseHandler);

    int GetWidth() const;
    int GetHeight() const;

    void SetGlobalCenterPosition(LONG x, LONG y);
    void SetCenterPosition(float fx, float fy, float fz) override;
    void GetCenterPosition(TPixelPosition *pPixelPosition);
    void SetCamera(float Distance, float Pitch, float Rotation, float fDestinationHeight);
    void GetCamera(float *Distance, float *Pitch, float *Rotation, float *DestinationHeight);
    void RotateCamera(int iDirection);
    void PitchCamera(int iDirection);
    void ZoomCamera(int iDirection);
    void MovieRotateCamera(int iDirection);
    void MoviePitchCamera(int iDirection);
    void MovieZoomCamera(int iDirection);
    void MovieResetCamera();
    void SetViewDirCameraSpeed(float fSpeed);
    void SetCrossDirCameraSpeed(float fSpeed);
    void SetUpDirCameraSpeed(float fSpeed);
    float GetRotation();
    float GetPitch();

    void SetFPS(int iFPS);
    void SetServerTime(time_t tTime) override;
    time_t GetServerTime() const;
    time_t GetServerTimeStamp() const;
    float GetGlobalTime() override;
    float GetGlobalElapsedTime() override;

    float GetFaceSpeed() const
    {
        return m_fFaceSpd;
    }
    float GetAveRenderTime() const
    {
        return m_fAveRenderTime;
    }
    DWORD GetCurRenderTime() const
    {
        return m_dwCurRenderTime;
    }
    DWORD GetCurUpdateTime() const
    {
        return m_dwCurUpdateTime;
    }
    DWORD GetUpdateFPS() const
    {
        return m_dwUpdateFPS;
    }
    DWORD GetRenderFPS() const
    {
        return m_dwRenderFPS;
    }
    DWORD GetLoad() const
    {
        return m_dwLoad;
    }
    DWORD GetFaceCount() const
    {
        return m_dwFaceCount;
    }

    void SetConnectData(const char *c_szIP, int iPort);
    void GetConnectData(std::string &rstIP, int &riPort);

    void RunIMEUpdate() override;
    void RunIMETabEvent() override;
    void RunIMEReturnEvent() override;
    void RunPressExitKey();

    void RunIMEChangeCodePage() override;
    void RunIMEOpenCandidateListEvent() override;
    void RunIMECloseCandidateListEvent() override;
    void RunIMEOpenReadingWndEvent() override;
    void RunIMECloseReadingWndEvent() override;

    void EnableSpecialCameraMode();
    void SetCameraSpeed(int iPercentage);

    bool IsLockCurrentCamera();
    void SetEventCamera(const SCameraSetting &c_rCameraSetting) override;
    void BlendEventCamera(const SCameraSetting &c_rCameraSetting, float fBlendTime) override;
    void SetDefaultCamera() override;

    void SetCameraSetting(const SCameraSetting &c_rCameraSetting);
    void GetCameraSetting(SCameraSetting *pCameraSetting);
    void SaveCameraSetting(const char *c_szFileName);
    bool LoadCameraSetting(const char *c_szFileName);

    void SetForceSightRange(int iRange);

    int OnLogoOpen(char *szName);
    int OnLogoUpdate();
    void OnLogoRender();
    void OnLogoClose();

  protected:
    IGraphBuilder *m_pGraphBuilder = nullptr;   // Graph Builder
    IBaseFilter *m_pFilterSG = nullptr;         // Sample Grabber 필터
    ISampleGrabber *m_pSampleGrabber = nullptr; // 영상 이미지 캡처를 위한 샘플 그래버
    IMediaControl *m_pMediaCtrl = nullptr;      // Media Control
    IMediaEventEx *m_pMediaEvent = nullptr;     // Media Event
    IVideoWindow *m_pVideoWnd = nullptr;        // Video Window
    IBasicVideo *m_pBasicVideo = nullptr;
    BYTE *m_pCaptureBuffer = nullptr;           // 영상 이미지를 캡처한 버퍼
    LONG m_lBufferSize = 0;                     // Video 버퍼 크기 변수
    CGraphicImageTexture *m_pLogoTex = nullptr; // 출력할 텍스쳐
    bool m_bLogoError = false;                  // 영상 읽기 상태
    bool m_bLogoPlay = false;

    int m_nLeft = 0, m_nRight = 0, m_nTop = 0, m_nBottom = 0;

    LRESULT WindowProcedure(HWND hWnd, UINT uiMsg, WPARAM wParam, LPARAM lParam) override;

    void OnCameraUpdate();

    void OnUIUpdate();
    void OnUIRender();

    void OnMouseUpdate();
    void OnMouseRender();

    void OnMouseWheel(int nLen);
    void OnMouseMove(int x, int y);
    void OnMouseMiddleButtonDown(int x, int y);
    void OnMouseMiddleButtonUp(int x, int y);
    void OnMouseLeftButtonDown(int x, int y);
    void OnMouseLeftButtonUp(int x, int y);
    void OnMouseLeftButtonDoubleClick(int x, int y);
    void OnMouseRightButtonDown(int x, int y);
    void OnMouseRightButtonUp(int x, int y);
    void OnSizeChange(int width, int height);
    void OnKeyDown(int iIndex) override;
    void OnKeyUp(int iIndex) override;
    void OnIMEKeyDown(int iIndex);

    int CheckDeviceState();

    BOOL __IsContinuousChangeTypeCursor(int iCursorNum);

    void __UpdateCamera();

    void __SetFullScreenWindow(HWND hWnd, DWORD dwWidth, DWORD dwHeight, DWORD dwBPP);
    void __MinimizeFullScreenWindow(HWND hWnd, DWORD dwWidth, DWORD dwHeight);

    CTimer m_timer;

    CLightManager m_LightManager;
    CSoundManager m_SoundManager;
    CFlyingManager m_FlyingManager;
    CRaceManager m_RaceManager;
    CGameEventManager m_GameEventManager;
    CItemManager m_kItemMgr;
    CMovieMan m_MovieManager;

    UI::CWindowManager m_kWndMgr;
    CEffectManager m_kEftMgr;
    CPythonCharacterManager m_kChrMgr;

    CServerStateChecker m_kServerStateChecker;
    CPythonGraphic m_pyGraphic;
    CPythonNetworkStream m_pyNetworkStream;
    // CPythonNetworkDatagram		m_pyNetworkDatagram;
    CPythonPlayer m_pyPlayer;
    CPythonIME m_pyIme;
    CPythonItem m_pyItem;
    CPythonShop m_pyShop;
    CPythonExchange m_pyExchange;
    CPythonChat m_pyChat;
    CPythonTextTail m_pyTextTail;
    CPythonNonPlayer m_pyNonPlayer;
    CPythonMiniMap m_pyMiniMap;
    CPythonEventManager m_pyEventManager;
    CPythonBackground m_pyBackground;
    CPythonSkill m_pySkill;
    CPythonResource m_pyRes;
    CPythonQuest m_pyQuest;
    CPythonMessenger m_pyManager;
    CPythonSafeBox m_pySafeBox;
    CPythonGuild m_pyGuild;

    CGuildMarkManager m_kGuildMarkManager;
    CGuildMarkDownloader m_kGuildMarkDownloader;
    CGuildMarkUploader m_kGuildMarkUploader;
    CAccountConnector m_kAccountConnector;

    CGraphicDevice m_grpDevice;
    CNetworkDevice m_netDevice;

    CPythonSystem m_pySystem;

    PyObject *m_poMouseHandler;
    D3DXVECTOR3 m_v3CenterPosition;

    unsigned int m_iFPS;
    float m_fAveRenderTime = 0.0f;
    DWORD m_dwCurRenderTime = 0U;
    DWORD m_dwCurUpdateTime = 0U;
    DWORD m_dwLoad = 0U;
    DWORD m_dwWidth;
    DWORD m_dwHeight;

    // Time
    DWORD m_dwLastIdleTime = 0;
    DWORD m_dwStartLocalTime;
    time_t m_tServerTime;
    time_t m_tLocalStartTime;
    float m_fGlobalTime = 0.0f;
    float m_fGlobalElapsedTime = 0.0f;

    /////////////////////////////////////////////////////////////
    // Camera
    SCameraSetting m_DefaultCameraSetting;
    SCameraSetting m_kEventCameraSetting;

    int m_iCameraMode;
    float m_fBlendCameraStartTime;
    float m_fBlendCameraBlendTime;
    SCameraSetting m_kEndBlendCameraSetting;

    float m_fRotationSpeed;
    float m_fPitchSpeed;
    float m_fZoomSpeed;
    float m_fCameraRotateSpeed;
    float m_fCameraPitchSpeed;
    float m_fCameraZoomSpeed;

    SCameraPos m_kCmrPos;
    SCameraSpeed m_kCmrSpd;

    BOOL m_isSpecialCameraMode;
    // Camera
    /////////////////////////////////////////////////////////////

    float m_fFaceSpd;
    DWORD m_dwFaceSpdSum;
    DWORD m_dwFaceSpdCount;

    DWORD m_dwFaceAccCount;
    DWORD m_dwFaceAccTime;

    DWORD m_dwUpdateFPS = 0;
    DWORD m_dwRenderFPS = 0;
    DWORD m_dwFaceCount = 0;

    DWORD m_dwLButtonDownTime = 0;
    DWORD m_dwLButtonUpTime = 0U;

    using TCursorHandleMap = std::map<int, HANDLE>;
    TCursorHandleMap m_CursorHandleMap;
    HANDLE m_hCurrentCursor = nullptr;

    BOOL m_bCursorVisible = TRUE;
    bool m_bLiarCursorOn = false;
    int m_iCursorMode = CURSOR_MODE_HARDWARE;
    bool m_isWindowed = false;
    bool m_isFrameSkipDisable = false;

    // Connect Data
    std::string m_strIP;
    int m_iPort;

    static CPythonApplication *ms_pInstance;

    bool m_isMinimizedWnd;
    bool m_isActivateWnd;
    BOOL m_isWindowFullScreenEnable;

    DWORD m_dwStickyKeysFlag = 0U;
    DWORD m_dwBufSleepSkipTime = 0U;
    int m_iForceSightRange;

    int m_iCursorNum;
    int m_iContinuousCursorNum;
};
