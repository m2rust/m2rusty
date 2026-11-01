#ifndef __INC_GRPIMAGE_H__
#define __INC_GRPIMAGE_H__

#include "Ref.h"
#include "Resource.h"
#include "GrpImageTexture.h"

class CGraphicImage : public CResource
{
  public:
    using TRef = CRef<CGraphicImage>;

    static TType Type();

    CGraphicImage(const char *c_szFileName, DWORD dwFilter = D3DX_FILTER_LINEAR);
    ~CGraphicImage() override;

    bool CreateDeviceObjects() override;
    void DestroyDeviceObjects() override;

    int GetWidth() const;
    int GetHeight() const;

    const RECT &GetRectReference() const;

    const CGraphicTexture &GetTextureReference() const;
    CGraphicTexture *GetTexturePointer();

  protected:
    bool OnLoad(int iSize, const void *c_pvBuf) override;

    void OnClear() override;
    bool OnIsEmpty() const override;
    bool OnIsType(TType type) override;

    CGraphicImageTexture m_imageTexture;
    RECT m_rect{};
    DWORD m_dwFilter;
};

#endif
