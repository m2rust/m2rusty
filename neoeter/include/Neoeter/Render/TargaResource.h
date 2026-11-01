#pragma once

#include "Ref.h"
#include "Resource.h"
#include <Neoeter/Image/TGAImage.h>

class CTargaResource : public CResource
{
  public:
    using TRef = CRef<CTargaResource>;

    static TType Type();

    CTargaResource(const char *c_pszFileName);
    ~CTargaResource() override;

    DWORD *GetMemPtr();
    void GetRect(DWORD &w, DWORD &h);

    TGA_HEADER &GetTgaHeader();

  protected:
    bool OnLoad(int iSize, const void *c_pvBuf) override;
    void OnClear() override;
    bool OnIsEmpty() const override;
    bool OnIsType(TType type) override;

    CTGAImage image;
};
