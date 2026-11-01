#pragma once

#include "DibBar.h"

class CTextBar : public CDibBar
{
  public:
    CTextBar(int fontSize, bool isBold);
    ~CTextBar() override;

    void TextOut(int ix, int iy, const char *c_szText);
    void SetTextColor(int r, int g, int b);
    void GetTextExtent(const char *c_szText, SIZE *p_size);

  protected:
    void __SetFont(int fontSize, bool isBold);

    void OnCreate() override;

    HFONT m_hFont = nullptr;
    HFONT m_hOldFont;

    int m_fontSize;
    bool m_isBold;
};
