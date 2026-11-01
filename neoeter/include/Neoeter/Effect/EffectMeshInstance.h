#pragma once

#include <Neoeter/Render/GrpScreen.h>
#include <Neoeter/Render/GrpImageInstance.h>
#include "EffectElementBaseInstance.h"
#include "FrameController.h"
#include "EffectMesh.h"

class CEffectMeshInstance : public CEffectElementBaseInstance
{
  public:
    // NOTE : Mesh 단위 텍스춰 데이타의 인스턴스이다.
    using TTextureInstance = struct STextureInstance
    {
        CFrameController TextureFrameController;
        std::vector<CGraphicImageInstance *> TextureInstanceVector;
    };

    CEffectMeshInstance();
    ~CEffectMeshInstance() override;

    static void DestroySystem();

    static CEffectMeshInstance *New();
    static void Delete(CEffectMeshInstance *pkMeshInstance);

    static CDynamicPool<CEffectMeshInstance> ms_kPool;

  protected:
    void OnSetDataPointer(CEffectElementBase *pElement) override;

    void OnInitialize() override;
    void OnDestroy() override;

    bool OnUpdate(float fElapsedTime) override;
    void OnRender() override;

    BOOL isActive();

    CEffectMeshScript *m_pMeshScript = nullptr;
    CEffectMesh *m_pEffectMesh = nullptr;

    CFrameController m_MeshFrameController;
    std::vector<TTextureInstance> m_TextureInstanceVector;

    CEffectMesh::TRef m_roMesh;
};
