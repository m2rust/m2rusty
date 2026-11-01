///////////////////////////////////////////////////////////////////////
//	SpeedTreeRTExample Class
//
//	(c) 2003 IDV, Inc.
//
//	This class is provided to illustrate one way to incorporate
//	SpeedTreeRT into an OpenGL application.  All of the SpeedTreeRT
//	calls that must be made on a per tree basis are done by this class.
//	Calls that apply to all trees (i.e. static SpeedTreeRT functions)
//	are made in the functions in main.cpp.
//
//
//	*** INTERACTIVE DATA VISUALIZATION (IDV) PROPRIETARY INFORMATION ***
//
//	This software is supplied under the terms of a license agreement or
//	nondisclosure agreement with Interactive Data Visualization and may
//	not be copied or disclosed except in accordance with the terms of
//	that agreement.
//
//      Copyright (c) 2001-2003 IDV, Inc.
//      All Rights Reserved.
//
//		IDV, Inc.
//		1233 Washington St. Suite 610
//		Columbia, SC 29201
//		Voice: (803) 799-1699
//		Fax:   (803) 931-0320
//		Web:   http://www.idvinc.com
//

#pragma once

#include <Neoeter/Model/SpeedTree/SpeedTreeMaterial.h>
#include <Neoeter/Model/SpeedTree/SpeedTreeRTCompat.h>
#include <Platform/DirectXMathCompat.h>
#include <vector>

#include <Neoeter/Render/GrpObjectInstance.h>
#include <Neoeter/Render/GrpImageInstance.h>

#ifndef SAFE_DELETE
#define SAFE_DELETE(p)                                                                                                 \
    {                                                                                                                  \
        if (p)                                                                                                         \
        {                                                                                                              \
            delete (p);                                                                                                \
            (p) = NULL;                                                                                                \
        }                                                                                                              \
    }
#endif

#ifndef SAFE_DELETE_ARRAY
#define SAFE_DELETE_ARRAY(p)                                                                                           \
    {                                                                                                                  \
        if (p)                                                                                                         \
        {                                                                                                              \
            delete[] (p);                                                                                              \
            (p) = NULL;                                                                                                \
        }                                                                                                              \
    }
#endif

#ifndef SAFE_RELEASE
#define SAFE_RELEASE(p)                                                                                                \
    {                                                                                                                  \
        if (p)                                                                                                         \
        {                                                                                                              \
            (p)->Release();                                                                                            \
            (p) = NULL;                                                                                                \
        }                                                                                                              \
    }
#endif

///////////////////////////////////////////////////////////////////////
//	class CSpeedTreeWrapper declaration

class CSpeedTreeWrapper : public CGraphicObjectInstance
{
  public:
    enum
    {
        ID = TREE_OBJECT
    };
    int GetType() const override
    {
        return ID;
    }

    // Collision Data
  protected:
    UINT GetCollisionObjectCount();
    void GetCollisionObject(UINT nIndex, CSpeedTreeRT::ECollisionObjectType &eType, float *pPosition,
                            float *pDimensions);
    void OnUpdateCollisionData(const CStaticCollisionDataVector *pscdVector) override;
    void OnUpdateHeighInstance(CAttributeInstance *pAttributeInstance) override
    {
    }
    bool OnGetObjectHeight(float fX, float fY, float *pfHeight) override
    {
        return false;
    }
    // Bounding Sphere
  public:
    bool GetBoundingSphere(D3DXVECTOR3 &v3Center, float &fRadius) override;

    static bool ms_bSelfShadowOn;

    // methods from CGraphicObjectInstance
    virtual void SetPosition(float x, float y, float z);
    virtual void CalculateBBox();
    void OnRender() override;
    void OnRenderPCBlocker() override;
    void OnBlendRender() override
    {
    }
    void OnRenderToShadowMap() override
    {
    }
    void OnRenderShadow() override
    {
    }

    CSpeedTreeWrapper();
    ~CSpeedTreeWrapper() override;

    // geometry
    bool LoadTree(const char *pszSptFile, const BYTE *c_pbBlock, unsigned int uiBlockSize, unsigned int nSeed = 1,
                  float fSize = -1.0f, float fSizeVariance = -1.0f);
    const float *GetBoundingBox() const
    {
        return m_afBoundingBox;
    }

