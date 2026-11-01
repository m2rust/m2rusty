#pragma once

#include <Neoeter/Platform/Base/Utils.h>

namespace UI
{
class CWindow
{
  public:
    using TWindowContainer = std::list<CWindow *>;

    static DWORD Type();
    BOOL IsType(DWORD dwType);

    enum EHorizontalAlign
    {
        HORIZONTAL_ALIGN_LEFT = 0,
        HORIZONTAL_ALIGN_CENTER = 1,
        HORIZONTAL_ALIGN_RIGHT = 2,
    };

    enum EVerticalAlign
    {
        VERTICAL_ALIGN_TOP = 0,
        VERTICAL_ALIGN_CENTER = 1,
        VERTICAL_ALIGN_BOTTOM = 2,
    };

    enum EFlags
    {
        FLAG_MOVABLE = (1 << 0), // 움직일 수 있는 창
        FLAG_LIMIT = (1 << 1),   // 창이 화면을 벗어나지 않음
        FLAG_SNAP = (1 << 2),    // 스냅 될 수 있는 창
        FLAG_DRAGABLE = (1 << 3),
        FLAG_ATTACH = (1 << 4),     // 완전히 부모에 붙어 있는 창 (For Drag / ex. ScriptWindow)
        FLAG_RESTRICT_X = (1 << 5), // 좌우 이동 제한
        FLAG_RESTRICT_Y = (1 << 6), // 상하 이동 제한
        FLAG_NOT_CAPTURE = (1 << 7),
        FLAG_FLOAT = (1 << 8),    // 공중에 떠있어서 순서 재배치가 되는 창
        FLAG_NOT_PICK = (1 << 9), // 마우스에 의해 Pick되지 않는 창
        FLAG_IGNORE_SIZE = (1 << 10),
        FLAG_RTL = (1 << 11), // Right-to-left
    };

    CWindow(PyObject *ppyObject);
    virtual ~CWindow();

    void AddChild(CWindow *pWin);

    void Clear();
    void DestroyHandle();
    void Update();
    void Render();

    void SetName(const char *c_szName);
    const char *GetName()
    {
        return m_strName.c_str();
    }
    void SetSize(long width, long height);
    long GetWidth() const
    {
        return m_lWidth;
    }
    long GetHeight() const
    {
        return m_lHeight;
    }

    void SetHorizontalAlign(DWORD dwAlign);
    void SetVerticalAlign(DWORD dwAlign);
    void SetPosition(long x, long y);
    void GetPosition(long *plx, long *ply) const;
    long GetPositionX() const
    {
        return m_x;
    }
    long GetPositionY() const
    {
        return m_y;
    }
    RECT &GetRect()
    {
        return m_rect;
    }
    void GetLocalPosition(long &rlx, long &rly) const;
    void GetMouseLocalPosition(long &rlx, long &rly) const;
    long UpdateRect();

    RECT &GetLimitBias()
    {
        return m_limitBiasRect;
    }
    void SetLimitBias(long l, long r, long t, long b)
    {
        m_limitBiasRect.left = l, m_limitBiasRect.right = r, m_limitBiasRect.top = t, m_limitBiasRect.bottom = b;
    }

    void Show();
    void Hide();
    bool IsShow() const
    {
        return m_bShow;
    }
    bool IsRendering();

    bool HasParent()
    {
        return m_pParent != nullptr;
    }
    bool HasChild()
    {
        return !m_pChildList.empty();
    }
    int GetChildCount()
    {
        return m_pChildList.size();
    }

    CWindow *GetRoot();
    CWindow *GetParent();
    bool IsChild(CWindow *pWin);
    void DeleteChild(CWindow *pWin);
    void SetTop(CWindow *pWin);

    bool IsIn(long x, long y) const;
    bool IsIn();
    CWindow *PickWindow(long x, long y);
    CWindow *PickTopWindow(long x,
                           long y); // NOTE : Children으로 내려가지 않고 상위에서만
                                    //        체크 하는 특화된 함수

