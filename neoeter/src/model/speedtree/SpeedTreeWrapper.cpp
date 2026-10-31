///////////////////////////////////////////////////////////////////////
//	CSpeedTreeWrapper Class
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
//	  Copyright (c) 2001-2003 IDV, Inc.
//	  All Rights Reserved.
//
//		IDV, Inc.
//		1233 Washington St. Suite 610
//		Columbia, SC 29201
//		Voice: (803) 799-1699
//		Fax:   (803) 931-0320
//		Web:   http://www.idvinc.com
//

///////////////////////////////////////////////////////////////////////
//	Include Files
#include <Neoeter/Model/SpeedTree/StdAfx.h>

#include <Neoeter/Platform/Base/Debug.h>
#include <Neoeter/Platform/Base/Filename.h>
#include <Neoeter/Platform/Base/Timer.h>
#include <Neoeter/Render/Camera.h>
#include <Neoeter/Render/ResourceManager.h>
#include <Neoeter/Render/StateManager.h>
#include <cstdio>
#include <cstdlib>

#include <Neoeter/Model/SpeedTree/CSpeedTreeDirectX.h>
#include <Neoeter/Model/SpeedTree/SpeedTreeConfig.h>
#include <Neoeter/Model/SpeedTree/SpeedTreeWrapper.h>
#include <Neoeter/Model/SpeedTree/VertexShaders.h>

using namespace std;

unsigned int CSpeedTreeWrapper::m_unNumWrappersActive = 0;
LPDIRECT3DVERTEXSHADER9 CSpeedTreeWrapper::ms_lpBranchVertexShader = NULL;
LPDIRECT3DVERTEXSHADER9 CSpeedTreeWrapper::ms_lpLeafVertexShader = NULL;
bool CSpeedTreeWrapper::ms_bSelfShadowOn = true;
#define AGBR2ARGB(dwColor)                                                     \
  (dwColor & 0xff00ff00) + ((dwColor & 0x00ff0000) >> 16) +                    \
      ((dwColor & 0x000000ff) << 16)

///////////////////////////////////////////////////////////////////////
//	CSpeedTreeWrapper::CSpeedTreeWrapper
CSpeedTreeWrapper::CSpeedTreeWrapper()
    : m_pSpeedTree(new CSpeedTreeRT), m_bIsInstance(false), m_pInstanceOf(NULL),
      m_pGeometryCache(NULL), m_usNumLeafLods(0), m_unNumFrondLods(0),
      m_pBranchIndexCounts(NULL), m_pBranchIndexBuffer(NULL),
      m_pBranchVertexBuffer(NULL), m_pFrondIndexCounts(NULL),
      m_pFrondIndexBuffers(NULL), m_pFrondVertexBuffer(NULL),
      m_pLeafVertexBuffer(NULL), m_pLeavesUpdatedByCpu(NULL),
      m_unBranchVertexCount(0), m_unFrondVertexCount(0), m_pTextureInfo(NULL) {
  // set initial position
  m_afPos[0] = m_afPos[1] = m_afPos[2] = 0.0f;
  m_unNumWrappersActive++;
}

void CSpeedTreeWrapper::OnRenderPCBlocker() {
  STATEMANAGER.SetTextureStageState(0, D3DTSS_COLORARG1, D3DTA_TEXTURE);
  STATEMANAGER.SetTextureStageState(0, D3DTSS_COLORARG2, D3DTA_DIFFUSE);
  STATEMANAGER.SetTextureStageState(0, D3DTSS_COLOROP, D3DTOP_MODULATE);
  STATEMANAGER.SetTextureStageState(0, D3DTSS_ALPHAARG1, D3DTA_TEXTURE);
  STATEMANAGER.SetTextureStageState(0, D3DTSS_ALPHAARG2, D3DTA_DIFFUSE);
  STATEMANAGER.SetTextureStageState(0, D3DTSS_ALPHAOP, D3DTOP_MODULATE);

  STATEMANAGER.SetTextureStageState(1, D3DTSS_ALPHAARG1, D3DTA_TEXTURE);
  STATEMANAGER.SetTextureStageState(1, D3DTSS_ALPHAARG2, D3DTA_CURRENT);
  STATEMANAGER.SetTextureStageState(1, D3DTSS_ALPHAOP, D3DTOP_MODULATE);

  DWORD dwLighting = STATEMANAGER.GetRenderState(D3DRS_LIGHTING);
  DWORD dwFogEnable = STATEMANAGER.GetRenderState(D3DRS_FOGENABLE);
  DWORD dwAlphaBlendEnable =
      STATEMANAGER.GetRenderState(D3DRS_ALPHABLENDENABLE);
  STATEMANAGER.SetRenderState(D3DRS_LIGHTING, FALSE);
  STATEMANAGER.SaveRenderState(D3DRS_COLORVERTEX, TRUE);
  STATEMANAGER.SetRenderState(D3DRS_ALPHABLENDENABLE, TRUE);
  STATEMANAGER.SaveRenderState(D3DRS_ALPHATESTENABLE, TRUE);
  STATEMANAGER.SaveRenderState(D3DRS_ALPHAFUNC, D3DCMP_GREATER);
  STATEMANAGER.SaveRenderState(D3DRS_CULLMODE, D3DCULL_CW);
  STATEMANAGER.SetRenderState(D3DRS_FOGENABLE, FALSE);
  STATEMANAGER.SetFVF(D3DFVF_SPEEDTREE_BRANCH_VERTEX);

  // choose fixed function pipeline or custom shader for fronds and branches
  STATEMANAGER.SetVertexShader(ms_lpBranchVertexShader);
  {
    LPDIRECT3DTEXTURE9 lpd3dTexture = NULL;
    if (!m_BranchImageInstance.IsEmpty())
      lpd3dTexture = m_BranchImageInstance.GetTextureReference().GetD3DTexture();
    if (lpd3dTexture)
      STATEMANAGER.SetTexture(0, lpd3dTexture);

    if (m_pGeometryCache->m_sBranches.m_usVertexCount > 0 &&
        m_pBranchVertexBuffer && m_pBranchIndexBuffer) {
      STATEMANAGER.SetStreamSource(0, m_pBranchVertexBuffer,
                                   sizeof(SFVFBranchVertex));
      STATEMANAGER.SetIndices(m_pBranchIndexBuffer, 0);
      RenderBranches();
    }
  }

  if (!m_CompositeImageInstance.IsEmpty())
    STATEMANAGER.SetTexture(
        0, m_CompositeImageInstance.GetTextureReference().GetD3DTexture());
  STATEMANAGER.SetRenderState(D3DRS_CULLMODE, D3DCULL_NONE);
  {
    if (m_pGeometryCache->m_sFronds.m_usVertexCount > 0 &&
        m_pGeometryCache->m_sFronds.m_nDiscreteLodLevel > -1 &&
        m_pFrondIndexCounts[m_pGeometryCache->m_sFronds.m_nDiscreteLodLevel] >
            0) {
      if (!m_CompositeImageInstance.IsEmpty())
        STATEMANAGER.SetTexture(
            0, m_CompositeImageInstance.GetTextureReference().GetD3DTexture());

      STATEMANAGER.SetStreamSource(0, m_pFrondVertexBuffer,
                                   sizeof(SFVFBranchVertex));
      STATEMANAGER.SetIndices(
          m_pFrondIndexBuffers[m_pGeometryCache->m_sFronds.m_nDiscreteLodLevel],
          0);
      RenderFronds();
    }
  }
  {
    STATEMANAGER.SetFVF(D3DFVF_SPEEDTREE_LEAF_VERTEX);
    STATEMANAGER.SetVertexShader(ms_lpLeafVertexShader);

    if (!m_CompositeImageInstance.IsEmpty())
      STATEMANAGER.SetTexture(
          0, m_CompositeImageInstance.GetTextureReference().GetD3DTexture());

    RenderLeaves();
    EndLeafForTreeType();
  }

  STATEMANAGER.SetVertexShader(NULL);
  STATEMANAGER.SetFVF(D3DFVF_SPEEDTREE_BILLBOARD_VERTEX);
  STATEMANAGER.SetRenderState(D3DRS_LIGHTING, FALSE);
  STATEMANAGER.SetRenderState(D3DRS_COLORVERTEX, FALSE);
  RenderBillboards();

  STATEMANAGER.RestoreRenderState(D3DRS_COLORVERTEX);
  STATEMANAGER.RestoreRenderState(D3DRS_CULLMODE);
  STATEMANAGER.RestoreRenderState(D3DRS_ALPHATESTENABLE);
  STATEMANAGER.RestoreRenderState(D3DRS_ALPHAFUNC);
  STATEMANAGER.SetRenderState(D3DRS_ALPHABLENDENABLE, dwAlphaBlendEnable);
  STATEMANAGER.SetRenderState(D3DRS_LIGHTING, dwLighting);
  STATEMANAGER.SetRenderState(D3DRS_FOGENABLE, dwFogEnable);

  STATEMANAGER.SetTextureStageState(1, D3DTSS_ALPHAOP, D3DTOP_SELECTARG1);
}

