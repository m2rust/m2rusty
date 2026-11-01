#pragma once
#include <map>
#include <Neoeter/Model/SpeedTree/SpeedTreeForest.h>
#include <Neoeter/Model/SpeedTree/SpeedTreeMaterial.h>

class CSpeedTreeDirectX : public CSpeedTreeForest, public CGraphicBase
{
  public:
    CSpeedTreeDirectX();
    ~CSpeedTreeDirectX() override;

    void UploadWindMatrix(unsigned int uiLocation, const float *pMatrix) const override;
    bool SetRenderingDevice() override;
    void Render(unsigned long ulRenderBitVector = Forest_RenderAll) override;
    void UpdateCompundMatrix(const D3DXVECTOR3 &c_rEyeVec, const D3DXMATRIX &c_rmatView,
                             const D3DXMATRIX &c_rmatProj) override;

  private:
    bool InitVertexShaders();

    LPDIRECT3DVERTEXSHADER9 m_dwBranchVertexShader;
    LPDIRECT3DVERTEXSHADER9 m_dwLeafVertexShader;
};
