#pragma once

#include "GrpImage.h"

class CGraphicSubImage : public CGraphicImage
{
  public:
    using TRef = CRef<CGraphicImage>;

    static TType Type();
    static char m_SearchPath[256];

    CGraphicSubImage(const char *c_szFileName);
    ~CGraphicSubImage() override;

    bool CreateDeviceObjects() override;

    bool SetImageFileName(const char *c_szFileName);

    void SetRectPosition(int left, int top, int right, int bottom);

    void SetRectReference(const RECT &c_rRect);

    static void SetSearchPath(const char *c_szFileName);

  protected:
    void SetImagePointer(CGraphicImage *pImage);

    bool OnLoad(int iSize, const void *c_pvBuf) override;
    void OnClear() override;
    bool OnIsEmpty() const override;
    bool OnIsType(TType type) override;

    CGraphicImage::TRef m_roImage;
};