void CSpeedTreeWrapper::OnRender() {
  STATEMANAGER.SetTextureStageState(0, D3DTSS_COLORARG1, D3DTA_TEXTURE);
  STATEMANAGER.SetTextureStageState(0, D3DTSS_COLORARG2, D3DTA_DIFFUSE);
  STATEMANAGER.SetTextureStageState(0, D3DTSS_COLOROP, D3DTOP_MODULATE);
  STATEMANAGER.SetTextureStageState(0, D3DTSS_ALPHAARG1, D3DTA_TEXTURE);
  STATEMANAGER.SetTextureStageState(0, D3DTSS_ALPHAARG2, D3DTA_DIFFUSE);
  STATEMANAGER.SetTextureStageState(0, D3DTSS_ALPHAOP, D3DTOP_MODULATE);

  STATEMANAGER.SetTextureStageState(1, D3DTSS_COLOROP, D3DTOP_MODULATE);
  STATEMANAGER.SetTextureStageState(1, D3DTSS_COLORARG1, D3DTA_TEXTURE);
  STATEMANAGER.SetTextureStageState(1, D3DTSS_COLORARG2, D3DTA_CURRENT);
  STATEMANAGER.SetSamplerState(1, D3DSAMP_ADDRESSU, D3DTADDRESS_WRAP);
  STATEMANAGER.SetSamplerState(1, D3DSAMP_ADDRESSV, D3DTADDRESS_WRAP);

  STATEMANAGER.SaveRenderState(D3DRS_LIGHTING, FALSE);
  STATEMANAGER.SaveRenderState(D3DRS_COLORVERTEX, TRUE);
  STATEMANAGER.SaveRenderState(D3DRS_ALPHATESTENABLE, TRUE);
  STATEMANAGER.SaveRenderState(D3DRS_ALPHAFUNC, D3DCMP_GREATER);
  STATEMANAGER.SaveRenderState(D3DRS_CULLMODE, D3DCULL_CW);
  STATEMANAGER.SaveRenderState(D3DRS_FOGENABLE, FALSE);
  STATEMANAGER.SetFVF(D3DFVF_SPEEDTREE_BRANCH_VERTEX);

  // choose fixed function pipeline or custom shader for fronds and branches
  STATEMANAGER.SetVertexShader(ms_lpBranchVertexShader);

  SetupBranchForTreeType();
  RenderBranches();

  if (!m_CompositeImageInstance.IsEmpty())
    STATEMANAGER.SetTexture(
        0, m_CompositeImageInstance.GetTextureReference().GetD3DTexture());
  STATEMANAGER.SetRenderState(D3DRS_CULLMODE, D3DCULL_NONE);

  SetupFrondForTreeType();
  RenderFronds();

  STATEMANAGER.SetFVF(D3DFVF_SPEEDTREE_LEAF_VERTEX);
  STATEMANAGER.SetVertexShader(ms_lpLeafVertexShader);

  SetupLeafForTreeType();
  RenderLeaves();
  EndLeafForTreeType();

  STATEMANAGER.SetVertexShader(NULL);
  STATEMANAGER.SetFVF(D3DFVF_SPEEDTREE_BILLBOARD_VERTEX);
  STATEMANAGER.SetRenderState(D3DRS_LIGHTING, FALSE);
  STATEMANAGER.SetRenderState(D3DRS_COLORVERTEX, FALSE);
  RenderBillboards();

  STATEMANAGER.RestoreRenderState(D3DRS_LIGHTING);
  STATEMANAGER.RestoreRenderState(D3DRS_COLORVERTEX);
  STATEMANAGER.RestoreRenderState(D3DRS_ALPHATESTENABLE);
  STATEMANAGER.RestoreRenderState(D3DRS_ALPHAFUNC);
  STATEMANAGER.RestoreRenderState(D3DRS_CULLMODE);
  STATEMANAGER.RestoreRenderState(D3DRS_FOGENABLE);
}

UINT CSpeedTreeWrapper::GetCollisionObjectCount() {
  assert(m_pSpeedTree);
  return m_pSpeedTree->GetCollisionObjectCount();
}

void CSpeedTreeWrapper::GetCollisionObject(
    UINT nIndex, CSpeedTreeRT::ECollisionObjectType &eType, float *pPosition,
    float *pDimensions) {
  assert(m_pSpeedTree);
  m_pSpeedTree->GetCollisionObject(nIndex, eType, pPosition, pDimensions);
}