    // rendering
    void SetupBranchForTreeType() const;
    void SetupFrondForTreeType() const;
    void SetupLeafForTreeType() const;
    void EndLeafForTreeType();
    void RenderBranches() const;
    void RenderFronds() const;
    void RenderLeaves() const;
    void RenderBillboards() const;

    // instancing
    CSpeedTreeWrapper **GetInstances(unsigned int &nCount);
    CSpeedTreeWrapper *InstanceOf() const
    {
        return m_pInstanceOf;
    }
    CSpeedTreeWrapper *MakeInstance();
    void DeleteInstance(CSpeedTreeWrapper *pInstance);
    CSpeedTreeRT *GetSpeedTree() const
    {
        return m_pSpeedTree;
    }

    // lighting
    const CSpeedTreeMaterial &GetBranchMaterial() const
    {
        return m_cBranchMaterial;
    }
    const CSpeedTreeMaterial &GetFrondMaterial() const
    {
        return m_cFrondMaterial;
    }
    const CSpeedTreeMaterial &GetLeafMaterial() const
    {
        return m_cLeafMaterial;
    }
    float GetLeafLightingAdjustment() const
    {
        return m_pSpeedTree->GetLeafLightingAdjustment();
    }

    // wind
    void Advance();

    // utility
    void CleanUpMemory();

  private:
    void SetupBuffers();
    void SetupBranchBuffers();
    void SetupFrondBuffers();
    void SetupLeafBuffers();
    void PositionTree() const;
    static bool LoadTexture(const char *pFilename, CGraphicImageInstance &rImage);
    void SetShaderConstants(const float *pMaterial) const;

    // SpeedTreeRT data
    CSpeedTreeRT *m_pSpeedTree;                    // the SpeedTree object
    CSpeedTreeRT::STextures *m_pTextureInfo;       // texture info cache
    bool m_bIsInstance = false;                    // is this an instance?
    std::vector<CSpeedTreeWrapper *> m_vInstances; // what is an instance of us
    CSpeedTreeWrapper *m_pInstanceOf;              // which tree is this an instance of

    // geometry cache
    CSpeedTreeRT::SGeometry *m_pGeometryCache; // cache for pulling geometry from SpeedTree avoids lots
                                               // of reallocation

    // branch buffers
    LPDIRECT3DVERTEXBUFFER9 m_pBranchVertexBuffer; // branch vertex buffer
    unsigned int m_unBranchVertexCount = 0;        // number of vertices in branches
    LPDIRECT3DINDEXBUFFER9 m_pBranchIndexBuffer;   // branch index buffer
    unsigned short *m_pBranchIndexCounts;          // number of indexes per branch LOD level

    // frond buffers
    LPDIRECT3DVERTEXBUFFER9 m_pFrondVertexBuffer; // frond vertex buffer
    unsigned int m_unFrondVertexCount = 0;        // number of vertices in frond
    unsigned int m_unNumFrondLods = 0;
    LPDIRECT3DINDEXBUFFER9 *m_pFrondIndexBuffers;
    unsigned short *m_pFrondIndexCounts;

    // leaf buffers
    unsigned short m_usNumLeafLods = 0;           // the number of leaf LODs
    LPDIRECT3DVERTEXBUFFER9 *m_pLeafVertexBuffer; // leaf vertex buffer
    bool *m_pLeavesUpdatedByCpu;                  // stores which LOD's have been updated already
                                                  // per frame

    // tree properties
    float m_afPos[3]{};         // tree position
    float m_afBoundingBox[6]{}; // tree bounding box

    // materials
    CSpeedTreeMaterial m_cBranchMaterial; // branch material
    CSpeedTreeMaterial m_cLeafMaterial;   // leaf material
    CSpeedTreeMaterial m_cFrondMaterial;  // frond material

    // branch texture
    CGraphicImageInstance m_BranchImageInstance;
    CGraphicImageInstance m_ShadowImageInstance; // shadow texture object (used
                                                 // if shadows are enabled)
    CGraphicImageInstance m_CompositeImageInstance;

    static unsigned int m_unNumWrappersActive;

    static LPDIRECT3DVERTEXSHADER9 ms_lpBranchVertexShader;
    static LPDIRECT3DVERTEXSHADER9 ms_lpLeafVertexShader;
};