    void __RemoveReserveChildren();

    void AddFlag(DWORD flag)
    {
        SET_BIT(m_dwFlag, flag);
    }
    void RemoveFlag(DWORD flag)
    {
        REMOVE_BIT(m_dwFlag, flag);
    }
    bool IsFlag(DWORD flag) const
    {
        return (m_dwFlag & flag) != 0;
    }
    /////////////////////////////////////

    virtual void OnRender();
    virtual void OnUpdate();
    virtual void OnChangePosition()
    {
    }

    virtual void OnSetFocus();
    virtual void OnKillFocus();

    virtual void OnMouseDrag(long lx, long ly);
    virtual void OnMouseOverIn();
    virtual void OnMouseOverOut();
    virtual void OnMouseOver();
    virtual void OnDrop();
    virtual void OnTop();
    virtual void OnIMEUpdate();

    virtual void OnMoveWindow(long x, long y);

    ///////////////////////////////////////

    BOOL RunIMETabEvent();
    BOOL RunIMEReturnEvent();
    BOOL RunIMEKeyDownEvent(int ikey);

    CWindow *RunKeyDownEvent(int ikey);
    BOOL RunKeyUpEvent(int ikey);
    BOOL RunPressEscapeKeyEvent();
    BOOL RunPressExitKeyEvent();

    virtual BOOL OnIMETabEvent();
    virtual BOOL OnIMEReturnEvent();
    virtual BOOL OnIMEKeyDownEvent(int ikey);

    virtual BOOL OnIMEChangeCodePage();
    virtual BOOL OnIMEOpenCandidateListEvent();
    virtual BOOL OnIMECloseCandidateListEvent();
    virtual BOOL OnIMEOpenReadingWndEvent();
    virtual BOOL OnIMECloseReadingWndEvent();

    virtual BOOL OnMouseLeftButtonDown();
    virtual BOOL OnMouseLeftButtonUp();
    virtual BOOL OnMouseLeftButtonDoubleClick();
    virtual BOOL OnMouseRightButtonDown();
    virtual BOOL OnMouseRightButtonUp();
    virtual BOOL OnMouseRightButtonDoubleClick();
    virtual BOOL OnMouseMiddleButtonDown();
    virtual BOOL OnMouseMiddleButtonUp();

    virtual BOOL OnKeyDown(int ikey);
    virtual BOOL OnKeyUp(int ikey);
    virtual BOOL OnPressEscapeKey();
    virtual BOOL OnPressExitKey();
    ///////////////////////////////////////

    virtual void SetColor(DWORD dwColor)
    {
    }
    virtual BOOL OnIsType(DWORD dwType);
    /////////////////////////////////////

    virtual BOOL IsWindow()
    {
        return TRUE;
    }
    /////////////////////////////////////

  protected:
    std::string m_strName;

    EHorizontalAlign m_HorizontalAlign;
    EVerticalAlign m_VerticalAlign;
    long m_x = 0, m_y = 0;            // X,Y 상대좌표
    long m_lWidth = 0, m_lHeight = 0; // 크기
    RECT m_rect{};                    // Global 좌표
    RECT m_limitBiasRect{};           // limit bias 값

    bool m_bMovable = false;
    bool m_bShow = false;

    DWORD m_dwFlag = 0;

    PyObject *m_poHandler;

    CWindow *m_pParent;
    TWindowContainer m_pChildList;

    BOOL m_isUpdatingChildren = FALSE;
    TWindowContainer m_pReserveChildList;

#ifdef _DEBUG
  public:
    DWORD DEBUG_dwCounter;
#endif
};

class CLayer : public CWindow
{
  public:
    CLayer(PyObject *ppyObject) : CWindow(ppyObject)
    {
    }
    ~CLayer() override = default;

    BOOL IsWindow() override
    {
        return FALSE;
    }
};

class CBox : public CWindow
{
  public:
    CBox(PyObject *ppyObject);
    ~CBox() override;