void CSpeedTreeWrapper::OnUpdateCollisionData(
    const CStaticCollisionDataVector *) {
  D3DXMATRIX mat;
  D3DXMatrixTranslation(&mat, m_afPos[0], m_afPos[1], m_afPos[2]);
  for (UINT i = 0; i < GetCollisionObjectCount(); ++i) {
    CSpeedTreeRT::ECollisionObjectType ObjectType;
    CStaticCollisionData CollisionData;

    GetCollisionObject(i, ObjectType, (float *)&CollisionData.v3Position,
                       CollisionData.fDimensions);

    if (ObjectType == CSpeedTreeRT::CO_BOX)
      continue;

    switch (ObjectType) {
    case CSpeedTreeRT::CO_SPHERE:
      CollisionData.dwType = COLLISION_TYPE_SPHERE;
      CollisionData.fDimensions[0] = CollisionData.fDimensions[0];
      break;

    case CSpeedTreeRT::CO_CYLINDER:
      CollisionData.dwType = COLLISION_TYPE_CYLINDER;
      CollisionData.fDimensions[0] = CollisionData.fDimensions[0];
      CollisionData.fDimensions[1] = CollisionData.fDimensions[1];
      break;

    case CSpeedTreeRT::CO_BOX:
    default:
      break;
    }
    AddCollision(&CollisionData, &mat);
  }
}

bool CSpeedTreeWrapper::GetBoundingSphere(D3DXVECTOR3 &v3Center,
                                          float &fRadius) {
  float fX, fY, fZ;
  fX = m_afBoundingBox[3] - m_afBoundingBox[0];
  fY = m_afBoundingBox[4] - m_afBoundingBox[1];
  fZ = m_afBoundingBox[5] - m_afBoundingBox[2];
  v3Center.x = 0.0f;
  v3Center.y = 0.0f;
  v3Center.z = fZ * 0.5f;
  fRadius = sqrtf(fX * fX + fY * fY + fZ * fZ) * 0.5f * 0.9f;
  D3DXVECTOR3 vec = m_pSpeedTree->GetTreePosition();
  v3Center += vec;
  return true;
}

void CSpeedTreeWrapper::CalculateBBox() {
  float fX, fY, fZ;

  fX = m_afBoundingBox[3] - m_afBoundingBox[0];
  fY = m_afBoundingBox[4] - m_afBoundingBox[1];
  fZ = m_afBoundingBox[5] - m_afBoundingBox[2];

  m_v3BBoxMin.x = -fX / 2.0f;
  m_v3BBoxMin.y = -fY / 2.0f;
  m_v3BBoxMin.z = 0.0f;
  m_v3BBoxMax.x = fX / 2.0f;
  m_v3BBoxMax.y = fY / 2.0f;
  m_v3BBoxMax.z = fZ;

  m_v4TBBox[0] = D3DXVECTOR4(m_v3BBoxMin.x, m_v3BBoxMin.y, m_v3BBoxMin.z, 1.0f);
  m_v4TBBox[1] = D3DXVECTOR4(m_v3BBoxMin.x, m_v3BBoxMax.y, m_v3BBoxMin.z, 1.0f);
  m_v4TBBox[2] = D3DXVECTOR4(m_v3BBoxMax.x, m_v3BBoxMin.y, m_v3BBoxMin.z, 1.0f);
  m_v4TBBox[3] = D3DXVECTOR4(m_v3BBoxMax.x, m_v3BBoxMax.y, m_v3BBoxMin.z, 1.0f);
  m_v4TBBox[4] = D3DXVECTOR4(m_v3BBoxMin.x, m_v3BBoxMin.y, m_v3BBoxMax.z, 1.0f);
  m_v4TBBox[5] = D3DXVECTOR4(m_v3BBoxMin.x, m_v3BBoxMax.y, m_v3BBoxMax.z, 1.0f);
  m_v4TBBox[6] = D3DXVECTOR4(m_v3BBoxMax.x, m_v3BBoxMin.y, m_v3BBoxMax.z, 1.0f);
  m_v4TBBox[7] = D3DXVECTOR4(m_v3BBoxMax.x, m_v3BBoxMax.y, m_v3BBoxMax.z, 1.0f);

  const D3DXMATRIX &c_rmatTransform = GetTransform();

  for (DWORD i = 0; i < 8; ++i) {
    D3DXVec4Transform(&m_v4TBBox[i], &m_v4TBBox[i], &c_rmatTransform);
    if (0 == i) {
      m_v3TBBoxMin.x = m_v4TBBox[i].x;
      m_v3TBBoxMin.y = m_v4TBBox[i].y;
      m_v3TBBoxMin.z = m_v4TBBox[i].z;
      m_v3TBBoxMax.x = m_v4TBBox[i].x;
      m_v3TBBoxMax.y = m_v4TBBox[i].y;
      m_v3TBBoxMax.z = m_v4TBBox[i].z;
    } else {
      if (m_v3TBBoxMin.x > m_v4TBBox[i].x)
        m_v3TBBoxMin.x = m_v4TBBox[i].x;
      if (m_v3TBBoxMax.x < m_v4TBBox[i].x)
        m_v3TBBoxMax.x = m_v4TBBox[i].x;
      if (m_v3TBBoxMin.y > m_v4TBBox[i].y)
        m_v3TBBoxMin.y = m_v4TBBox[i].y;
      if (m_v3TBBoxMax.y < m_v4TBBox[i].y)
        m_v3TBBoxMax.y = m_v4TBBox[i].y;
      if (m_v3TBBoxMin.z > m_v4TBBox[i].z)
        m_v3TBBoxMin.z = m_v4TBBox[i].z;
      if (m_v3TBBoxMax.z < m_v4TBBox[i].z)
        m_v3TBBoxMax.z = m_v4TBBox[i].z;
    }
  }
}

///////////////////////////////////////////////////////////////////////
//	CSpeedTreeWrapper::~CSpeedTreeWrapper

CSpeedTreeWrapper::~CSpeedTreeWrapper() {
  // if this is not an instance, clean up
  if (!m_bIsInstance) {
    if (m_unBranchVertexCount > 0) {
      SAFE_RELEASE(m_pBranchVertexBuffer);
      SAFE_RELEASE(m_pBranchIndexBuffer);
      SAFE_DELETE_ARRAY(m_pBranchIndexCounts);
    }

    if (m_unFrondVertexCount > 0) {
      SAFE_RELEASE(m_pFrondVertexBuffer);

      for (unsigned int i = 0; i < m_unNumFrondLods; ++i)
        if (m_pFrondIndexCounts[i] > 0)
          SAFE_RELEASE(m_pFrondIndexBuffers[i]);

      SAFE_DELETE_ARRAY(m_pFrondIndexBuffers);
      SAFE_DELETE_ARRAY(m_pFrondIndexCounts);
    }

    for (unsigned short i = 0; i < m_usNumLeafLods; ++i) {
      m_pSpeedTree->GetGeometry(*m_pGeometryCache, SpeedTree_LeafGeometry, -1,
                                -1, i);

      if (m_pGeometryCache->m_sLeaves0.m_usLeafCount > 0)
        SAFE_RELEASE(m_pLeafVertexBuffer[i]);
    }

    SAFE_DELETE_ARRAY(m_pLeavesUpdatedByCpu);
    SAFE_DELETE_ARRAY(m_pLeafVertexBuffer);

    SAFE_DELETE(m_pTextureInfo);

    SAFE_DELETE(m_pGeometryCache);
  }

  // always delete the speedtree
  SAFE_DELETE(m_pSpeedTree);

  --m_unNumWrappersActive;

  Clear();
}

