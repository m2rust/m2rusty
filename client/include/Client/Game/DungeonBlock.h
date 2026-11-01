#pragma once

#include <Neoeter/Render/ResourceManager.h>
#include <Neoeter/Render/GrpObjectInstance.h>
#include <Neoeter/Model/Grn/ModelInstance.h>
#include <Neoeter/Model/Grn/Thing.h>

class CDungeonModelInstance;

class CDungeonBlock : public CGraphicObjectInstance
{
  public:
    using TModelInstanceContainer = std::vector<CDungeonModelInstance *>;
    enum
    {
        ID = THING_OBJECT
    };
    int GetType() const override
    {
        return ID;
    }

    CDungeonBlock();
    ~CDungeonBlock() override;

    void Destroy();

    void BuildBoundingSphere();
    bool Load(const char *c_szFileName);

    bool Intersect(float *pfu, float *pfv, float *pft);
    void GetBoundBox(D3DXVECTOR3 *pv3Min, D3DXVECTOR3 *pv3Max);

    void Update();
    void Render();

    bool GetBoundingSphere(D3DXVECTOR3 &v3Center, float &fRadius) override;
    void OnUpdateCollisionData(const CStaticCollisionDataVector *pscdVector) override;
    void OnUpdateHeighInstance(CAttributeInstance *pAttributeInstance) override;
    bool OnGetObjectHeight(float fX, float fY, float *pfHeight) override;

    void OnRender() override
    {
    }
    void OnBlendRender() override
    {
    }
    void OnRenderToShadowMap() override
    {
    }
    void OnRenderShadow() override;
    void OnRenderPCBlocker() override
    {
    }

  protected:
    void __Initialize();

    D3DXVECTOR3 m_v3Center;
    float m_fRadius = 0.0F;

    CGraphicThing *m_pThing = nullptr;
    TModelInstanceContainer m_ModelInstanceContainer;
    CGraphicVertexBuffer m_kDeformableVertexBuffer;
};