    void SetColor(DWORD dwColor) override;

  protected:
    void OnRender() override;

    DWORD m_dwColor = 0xff000000;
};

class CBar : public CWindow
{
  public:
    CBar(PyObject *ppyObject);
    ~CBar() override;

    void SetColor(DWORD dwColor) override;

  protected:
    void OnRender() override;

    DWORD m_dwColor = 0xff000000;
};

class CLine : public CWindow
{
  public:
    CLine(PyObject *ppyObject);
    ~CLine() override;

    void SetColor(DWORD dwColor) override;

  protected:
    void OnRender() override;

    DWORD m_dwColor = 0xff000000;
};

class CBar3D : public CWindow
{
  public:
    static DWORD Type();

    CBar3D(PyObject *ppyObject);
    ~CBar3D() override;

    void SetColor(DWORD dwLeft, DWORD dwRight, DWORD dwCenter);

  protected:
    void OnRender() override;

    DWORD m_dwLeftColor;
    DWORD m_dwRightColor;
    DWORD m_dwCenterColor;
};

// Text
class CTextLine : public CWindow
{
  public:
    CTextLine(PyObject *ppyObject);
    ~CTextLine() override;

    void SetMax(int iMax);
    void SetHorizontalAlign(int iType);
    void SetVerticalAlign(int iType);
    void SetSecret(BOOL bFlag);
    void SetOutline(BOOL bFlag);
    void SetFeather(BOOL bFlag);
    void SetMultiLine(BOOL bFlag);
    void SetFontName(const char *c_szFontName);
    void SetFontColor(DWORD dwColor);
    void SetLimitWidth(float fWidth);

    void ShowCursor();
    void HideCursor();
    int GetCursorPosition();

    void SetText(const char *c_szText);
    const char *GetText();

    void GetTextSize(int *pnWidth, int *pnHeight);

  protected:
    void OnUpdate() override;
    void OnRender() override;
    void OnChangePosition() override;

    virtual void OnSetText(const char *c_szText);

    CGraphicTextInstance m_TextInstance;
};

class CNumberLine : public CWindow
{
  public:
    CNumberLine(PyObject *ppyObject);
    CNumberLine(CWindow *pParent);
    ~CNumberLine() override;

    void SetPath(const char *c_szPath);
    void SetHorizontalAlign(int iType);
    void SetNumber(const char *c_szNumber);

  protected:
    void ClearNumber();
    void OnRender() override;
    void OnChangePosition() override;

    std::string m_strPath;
    std::string m_strNumber;
    std::vector<CGraphicImageInstance *> m_ImageInstanceVector;

    int m_iHorizontalAlign;
    DWORD m_dwWidthSummary;
};

// Image
class CImageBox : public CWindow
{
  public:
    CImageBox(PyObject *ppyObject);
    ~CImageBox() override;

    BOOL LoadImage(const char *c_szFileName);
    void SetDiffuseColor(float fr, float fg, float fb, float fa);

    int GetWidth();
    int GetHeight();

  protected:
    virtual void OnCreateInstance();
    virtual void OnDestroyInstance();

    void OnUpdate() override;
    void OnRender() override;
    void OnChangePosition() override;

    CGraphicImageInstance *m_pImageInstance;
};
class CMarkBox : public CWindow
{
  public:
    CMarkBox(PyObject *ppyObject);
    ~CMarkBox() override;

    void LoadImage(const char *c_szFilename);
    void SetDiffuseColor(float fr, float fg, float fb, float fa);
    void SetIndex(UINT uIndex);
    void SetScale(FLOAT fScale);

  protected:
    virtual void OnCreateInstance();
    virtual void OnDestroyInstance();

    void OnUpdate() override;
    void OnRender() override;
    void OnChangePosition() override;

    CGraphicMarkInstance *m_pMarkInstance;
};
class CExpandedImageBox : public CImageBox
{
  public:
    static DWORD Type();

    CExpandedImageBox(PyObject *ppyObject);
    ~CExpandedImageBox() override;