///////////////////////////////////////////////////////////////////////
//	CSpeedTreeWrapper::LoadTree
bool CSpeedTreeWrapper::LoadTree(const char *pszSptFile,
                                 const BYTE *c_pbBlock = NULL,
                                 unsigned int uiBlockSize = 0,
                                 unsigned int nSeed, float fSize,
                                 float fSizeVariance) {
  bool bSuccess = false;

  // directx, so allow for flipping of the texture coordinate
#ifdef WRAPPER_FLIP_T_TEXCOORD
  m_pSpeedTree->SetTextureFlip(true);
#endif

  // load the tree file
  if (!m_pSpeedTree->LoadTree(c_pbBlock, uiBlockSize)) {
    if (!m_pSpeedTree->LoadTree(pszSptFile)) {
      TraceError("SpeedTreeRT Error: %s", CSpeedTreeRT::GetCurrentError());
      return false;
    }
  }

  // override the lighting method stored in the spt file
  m_pSpeedTree->SetBranchLightingMethod(CSpeedTreeRT::LIGHT_STATIC);
  m_pSpeedTree->SetLeafLightingMethod(CSpeedTreeRT::LIGHT_STATIC);
  m_pSpeedTree->SetFrondLightingMethod(CSpeedTreeRT::LIGHT_STATIC);

  // set the wind method
#ifdef WRAPPER_USE_NO_WIND
  m_pSpeedTree->SetBranchWindMethod(CSpeedTreeRT::WIND_NONE);
  m_pSpeedTree->SetLeafWindMethod(CSpeedTreeRT::WIND_NONE);
  m_pSpeedTree->SetFrondWindMethod(CSpeedTreeRT::WIND_NONE);
#endif

  m_pSpeedTree->SetNumLeafRockingGroups(1);

  // override the size, if necessary
  if (fSize >= 0.0f && fSizeVariance >= 0.0f)
    m_pSpeedTree->SetTreeSize(fSize, fSizeVariance);

  // generate tree geometry
  if (m_pSpeedTree->Compute(NULL, nSeed)) {
    // get the dimensions
    m_pSpeedTree->GetBoundingBox(m_afBoundingBox);

    // make the leaves rock in the wind
    m_pSpeedTree->SetLeafRockingState(true);

    // billboard setup
    CSpeedTreeRT::SetDropToBillboard(true);

    // query & set materials
    m_cBranchMaterial.Set(m_pSpeedTree->GetBranchMaterial());
    m_cFrondMaterial.Set(m_pSpeedTree->GetFrondMaterial());
    m_cLeafMaterial.Set(m_pSpeedTree->GetLeafMaterial());

    // adjust lod distances
    float fHeight = m_afBoundingBox[5] - m_afBoundingBox[2];
    m_pTextureInfo = new CSpeedTreeRT::STextures;
    m_pSpeedTree->GetTextures(*m_pTextureInfo);
    string strSptFile = pszSptFile ? pszSptFile : "";
    string strPath = CFileNameHelper::GetPath(strSptFile);

    if (m_pTextureInfo->m_pBranchTextureFilename) {
      string strTexture = m_pTextureInfo->m_pBranchTextureFilename;
      LoadTexture(
          (strPath + CFileNameHelper::NoExtension(strTexture) + ".dds").c_str(),
          m_BranchImageInstance);
    }

    if (m_pTextureInfo->m_pSelfShadowFilename != NULL) {
      string strTexture = m_pTextureInfo->m_pSelfShadowFilename;
      LoadTexture(
          (strPath + CFileNameHelper::NoExtension(strTexture) + ".dds").c_str(),
          m_ShadowImageInstance);
    }

    if (m_pTextureInfo->m_pCompositeFilename) {
      string strTexture = m_pTextureInfo->m_pCompositeFilename;
      LoadTexture(
          (strPath + CFileNameHelper::NoExtension(strTexture) + ".dds").c_str(),
          m_CompositeImageInstance);
    }

    // setup the index and vertex buffers
    SetupBuffers();

    // everything appeared to go well
    bSuccess = true;
  } else // tree failed to compute
    fprintf(stderr, "\nFatal Error, cannot compute tree [%s]\n\n",
            CSpeedTreeRT::GetCurrentError());

  return bSuccess;
}

///////////////////////////////////////////////////////////////////////
//	CSpeedTreeWrapper::SetupBuffers

void CSpeedTreeWrapper::SetupBuffers(void) {
  // read all the geometry for highest LOD into the geometry cache (just a
  // precaution, it's updated later)
  if (m_pGeometryCache == NULL)
    m_pGeometryCache = new CSpeedTreeRT::SGeometry;

  m_pSpeedTree->GetGeometry(*m_pGeometryCache);

  // setup the buffers for each part
  SetupBranchBuffers();
  SetupFrondBuffers();
  SetupLeafBuffers();
}

///////////////////////////////////////////////////////////////////////
//	CSpeedTreeWrapper::SetupBranchBuffers

void CSpeedTreeWrapper::SetupBranchBuffers(void) {
  // reference to branch structure
  CSpeedTreeRT::SGeometry::SIndexed *pBranches =
      &(m_pGeometryCache->m_sBranches);
  m_unBranchVertexCount =
      pBranches->m_usVertexCount; // we asked for a contiguous strip

  // check if this tree has branches
  if (m_unBranchVertexCount > 1) {
    // create the vertex buffer for storing branch vertices
    SFVFBranchVertex *pVertexBuffer = NULL;

    ms_lpd3dDevice->CreateVertexBuffer(
        m_unBranchVertexCount * sizeof(SFVFBranchVertex), D3DUSAGE_WRITEONLY,
        D3DFVF_SPEEDTREE_BRANCH_VERTEX, D3DPOOL_MANAGED, &m_pBranchVertexBuffer,
        NULL);
    // fill the vertex buffer by interleaving SpeedTree data
    m_pBranchVertexBuffer->Lock(0, 0, reinterpret_cast<void **>(&pVertexBuffer),
                                0);

    for (unsigned int i = 0; i < m_unBranchVertexCount; ++i) {
      // position
      memcpy(&pVertexBuffer->m_vPosition, &(pBranches->m_pCoords[i * 3]),
             3 * sizeof(float));

      // normal or color
      pVertexBuffer->m_dwDiffuseColor = AGBR2ARGB(pBranches->m_pColors[i]);

      // texcoords for layer 0
      pVertexBuffer->m_fShadowCoords[0] = pBranches->m_pTexCoords1[i * 2];
      pVertexBuffer->m_fShadowCoords[1] = pBranches->m_pTexCoords1[i * 2 + 1];

      // texcoords for layer 1 (if enabled)
      pVertexBuffer->m_fTexCoords[0] = pBranches->m_pTexCoords0[i * 2];
      pVertexBuffer->m_fTexCoords[1] = pBranches->m_pTexCoords0[i * 2 + 1];

      ++pVertexBuffer;
    }
    m_pBranchVertexBuffer->Unlock();

    unsigned int unNumLodLevels = m_pSpeedTree->GetNumBranchLodLevels();
    m_pBranchIndexCounts = new unsigned short[unNumLodLevels];
    for (unsigned int i = 0; i < unNumLodLevels; ++i) {
      // force update for particular LOD
      m_pSpeedTree->GetGeometry(*m_pGeometryCache, SpeedTree_BranchGeometry,
                                (short)i);

      // check if this LOD has branches
      if (pBranches->m_usNumStrips > 0)
        m_pBranchIndexCounts[i] = pBranches->m_pStripLengths[0];
      else
        m_pBranchIndexCounts[i] = 0;
    }
    // set back to highest LOD
    m_pSpeedTree->GetGeometry(*m_pGeometryCache, SpeedTree_BranchGeometry, 0);

    // the first LOD level contains the most indices of all the levels, so
    // we use its size to allocate the index buffer
    ms_lpd3dDevice->CreateIndexBuffer(
        m_pBranchIndexCounts[0] * sizeof(unsigned short), D3DUSAGE_WRITEONLY,
        D3DFMT_INDEX16, D3DPOOL_MANAGED, &m_pBranchIndexBuffer, NULL);

    // fill the index buffer
    unsigned short *pIndexBuffer = NULL;
    m_pBranchIndexBuffer->Lock(0, 0, reinterpret_cast<void **>(&pIndexBuffer),
                               0);
    memcpy(pIndexBuffer, pBranches->m_pStrips[0],
           pBranches->m_pStripLengths[0] * sizeof(unsigned short));
    m_pBranchIndexBuffer->Unlock();
  }
}

