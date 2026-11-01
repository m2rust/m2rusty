#pragma once

#include <Neoeter/Render/GrpVertexBuffer.h>
#include <Neoeter/Render/GrpIndexBuffer.h>

#include "Mesh.h"

class CGrannyModel : public CReferenceObject
{
  public:
    using TMeshNode = struct SMeshNode
    {
        int iMesh;
        const CGrannyMesh *pMesh;
        SMeshNode *pNextMeshNode;
    };

    CGrannyModel();
    ~CGrannyModel() override;

    bool IsEmpty() const;
    bool CreateFromGrannyModelPointer(granny_model *pgrnModel);
    bool CreateDeviceObjects();
    void DestroyDeviceObjects();
    void Destroy();

    int GetRigidVertexCount() const;
    int GetDeformVertexCount() const;
    int GetVertexCount() const;

    bool CanDeformPNTVertices() const;
    void DeformPNTVertices(void *dstBaseVertices, D3DXMATRIX *boneMatrices,
                           const std::vector<granny_mesh_binding *> &c_rvct_pgrnMeshBinding) const;

    int GetIdxCount() const;
    int GetMeshCount() const;
    CGrannyMesh *GetMeshPointer(int iMesh);
    granny_model *GetGrannyModelPointer();
    const CGrannyMesh *GetMeshPointer(int iMesh) const;

    LPDIRECT3DVERTEXBUFFER9 GetPNTD3DVertexBuffer() const;
    LPDIRECT3DINDEXBUFFER9 GetD3DIndexBuffer() const;

    const CGrannyModel::TMeshNode *GetMeshNodeList(CGrannyMesh::EType eMeshType,
                                                   CGrannyMaterial::EType eMtrlType) const;

    bool LockVertices(void **indicies, void **vertices) const;
    void UnlockVertices() const;

    const CGrannyMaterialPalette &GetMaterialPalette() const;

  protected:
    bool LoadMeshs();
    bool LoadPNTVertices();
    bool LoadIndices();
    void Initialize();

    BOOL CheckMeshIndex(int iIndex) const;
    void AppendMeshNode(CGrannyMesh::EType eMeshType, CGrannyMaterial::EType eMtrlType, int iMesh);

    // Granny Data
    granny_model *m_pgrnModel = nullptr;

    // Static Data
    CGrannyMesh *m_meshs = nullptr;

    CGraphicVertexBuffer m_pntVtxBuf; // for rigid mesh
    CGraphicIndexBuffer m_idxBuf;

    TMeshNode *m_meshNodes = nullptr;
    TMeshNode *m_meshNodeLists[CGrannyMesh::TYPE_MAX_NUM][CGrannyMaterial::TYPE_MAX_NUM]{};

    int m_deformVtxCount = 0;
    int m_rigidVtxCount = 0;
    int m_vtxCount = 0;
    int m_idxCount = 0;

    int m_meshNodeSize = 0;
    int m_meshNodeCapacity = 0;

    bool m_canDeformPNVertices = false;

    CGrannyMaterialPalette m_kMtrlPal;

  private:
    bool m_bHaveBlendThing = false;

  public:
    bool HaveBlendThing() const
    {
        return m_bHaveBlendThing;
    }

    //////////////////////////////////////////////////////////////////////////
    // New members to support PNT2 type models
  protected:
    bool __LoadVertices();

    DWORD m_dwFvF = 0U;
    // New members to support PNT2 type models
    //////////////////////////////////////////////////////////////////////////
};
