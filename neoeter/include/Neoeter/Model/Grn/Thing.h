#pragma once

#include "Model.h"
#include "Motion.h"

class CGraphicThing : public CResource
{
  public:
    using TRef = CRef<CGraphicThing>;

    static CGraphicThing::TType Type();

    CGraphicThing(const char *c_szFileName);
    ~CGraphicThing() override;

    bool CreateDeviceObjects() override;
    void DestroyDeviceObjects() override;

    bool CheckModelIndex(int iModel) const;
    CGrannyModel *GetModelPointer(int iModel);
    int GetModelCount() const;

    bool CheckMotionIndex(int iMotion) const;
    CGrannyMotion *GetMotionPointer(int iMotion);
    int GetMotionCount() const;

  protected:
    void Initialize();

    bool LoadModels();
    bool LoadMotions();

    bool OnLoad(int iSize, const void *c_pvBuf) override;
    void OnClear() override;
    bool OnIsEmpty() const override;
    bool OnIsType(TType type) override;

    granny_file *m_pgrnFile = nullptr;
    granny_file_info *m_pgrnFileInfo = nullptr;

    granny_animation *m_pgrnAni = nullptr;

    CGrannyModel *m_models = nullptr;
    CGrannyMotion *m_motions = nullptr;
};