///////////////////////////////////////////////////////////////////////
//	CSpeedTreeWrapper::SetupFrondBuffers

void CSpeedTreeWrapper::SetupFrondBuffers(void) {
  // reference to frond structure
  CSpeedTreeRT::SGeometry::SIndexed *pFronds = &(m_pGeometryCache->m_sFronds);
  m_unFrondVertexCount =
      pFronds->m_usVertexCount; // we asked for a contiguous strip

  // check if tree has fronds
  if (m_unFrondVertexCount > 1) {
    // create the vertex buffer for storing frond vertices
    SFVFBranchVertex *pVertexBuffer = NULL;
    ms_lpd3dDevice->CreateVertexBuffer(
        m_unFrondVertexCount * sizeof(SFVFBranchVertex),
        D3DUSAGE_DYNAMIC | D3DUSAGE_WRITEONLY, D3DFVF_SPEEDTREE_BRANCH_VERTEX,
        D3DPOOL_SYSTEMMEM, &m_pFrondVertexBuffer, NULL);
    // fill the vertex buffer by interleaving SpeedTree data
    m_pFrondVertexBuffer->Lock(0, 0, reinterpret_cast<void **>(&pVertexBuffer),
                               D3DLOCK_DISCARD | D3DLOCK_NOSYSLOCK);

    for (unsigned short i = 0; i < m_unFrondVertexCount; ++i) {
      // position
      memcpy(&pVertexBuffer->m_vPosition, &(pFronds->m_pCoords[i * 3]),
             3 * sizeof(float));

      // normal or color
      pVertexBuffer->m_dwDiffuseColor = AGBR2ARGB(pFronds->m_pColors[i]);

      // texcoords for layer 0
      pVertexBuffer->m_fShadowCoords[0] = pFronds->m_pTexCoords1[i * 2];
      pVertexBuffer->m_fShadowCoords[1] = pFronds->m_pTexCoords1[i * 2 + 1];

      // texcoords for layer 1 (if enabled)
      pVertexBuffer->m_fTexCoords[0] = pFronds->m_pTexCoords0[i * 2];
      pVertexBuffer->m_fTexCoords[1] = pFronds->m_pTexCoords0[i * 2 + 1];

      ++pVertexBuffer;
    }
    m_pFrondVertexBuffer->Unlock();

    // create and fill the index counts for each LOD
    m_unNumFrondLods = m_pSpeedTree->GetNumFrondLodLevels();
    m_pFrondIndexCounts = new unsigned short[m_unNumFrondLods];
    m_pFrondIndexBuffers = new LPDIRECT3DINDEXBUFFER9[m_unNumFrondLods];

    for (unsigned short i = 0; i < m_unNumFrondLods; ++i) {
      // force update for this LOD
      m_pSpeedTree->GetGeometry(*m_pGeometryCache, SpeedTree_FrondGeometry, -1,
                                i);

      // check if this LOD has fronds
      if (pFronds->m_usNumStrips > 0)
        m_pFrondIndexCounts[i] = pFronds->m_pStripLengths[0];
      else
        m_pFrondIndexCounts[i] = 0;

      if (m_pFrondIndexCounts[i] > 0) {
        // the first LOD level contains the most indices of all the levels, so
        // we use its size to allocate the index buffer
        ms_lpd3dDevice->CreateIndexBuffer(
            m_pFrondIndexCounts[i] * sizeof(unsigned short), D3DUSAGE_WRITEONLY,
            D3DFMT_INDEX16, D3DPOOL_MANAGED, &m_pFrondIndexBuffers[i], NULL);

        // fill the index buffer
        unsigned short *pIndexBuffer = NULL;
        m_pFrondIndexBuffers[i]->Lock(
            0, 0, reinterpret_cast<void **>(&pIndexBuffer), 0);
        memcpy(pIndexBuffer, pFronds->m_pStrips[0],
               m_pFrondIndexCounts[i] * sizeof(unsigned short));
        m_pFrondIndexBuffers[i]->Unlock();
      }
    }

    m_pSpeedTree->GetGeometry(*m_pGeometryCache, SpeedTree_FrondGeometry, -1,
                              0);
  }
}

///////////////////////////////////////////////////////////////////////
//	CSpeedTreeWrapper::SetupLeafBuffers

