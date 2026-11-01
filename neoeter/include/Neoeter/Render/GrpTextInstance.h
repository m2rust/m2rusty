#ifndef __INC_ETERLIB_GRPTEXTINSTANCE_H__
#define __INC_ETERLIB_GRPTEXTINSTANCE_H__

#include "Pool.h"
#include "GrpText.h"

class CGraphicTextInstance
{
  public:
    using TPool = CDynamicPool<CGraphicTextInstance>;

    enum EHorizontalAlign
    {
        HORIZONTAL_ALIGN_LEFT = 0x01,
        HORIZONTAL_ALIGN_CENTER = 0x02,
        HORIZONTAL_ALIGN_RIGHT = 0x03,
    };
    enum EVerticalAlign
    {
        VERTICAL_ALIGN_TOP = 0x10,
        VERTICAL_ALIGN_CENTER = 0x20,
        VERTICAL_ALIGN_BOTTOM = 0x30
    };

    static void Hyperlink_UpdateMousePos(int x, int y);
    static int Hyperlink_GetText(char *buf, int len);

    CGraphicTextInstance();
    virtual ~CGraphicTextInstance();

    void Destroy();

    void Update();
    void Render(RECT *pClipRect = nullptr);

    void ShowCursor();
    void HideCursor();

    void ShowOutLine();
    void HideOutLine();

    void SetColor(DWORD color);
    void SetColor(float r, float g, float b, float a = 1.0f);

    void SetOutLineColor(DWORD color);
    void SetOutLineColor(float r, float g, float b, float a = 1.0f);

    void SetHorizonalAlign(int hAlign);
    void SetVerticalAlign(int vAlign);
    void SetMax(int iMax);
    void SetTextPointer(CGraphicText *pText);
    void SetValueString(const std::string &c_stValue);
    void SetValue(const char *c_szText, size_t len = -1);
    void SetPosition(float fx, float fy, float fz = 0.0f);
    void SetSecret(bool Value);
    void SetOutline(bool Value);
    void SetFeather(bool Value);
    void SetMultiLine(bool Value);
    void SetLimitWidth(float fWidth);

    void GetTextSize(int *pRetWidth, int *pRetHeight) const;
    const std::string &GetValueStringReference();
    WORD GetTextLineCount();

    int PixelPositionToCharacterPosition(int iPixelPosition);
    int GetHorizontalAlign() const;

  protected:
    void __Initialize();
    int __DrawCharacter(CGraphicFontTexture *pFontTexture, WORD codePage, wchar_t text, DWORD dwColor);
    void __GetTextPos(DWORD index, float *x, float *y);
    int __GetTextTag(const wchar_t *src, int maxLen, int &tagLen, std::wstring &extraInfo);

    struct SHyperlink
    {
        short sx = 0;
        short ex = 0;
        std::wstring text;

        SHyperlink()
        {
        }
    };

    DWORD m_dwTextColor = 0U;
    DWORD m_dwOutLineColor = 0U;

    WORD m_textWidth = 0U;
    WORD m_textHeight = 0U;

    BYTE m_hAlign = 0U;
    BYTE m_vAlign = 0U;

    WORD m_iMax = 0U;
    float m_fLimitWidth = 0.0F;

    bool m_isCursor = false;
    bool m_isSecret = false;
    bool m_isMultiLine = false;

    bool m_isOutline = false;
    float m_fFontFeather = 0.0F;

    /////

    std::string m_stText;
    D3DXVECTOR3 m_v3Position;

  private:
    bool m_isUpdate = false;
    bool m_isUpdateFontTexture = false;

    CGraphicText::TRef m_roText;
    CGraphicFontTexture::TPCharacterInfomationVector m_pCharInfoVector;
    std::vector<DWORD> m_dwColorInfoVector;
    std::vector<SHyperlink> m_hyperlinkVector;

  public:
    static void CreateSystem(UINT uCapacity);
    static void DestroySystem();

    static CGraphicTextInstance *New();
    static void Delete(CGraphicTextInstance *pkInst);

    static CDynamicPool<CGraphicTextInstance> ms_kPool;
};

extern const char *FindToken(const char *begin, const char *end);
extern int ReadToken(const char *token);

#endif