    void SetScale(float fx, float fy);
    void SetOrigin(float fx, float fy);
    void SetRotation(float fRotation);
    void SetRenderingRect(float fLeft, float fTop, float fRight, float fBottom);
    void SetRenderingMode(int iMode);

  protected:
    void OnCreateInstance() override;
    void OnDestroyInstance() override;

    void OnUpdate() override;
    void OnRender() override;

    BOOL OnIsType(DWORD dwType) override;
};
class CAniImageBox : public CWindow
{
  public:
    static DWORD Type();

    CAniImageBox(PyObject *ppyObject);
    ~CAniImageBox() override;

    void SetDelay(int iDelay);
    void AppendImage(const char *c_szFileName);
    void SetRenderingRect(float fLeft, float fTop, float fRight, float fBottom);
    void SetRenderingMode(int iMode);

    void ResetFrame();

  protected:
    void OnUpdate() override;
    void OnRender() override;
    void OnChangePosition() override;
    virtual void OnEndFrame();

    BOOL OnIsType(DWORD dwType) override;

    BYTE m_bycurDelay = 0;
    BYTE m_byDelay = 4;
    BYTE m_bycurIndex = 0;
    std::vector<CGraphicExpandedImageInstance *> m_ImageVector;
};

// Button
class CButton : public CWindow
{
  public:
    CButton(PyObject *ppyObject);
    ~CButton() override;

    BOOL SetUpVisual(const char *c_szFileName);
    BOOL SetOverVisual(const char *c_szFileName);
    BOOL SetDownVisual(const char *c_szFileName);
    BOOL SetDisableVisual(const char *c_szFileName);

    const char *GetUpVisualFileName();
    const char *GetOverVisualFileName();
    const char *GetDownVisualFileName();

    void Flash();
    void Enable();
    void Disable();

    void SetUp();
    void Up();
    void Over();
    void Down();

    BOOL IsDisable() const;
    BOOL IsPressed() const;

  protected:
    void OnUpdate() override;
    void OnRender() override;
    void OnChangePosition() override;

    BOOL OnMouseLeftButtonDown() override;
    BOOL OnMouseLeftButtonDoubleClick() override;
    BOOL OnMouseLeftButtonUp() override;
    void OnMouseOverIn() override;
    void OnMouseOverOut() override;

    BOOL IsEnable() const;

    void SetCurrentVisual(CGraphicImageInstance *pVisual);

    BOOL m_bEnable = TRUE;
    BOOL m_isPressed = FALSE;
    BOOL m_isFlash = FALSE;
    CGraphicImageInstance *m_pcurVisual;
    CGraphicImageInstance m_upVisual;
    CGraphicImageInstance m_overVisual;
    CGraphicImageInstance m_downVisual;
    CGraphicImageInstance m_disableVisual;
};
class CRadioButton : public CButton
{
  public:
    CRadioButton(PyObject *ppyObject);
    ~CRadioButton() override;

  protected:
    BOOL OnMouseLeftButtonDown() override;
    BOOL OnMouseLeftButtonUp() override;
    void OnMouseOverIn() override;
    void OnMouseOverOut() override;
};
class CToggleButton : public CButton
{
  public:
    CToggleButton(PyObject *ppyObject);
    ~CToggleButton() override;

  protected:
    BOOL OnMouseLeftButtonDown() override;
    BOOL OnMouseLeftButtonUp() override;
    void OnMouseOverIn() override;
    void OnMouseOverOut() override;
};
class CDragButton : public CButton
{
  public:
    CDragButton(PyObject *ppyObject);
    ~CDragButton() override;

    void SetRestrictMovementArea(int ix, int iy, int iwidth, int iheight);

  protected:
    void OnChangePosition() override;
    void OnMouseOverIn() override;
    void OnMouseOverOut() override;

    RECT m_restrictArea{};
};
}; // namespace UI

extern BOOL g_bOutlineBoxEnable;