void CSpeedTreeWrapper::SetupLeafBuffers(void) {
  // set up constants
  const short anVertexIndices[6] = {0, 1, 2, 0, 2, 3};
  // const int nNumLeafMaps = m_pTextureInfo->m_uiLeafTextureCount;

  // set up the leaf counts for each LOD
  m_usNumLeafLods = m_pSpeedTree->GetNumLeafLodLevels();

  // create array of vertex buffers (one for each LOD)
  m_pLeafVertexBuffer = new LPDIRECT3DVERTEXBUFFER9[m_usNumLeafLods];

  // create array of bools for CPU updating (so we don't update for each
  // instance)
  m_pLeavesUpdatedByCpu = new bool[m_usNumLeafLods];

  // cycle through LODs
  for (unsigned int unLod = 0; unLod < m_usNumLeafLods; ++unLod) {
    m_pSpeedTree->GetGeometry(*m_pGeometryCache, SpeedTree_LeafGeometry, -1, -1,
                              unLod);
    m_pLeavesUpdatedByCpu[unLod] = false;

    // if this LOD has no leaves, skip it
    unsigned short usLeafCount = m_pGeometryCache->m_sLeaves0.m_usLeafCount;

    if (usLeafCount < 1)
      continue;

    SFVFLeafVertex *pVertexBuffer = NULL;
    // create the vertex buffer for storing leaf vertices
    ms_lpd3dDevice->CreateVertexBuffer(
        usLeafCount * 6 * sizeof(SFVFLeafVertex), D3DUSAGE_DYNAMIC,
        D3DFVF_SPEEDTREE_LEAF_VERTEX, D3DPOOL_SYSTEMMEM,
        &m_pLeafVertexBuffer[unLod], NULL);
    // fill the vertex buffer by interleaving SpeedTree data
    m_pLeafVertexBuffer[unLod]->Lock(0, 0,
                                     reinterpret_cast<void **>(&pVertexBuffer),
                                     D3DLOCK_DISCARD | D3DLOCK_NOSYSLOCK);
    SFVFLeafVertex *pVertex = pVertexBuffer;
    for (unsigned int unLeaf = 0; unLeaf < usLeafCount; ++unLeaf) {
      const CSpeedTreeRT::SGeometry::SLeaf *pLeaf =
          &(m_pGeometryCache->m_sLeaves0);
      for (unsigned int unVert = 0; unVert < 6;
           ++unVert) // 6 verts == 2 triangles
      {
        // position
        memcpy(pVertex->m_vPosition, &(pLeaf->m_pCenterCoords[unLeaf * 3]),
               3 * sizeof(float));

        // color
        pVertex->m_dwDiffuseColor = AGBR2ARGB(pLeaf->m_pColors[unLeaf]);

        // tex coord
        memcpy(
            pVertex->m_fTexCoords,
            &(pLeaf->m_pLeafMapTexCoords[unLeaf][anVertexIndices[unVert] * 2]),
            2 * sizeof(float));

        ++pVertex;
      }
    }
    m_pLeafVertexBuffer[unLod]->Unlock();
  }
}

///////////////////////////////////////////////////////////////////////
//	CSpeedTreeWrapper::Advance

void CSpeedTreeWrapper::Advance(void) {}

///////////////////////////////////////////////////////////////////////
//	CSpeedTreeWrapper::MakeInstance
CSpeedTreeWrapper *CSpeedTreeWrapper::MakeInstance(void) {
  CSpeedTreeWrapper *pInstance = new CSpeedTreeWrapper;

  // make an instance of this object's SpeedTree
  pInstance->m_bIsInstance = true;
  pInstance->m_pSpeedTree = m_pSpeedTree->MakeInstance();

  if (pInstance->m_pSpeedTree) {
    // use the same materials
    pInstance->m_cBranchMaterial = m_cBranchMaterial;
    pInstance->m_cLeafMaterial = m_cLeafMaterial;
    pInstance->m_cFrondMaterial = m_cFrondMaterial;
    pInstance->m_CompositeImageInstance.SetImagePointer(
        m_CompositeImageInstance.GetGraphicImagePointer());
    pInstance->m_BranchImageInstance.SetImagePointer(
        m_BranchImageInstance.GetGraphicImagePointer());

    if (!m_ShadowImageInstance.IsEmpty())
      pInstance->m_ShadowImageInstance.SetImagePointer(
          m_ShadowImageInstance.GetGraphicImagePointer());

    pInstance->m_pTextureInfo = m_pTextureInfo;

    // use the same geometry cache
    pInstance->m_pGeometryCache = m_pGeometryCache;

    // use the same buffers
    pInstance->m_pBranchIndexBuffer = m_pBranchIndexBuffer;
    pInstance->m_pBranchIndexCounts = m_pBranchIndexCounts;
    pInstance->m_pBranchVertexBuffer = m_pBranchVertexBuffer;
    pInstance->m_unBranchVertexCount = m_unBranchVertexCount;

    pInstance->m_pFrondIndexBuffers = m_pFrondIndexBuffers;
    pInstance->m_unNumFrondLods = m_unNumFrondLods;
    pInstance->m_pFrondIndexCounts = m_pFrondIndexCounts;
    pInstance->m_pFrondVertexBuffer = m_pFrondVertexBuffer;
    pInstance->m_unFrondVertexCount = m_unFrondVertexCount;

    pInstance->m_pLeafVertexBuffer = m_pLeafVertexBuffer;
    pInstance->m_usNumLeafLods = m_usNumLeafLods;
    pInstance->m_pLeavesUpdatedByCpu = m_pLeavesUpdatedByCpu;

    // new stuff
    memcpy(pInstance->m_afPos, m_afPos, 3 * sizeof(float));
    memcpy(pInstance->m_afBoundingBox, m_afBoundingBox, 6 * sizeof(float));
    pInstance->m_pInstanceOf = this;
    m_vInstances.push_back(pInstance);
  } else {
    fprintf(stderr, "SpeedTreeRT Error: %s\n", m_pSpeedTree->GetCurrentError());
    delete pInstance;
    pInstance = NULL;
  }

  return pInstance;
}

///////////////////////////////////////////////////////////////////////
//	CSpeedTreeWrapper::GetInstances

CSpeedTreeWrapper **CSpeedTreeWrapper::GetInstances(unsigned int &nCount) {
  nCount = m_vInstances.size();
  return nCount ? &(m_vInstances[0]) : NULL;
}

void CSpeedTreeWrapper::DeleteInstance(CSpeedTreeWrapper *pInstance) {
  std::vector<CSpeedTreeWrapper *>::iterator itor = m_vInstances.begin();

  while (itor != m_vInstances.end()) {
    if (*itor == pInstance) {
      delete pInstance;
      itor = m_vInstances.erase(itor);
    } else
      ++itor;
  }
}

///////////////////////////////////////////////////////////////////////
//	CSpeedTreeWrapper::SetupBranchForTreeType

void CSpeedTreeWrapper::SetupBranchForTreeType(void) const {
  LPDIRECT3DTEXTURE9 lpd3dTexture = NULL;

  // set texture map
  if (!m_BranchImageInstance.IsEmpty())
    lpd3dTexture = m_BranchImageInstance.GetTextureReference().GetD3DTexture();
  if (lpd3dTexture)
    STATEMANAGER.SetTexture(0, lpd3dTexture);

  // bind shadow texture
  if (ms_bSelfShadowOn && !m_ShadowImageInstance.IsEmpty())
    lpd3dTexture = m_ShadowImageInstance.GetTextureReference().GetD3DTexture();
  else
    lpd3dTexture = NULL;

  if (lpd3dTexture)
    STATEMANAGER.SetTexture(1, lpd3dTexture);
  else
    STATEMANAGER.SetTexture(1, NULL);

  if (m_pGeometryCache->m_sBranches.m_usVertexCount > 0) {
    // activate the branch vertex buffer
    STATEMANAGER.SetStreamSource(0, m_pBranchVertexBuffer,
                                 sizeof(SFVFBranchVertex));
    // set the index buffer
    STATEMANAGER.SetIndices(m_pBranchIndexBuffer, 0);
  }
}

