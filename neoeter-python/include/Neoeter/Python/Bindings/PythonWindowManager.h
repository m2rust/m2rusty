#pragma once

namespace UI
{
class CWindow;

class CWindowManager : public CSingleton<CWindowManager>
{
  public:
    using TLayerContainer = std::map<std::string, CWindow *>;
    using TWindowContainer = std::list<CWindow *>;
    using TKeyCaptureWindowMap = std::map<int, CWindow *>;

    CWindowManager();
    ~CWindowManager() override;

    void Destroy();

    float GetAspect() const;
    void SetScreenSize(long lWidth, long lHeight);
    void SetResolution(int hres, int vres);

    void GetResolution(long &rx, long &ry) const
    {
        rx = m_iHres;
        ry = m_iVres;
    }

    void SetMouseHandler(PyObject *poMouseHandler);
    long GetScreenWidth() const
    {
        return m_lWidth;
    }
    long GetScreenHeight() const
    {
        return m_lHeight;
    }
    void GetMousePosition(long &rx, long &ry) const;
    BOOL IsDragging() const;

    CWindow *GetLockWindow()
    {
        return m_pLockWindow;
    }
    CWindow *GetPointWindow()
    {
        return m_pPointWindow;
    }
    bool IsFocus()
    {
        return (m_pActiveWindow || m_pLockWindow);
    }
    bool IsFocusWindow(CWindow *pWindow)
    {
        return pWindow == m_pActiveWindow;
    }

    void SetParent(CWindow *pWindow, CWindow *pParentWindow);
    void SetPickAlways(CWindow *pWindow);

    enum
    {
        WT_NORMAL,
        WT_SLOT,
        WT_GRIDSLOT,
        WT_TEXTLINE,
        WT_MARKBOX,
        WT_IMAGEBOX,
        WT_EXP_IMAGEBOX,
        WT_ANI_IMAGEBOX,
        WT_BUTTON,
        WT_RATIOBUTTON,
        WT_TOGGLEBUTTON,
        WT_DRAGBUTTON,
        WT_BOX,
        WT_BAR,
        WT_LINE,
        WT_BAR3D,
        WT_NUMLINE,
    };

    CWindow *RegisterWindow(PyObject *po, const char *c_szLayer);
    CWindow *RegisterTypeWindow(PyObject *po, DWORD dwWndType, const char *c_szLayer);

    CWindow *RegisterSlotWindow(PyObject *po, const char *c_szLayer);
    CWindow *RegisterGridSlotWindow(PyObject *po, const char *c_szLayer);
    CWindow *RegisterTextLine(PyObject *po, const char *c_szLayer);
    CWindow *RegisterMarkBox(PyObject *po, const char *c_szLayer);
    CWindow *RegisterImageBox(PyObject *po, const char *c_szLayer);
    CWindow *RegisterExpandedImageBox(PyObject *po, const char *c_szLayer);
    CWindow *RegisterAniImageBox(PyObject *po, const char *c_szLayer);
    CWindow *RegisterButton(PyObject *po, const char *c_szLayer);
    CWindow *RegisterRadioButton(PyObject *po, const char *c_szLayer);
    CWindow *RegisterToggleButton(PyObject *po, const char *c_szLayer);
    CWindow *RegisterDragButton(PyObject *po, const char *c_szLayer);
    CWindow *RegisterBox(PyObject *po, const char *c_szLayer);
    CWindow *RegisterBar(PyObject *po, const char *c_szLayer);
    CWindow *RegisterLine(PyObject *po, const char *c_szLayer);
    CWindow *RegisterBar3D(PyObject *po, const char *c_szLayer);
    CWindow *RegisterNumberLine(PyObject *po, const char *c_szLayer);

    void DestroyWindow(CWindow *pWin);
    void NotifyDestroyWindow(CWindow *pWindow);

    // Attaching Icon
    BOOL IsAttaching() const;
    DWORD GetAttachingType() const;
    DWORD GetAttachingIndex() const;
    DWORD GetAttachingSlotNumber() const;
    void GetAttachingIconSize(BYTE *pbyWidth, BYTE *pbyHeight) const;
    void AttachIcon(DWORD dwType, DWORD dwIndex, DWORD dwSlotNumber, BYTE byWidth, BYTE byHeight);
    void DeattachIcon();
    void SetAttachingFlag(BOOL bFlag);
    // Attaching Icon

    void OnceIgnoreMouseLeftButtonUpEvent();
    void LockWindow(CWindow *pWin);
    void UnlockWindow();

    void ActivateWindow(CWindow *pWin);
    void DeactivateWindow();
    CWindow *GetActivateWindow();
    void SetTop(CWindow *pWin);
    void SetTopUIWindow();
    void ResetCapture();

    void Update();
    void Render();

    void RunMouseMove(long x, long y);
    void RunMouseLeftButtonDown(long x, long y);
    void RunMouseLeftButtonUp(long x, long y);
    void RunMouseLeftButtonDoubleClick(long x, long y);
    void RunMouseRightButtonDown(long x, long y);
    void RunMouseRightButtonUp(long x, long y);
    void RunMouseRightButtonDoubleClick(long x, long y);
    void RunMouseMiddleButtonDown(long x, long y);
    void RunMouseMiddleButtonUp(long x, long y);

    void RunIMEUpdate();
    void RunIMETabEvent();
    void RunIMEReturnEvent();
    void RunIMEKeyDown(int vkey);
    void RunChangeCodePage();
    void RunOpenCandidate();
    void RunCloseCandidate();
    void RunOpenReading();
    void RunCloseReading();

    void RunKeyDown(int vkey);
    void RunKeyUp(int vkey);
    void RunPressEscapeKey();
    void RunPressExitKey();

  private:
    void SetMousePosition(long x, long y);
    CWindow *__PickWindow(long x, long y);

    CWindow *__NewWindow(PyObject *po, DWORD dwWndType);
    void __ClearReserveDeleteWindowList();

    long m_lWidth = 0L;
    long m_lHeight = 0L;

    int m_iVres = 0;
    int m_iHres = 0;

    long m_lMouseX = 0L, m_lMouseY = 0L;
    long m_lDragX = 0L, m_lDragY = 0L;
    long m_lPickedX = 0L, m_lPickedY = 0L;

    BOOL m_bOnceIgnoreMouseLeftButtonUpEventFlag = FALSE;
    int m_iIgnoreEndTime = 0;

    // Attaching Icon
    PyObject *m_poMouseHandler;
    BOOL m_bAttachingFlag = FALSE;
    DWORD m_dwAttachingType = 0;
    DWORD m_dwAttachingIndex = 0;
    DWORD m_dwAttachingSlotNumber = 0;
    BYTE m_byAttachingIconWidth = 0U;
    BYTE m_byAttachingIconHeight = 0U;
    // Attaching Icon

    CWindow *m_pActiveWindow;
    TWindowContainer m_ActiveWindowList;
    CWindow *m_pLockWindow;
    TWindowContainer m_LockWindowList;
    CWindow *m_pPointWindow;
    CWindow *m_pLeftCaptureWindow;
    CWindow *m_pRightCaptureWindow;
    CWindow *m_pMiddleCaptureWindow;
    TKeyCaptureWindowMap m_KeyCaptureWindowMap;
    TWindowContainer m_ReserveDeleteWindowList;
    TWindowContainer m_PickAlwaysWindowList;

    CWindow *m_pRootWindow;
    TWindowContainer m_LayerWindowList;
    TLayerContainer m_LayerWindowMap;
};

PyObject *BuildEmptyTuple();
}; // namespace UI
