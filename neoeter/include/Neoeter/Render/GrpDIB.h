#pragma once

class CGraphicDib
{
  public:
    CGraphicDib();
    virtual ~CGraphicDib();

    void Destroy();
    bool Create(HDC hDC, int width, int height);

    void SetBkMode(int iBkMode);
    void TextOut(int ix, int iy, const char *c_szText);
    void Put(HDC hDC, int x, int y);

    int GetWidth() const;
    int GetHeight() const;

    void *GetPointer();

    HDC GetDCHandle();

  protected:
    void Initialize();

    HDC m_hDC = nullptr;
    HBITMAP m_hBmp = nullptr;
    BITMAPINFO m_bmi{};

    int m_width = 0;
    int m_height = 0;

    void *m_pvBuf = nullptr;
};