///////////////////////////////////////////////////////////////////////
//	CSpeedTreeWrapper::RenderBranches

void CSpeedTreeWrapper::RenderBranches(void) const {
  m_pSpeedTree->GetGeometry(*m_pGeometryCache, SpeedTree_BranchGeometry);

  if (m_pGeometryCache->m_fBranchAlphaTestValue > 0.0f) {
    PositionTree();

    // set alpha test value
    STATEMANAGER.SetRenderState(
        D3DRS_ALPHAREF, DWORD(m_pGeometryCache->m_fBranchAlphaTestValue));

    // render if this LOD has branches
    if (m_pBranchIndexCounts &&
        m_pGeometryCache->m_sBranches.m_nDiscreteLodLevel > -1 &&
        m_pBranchIndexCounts[m_pGeometryCache->m_sBranches
                                 .m_nDiscreteLodLevel] > 0) {
      ms_faceCount += m_pBranchIndexCounts[m_pGeometryCache->m_sBranches
                                               .m_nDiscreteLodLevel] -
                      2;
      STATEMANAGER.DrawIndexedPrimitive(
          D3DPT_TRIANGLESTRIP, 0, m_pGeometryCache->m_sBranches.m_usVertexCount,
          0,
          m_pBranchIndexCounts[m_pGeometryCache->m_sBranches
                                   .m_nDiscreteLodLevel] -
              2);
    }
  }
}

///////////////////////////////////////////////////////////////////////
//	CSpeedTreeWrapper::SetupFrondForTreeType

void CSpeedTreeWrapper::SetupFrondForTreeType(void) const {
  // bind shadow texture
  LPDIRECT3DTEXTURE9 lpd3dTexture = NULL;

  if (!m_CompositeImageInstance.IsEmpty())
    lpd3dTexture = m_CompositeImageInstance.GetTextureReference().GetD3DTexture();
  if (lpd3dTexture)
    STATEMANAGER.SetTexture(0, lpd3dTexture);

  lpd3dTexture = NULL;
  if (!m_ShadowImageInstance.IsEmpty())
    lpd3dTexture = m_ShadowImageInstance.GetTextureReference().GetD3DTexture();
  if (lpd3dTexture)
    STATEMANAGER.SetTexture(1, lpd3dTexture);

  if (m_pGeometryCache->m_sFronds.m_usVertexCount > 0) {
    // activate the frond vertex buffer
    STATEMANAGER.SetStreamSource(0, m_pFrondVertexBuffer,
                                 sizeof(SFVFBranchVertex));
  }
}

///////////////////////////////////////////////////////////////////////
//	CSpeedTreeWrapper::RenderFronds

void CSpeedTreeWrapper::RenderFronds(void) const {
  m_pSpeedTree->GetGeometry(*m_pGeometryCache, SpeedTree_FrondGeometry);

  if (m_pGeometryCache->m_fFrondAlphaTestValue > 0.0f) {
    PositionTree();

    // set alpha test value
    STATEMANAGER.SetRenderState(
        D3DRS_ALPHAREF, DWORD(m_pGeometryCache->m_fFrondAlphaTestValue));

    // render if this LOD has fronds
    if (m_pFrondIndexCounts &&
        m_pGeometryCache->m_sFronds.m_nDiscreteLodLevel > -1 &&
        m_pFrondIndexCounts[m_pGeometryCache->m_sFronds.m_nDiscreteLodLevel] >
            0) {
      STATEMANAGER.SetIndices(
          m_pFrondIndexBuffers[m_pGeometryCache->m_sFronds.m_nDiscreteLodLevel],
          0);

      ms_faceCount +=
          m_pFrondIndexCounts[m_pGeometryCache->m_sFronds.m_nDiscreteLodLevel] -
          2;
      STATEMANAGER.DrawIndexedPrimitive(
          D3DPT_TRIANGLESTRIP, 0, m_pGeometryCache->m_sFronds.m_usVertexCount,
          0,
          m_pFrondIndexCounts[m_pGeometryCache->m_sFronds.m_nDiscreteLodLevel] -
              2);
    }
  }
}

///////////////////////////////////////////////////////////////////////
//	CSpeedTreeWrapper::SetupLeafForTreeType

void CSpeedTreeWrapper::SetupLeafForTreeType(void) const {
  if (!m_CompositeImageInstance.IsEmpty())
    STATEMANAGER.SetTexture(
        0, m_CompositeImageInstance.GetTextureReference().GetD3DTexture());

  // bind shadow texture
  STATEMANAGER.SetTexture(1, NULL);
}

///////////////////////////////////////////////////////////////////////
//	CSpeedTreeWrapper::RenderLeaves

void CSpeedTreeWrapper::RenderLeaves(void) const {
  // update leaf geometry
  m_pSpeedTree->GetGeometry(*m_pGeometryCache, SpeedTree_LeafGeometry);

#if !defined WRAPPER_USE_NO_WIND || defined WRAPPER_USE_CPU_LEAF_PLACEMENT
  // possibly need to update both leaf LOD's
  for (unsigned int i = 0; i < 2; ++i) {
    // reference to leaf structure
    const CSpeedTreeRT::SGeometry::SLeaf *pLeaf =
        (i == 0) ? &m_pGeometryCache->m_sLeaves0
                 : &m_pGeometryCache->m_sLeaves1;
    int unLod = pLeaf->m_nDiscreteLodLevel;

    if (pLeaf->m_bIsActive && pLeaf->m_usLeafCount > 0) {
      // update the centers
      SFVFLeafVertex *pVertex = NULL;
      m_pLeafVertexBuffer[unLod]->Lock(0, 0,
                                       reinterpret_cast<void **>(&pVertex),
                                       D3DLOCK_DISCARD | D3DLOCK_NOSYSLOCK);
      for (unsigned int unLeaf = 0; unLeaf < pLeaf->m_usLeafCount; ++unLeaf) {
        D3DXVECTOR3 vecCenter(&(pLeaf->m_pCenterCoords[unLeaf * 3]));
        D3DXVECTOR3 vec0(&pLeaf->m_pLeafMapCoords[unLeaf][0]);
        D3DXVECTOR3 vec1(&pLeaf->m_pLeafMapCoords[unLeaf][4]);
        D3DXVECTOR3 vec2(&pLeaf->m_pLeafMapCoords[unLeaf][8]);
        D3DXVECTOR3 vec3(&pLeaf->m_pLeafMapCoords[unLeaf][12]);

        (pVertex++)->m_vPosition = vecCenter + vec0;
        (pVertex++)->m_vPosition = vecCenter + vec1;
        (pVertex++)->m_vPosition = vecCenter + vec2;
        (pVertex++)->m_vPosition = vecCenter + vec0;
        (pVertex++)->m_vPosition = vecCenter + vec2;
        (pVertex++)->m_vPosition = vecCenter + vec3;
      }
      m_pLeafVertexBuffer[unLod]->Unlock();
    }
#endif
  }

  PositionTree();

  // render LODs, if needed
  for (unsigned int unLeafLevel = 0; unLeafLevel < 2; ++unLeafLevel) {
    const CSpeedTreeRT::SGeometry::SLeaf *pLeaf =
        (unLeafLevel == 0) ? &m_pGeometryCache->m_sLeaves0
                           : pLeaf = &m_pGeometryCache->m_sLeaves1;

    int unLod = pLeaf->m_nDiscreteLodLevel;

    if (unLod > -1 && pLeaf->m_bIsActive && pLeaf->m_usLeafCount > 0) {
      STATEMANAGER.SetStreamSource(0, m_pLeafVertexBuffer[unLod],
                                   sizeof(SFVFLeafVertex));
      STATEMANAGER.SetRenderState(D3DRS_ALPHAREF,
                                  DWORD(pLeaf->m_fAlphaTestValue));

      ms_faceCount += pLeaf->m_usLeafCount * 2;
      STATEMANAGER.DrawPrimitive(D3DPT_TRIANGLELIST, 0,
                                 pLeaf->m_usLeafCount * 2);
    }
  }
}

