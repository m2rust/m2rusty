#pragma once

#include "Resource.h"
#include "Ref.h"
#include "GrpFontTexture.h"

class CGraphicText : public CResource
{
  public:
    using TRef = CRef<CGraphicText>;

    static TType Type();

    CGraphicText(const char *c_szFileName);
    ~CGraphicText() override;

    bool CreateDeviceObjects() override;
    void DestroyDeviceObjects() override;

    CGraphicFontTexture *GetFontTexturePointer();

  protected:
    bool OnLoad(int iSize, const void *c_pvBuf) override;
    void OnClear() override;
    bool OnIsEmpty() const override;
    bool OnIsType(TType type) override;

    CGraphicFontTexture m_fontTexture;
};