void CSpeedTreeWrapper::EndLeafForTreeType(void) {}

void CSpeedTreeWrapper::RenderBillboards(void) const {
#ifdef WRAPPER_BILLBOARD_MODE
  PositionTree();
  struct SBillboardVertex {
    float fX, fY, fZ;
    DWORD dColor;
    float fU, fV;
  };

  m_pSpeedTree->GetGeometry(*m_pGeometryCache, SpeedTree_BillboardGeometry);

  if (m_pGeometryCache->m_sBillboard0.m_bIsActive) {
    const float *pCoords = m_pGeometryCache->m_sBillboard0.m_pCoords;
    const float *pTexCoords = m_pGeometryCache->m_sBillboard0.m_pTexCoords;
    SBillboardVertex sVertex[4] = {
        {pCoords[0], pCoords[1], pCoords[2], 0xffffff, pTexCoords[0],
         pTexCoords[1]},
        {pCoords[3], pCoords[4], pCoords[5], 0xffffff, pTexCoords[2],
         pTexCoords[3]},
        {pCoords[6], pCoords[7], pCoords[8], 0xffffff, pTexCoords[4],
         pTexCoords[5]},
        {pCoords[9], pCoords[10], pCoords[11], 0xffffff, pTexCoords[6],
         pTexCoords[7]},
    };
    STATEMANAGER.SetRenderState(
        D3DRS_ALPHAREF,
        DWORD(m_pGeometryCache->m_sBillboard0.m_fAlphaTestValue));

    ms_faceCount += 2;
    STATEMANAGER.DrawPrimitiveUP(D3DPT_TRIANGLEFAN, 2, sVertex,
                                 sizeof(SBillboardVertex));
  }

  // if tree supports 360 degree billboards, render the second
  if (m_pGeometryCache->m_sBillboard1.m_bIsActive) {
    const float *pCoords = m_pGeometryCache->m_sBillboard1.m_pCoords;
    const float *pTexCoords = m_pGeometryCache->m_sBillboard1.m_pTexCoords;
    SBillboardVertex sVertex[4] = {
        {pCoords[0], pCoords[1], pCoords[2], 0xffffff, pTexCoords[0],
         pTexCoords[1]},
        {pCoords[3], pCoords[4], pCoords[5], 0xffffff, pTexCoords[2],
         pTexCoords[3]},
        {pCoords[6], pCoords[7], pCoords[8], 0xffffff, pTexCoords[4],
         pTexCoords[5]},
        {pCoords[9], pCoords[10], pCoords[11], 0xffffff, pTexCoords[6],
         pTexCoords[7]},
    };
    STATEMANAGER.SetRenderState(
        D3DRS_ALPHAREF,
        DWORD(m_pGeometryCache->m_sBillboard1.m_fAlphaTestValue));

    ms_faceCount += 2;
    STATEMANAGER.DrawPrimitiveUP(D3DPT_TRIANGLEFAN, 2, sVertex,
                                 sizeof(SBillboardVertex));
  }
#endif
}

///////////////////////////////////////////////////////////////////////
//	CSpeedTreeWrapper::CleanUpMemory

void CSpeedTreeWrapper::CleanUpMemory(void) {
  if (!m_bIsInstance)
    m_pSpeedTree->DeleteTransientData();
}

///////////////////////////////////////////////////////////////////////
//	CSpeedTreeWrapper::PositionTree

void CSpeedTreeWrapper::PositionTree(void) const {
  D3DXVECTOR3 vecPosition = m_pSpeedTree->GetTreePosition();
  D3DXMATRIX matTranslation;
  D3DXMatrixIdentity(&matTranslation);
  D3DXMatrixTranslation(&matTranslation, vecPosition.x, vecPosition.y,
                        vecPosition.z);

  // store translation for client-side transformation
  STATEMANAGER.SetTransform(D3DTS_WORLD, &matTranslation);

  // store translation for use in vertex shader
  D3DXVECTOR4 vecConstant(vecPosition[0], vecPosition[1], vecPosition[2], 0.0f);
  STATEMANAGER.SetVertexShaderConstant(c_nVertexShader_TreePos,
                                       (float *)&vecConstant, 1);
}

///////////////////////////////////////////////////////////////////////
//	CSpeedTreeWrapper::LoadTexture

bool CSpeedTreeWrapper::LoadTexture(const char *pFilename,
                                    CGraphicImageInstance &rImage) {
  CResource *pResource =
      CResourceManager::Instance().GetResourcePointer(pFilename);
  rImage.SetImagePointer(static_cast<CGraphicImage *>(pResource));

  if (rImage.IsEmpty())
    return false;

  // TraceError("SpeedTreeWrapper::LoadTexture: %s", pFilename);
  return true;
}

///////////////////////////////////////////////////////////////////////
//	CSpeedTreeWrapper::SetShaderConstants

void CSpeedTreeWrapper::SetShaderConstants(const float *pMaterial) const {
  const float afUsefulConstants[] = {m_pSpeedTree->GetLeafLightingAdjustment(),
                                     0.0f, 0.0f, 0.0f};
  STATEMANAGER.SetVertexShaderConstant(c_nVertexShader_LeafLightingAdjustment,
                                       afUsefulConstants, 1);

  const float afMaterial[] = {pMaterial[0], pMaterial[1], pMaterial[2], 1.0f,
                              pMaterial[3], pMaterial[4], pMaterial[5], 1.0f};
  STATEMANAGER.SetVertexShaderConstant(c_nVertexShader_Material, afMaterial, 2);
}

void CSpeedTreeWrapper::SetPosition(float x, float y, float z) {
  m_afPos[0] = x;
  m_afPos[1] = y;
  m_afPos[2] = z;
  m_pSpeedTree->SetTreePosition(x, y, z);
  CGraphicObjectInstance::SetPosition(x, y, z);
}
