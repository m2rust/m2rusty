#include <Neoeter/Model/SpeedTree/StdAfx.h>
#include <Neoeter/Model/SpeedTree/CSpeedTreeDirectX.h>
#include <Platform/DirectXMathCompat.h>
#include <Neoeter/Model/SpeedTree/VertexShaders.h>
#include <Neoeter/Platform/Base/Timer.h>
#include <Neoeter/Render/StateManager.h>
#include <Neoeter/Render/Camera.h>

CSpeedTreeDirectX::CSpeedTreeDirectX() : m_dwBranchVertexShader(NULL), m_dwLeafVertexShader(NULL) {}

CSpeedTreeDirectX::~CSpeedTreeDirectX()
{
	SAFE_RELEASE(m_dwBranchVertexShader);
	SAFE_RELEASE(m_dwLeafVertexShader);
}

void CSpeedTreeDirectX::UploadWindMatrix(unsigned int uiLocation, const float* pMatrix) const
{
	STATEMANAGER.SetVertexShaderConstant(uiLocation, pMatrix, 4);
}

void CSpeedTreeDirectX::UpdateCompundMatrix(const D3DXVECTOR3& c_rEyeVec, const D3DXMATRIX& c_rmatView, const D3DXMATRIX& c_rmatProj)
{
	D3DXMATRIX matBlend;
	D3DXMatrixIdentity(&matBlend);

	D3DXMATRIX matBlendShader;
	D3DXMatrixMultiply(&matBlendShader, &c_rmatView, &c_rmatProj);

	float afDirection[3];
	afDirection[0] = matBlendShader.m[0][2];
	afDirection[1] = matBlendShader.m[1][2];
	afDirection[2] = matBlendShader.m[2][2];
	CSpeedTreeRT::SetCamera(c_rEyeVec, afDirection);

	D3DXMatrixTranspose(&matBlendShader, &matBlendShader);
	STATEMANAGER.SetVertexShaderConstant(c_nVertexShader_CompoundMatrix, (const float*)&matBlendShader, 4);
}

bool CSpeedTreeDirectX::SetRenderingDevice()
{
	if (!InitVertexShaders())
		return false;

	const float c_afLightPosition[4] = { -0.707f, -0.300f, 0.707f, 0.0f };
	const float	c_afLightAmbient[4] = { 0.5f, 0.5f, 0.5f, 1.0f };
	const float	c_afLightDiffuse[4] = { 1.0f, 1.0f, 1.0f, 1.0f };
	const float	c_afLightSpecular[4] = { 1.0f, 1.0f, 1.0f, 1.0f };
	float afLight1[] =
	{
		c_afLightPosition[0], c_afLightPosition[1], c_afLightPosition[2],
		c_afLightDiffuse[0], c_afLightDiffuse[1], c_afLightDiffuse[2],
		c_afLightAmbient[0], c_afLightAmbient[1], c_afLightAmbient[2],
		c_afLightSpecular[0], c_afLightSpecular[1], c_afLightSpecular[2],
		c_afLightPosition[3],
		1.0f, 0.0f, 0.0f
	};

	CSpeedTreeRT::SetLightAttributes(0, afLight1);
	CSpeedTreeRT::SetLightState(0, true);
	return true;
}

void CSpeedTreeDirectX::Render(unsigned long ulRenderBitVector)
{
	if (m_pMainTreeMap.empty())
		return;

	if (!(ulRenderBitVector & Forest_RenderToShadow) && !(ulRenderBitVector & Forest_RenderToMiniMap))
		UpdateCompundMatrix(CCameraManager::Instance().GetCurrentCamera()->GetEye(), ms_matView, ms_matProj);

	DWORD dwLightState = STATEMANAGER.GetRenderState(D3DRS_LIGHTING);
	DWORD dwColorVertexState = STATEMANAGER.GetRenderState(D3DRS_COLORVERTEX);
	DWORD dwFogVertexMode = STATEMANAGER.GetRenderState(D3DRS_FOGVERTEXMODE);

	STATEMANAGER.SetRenderState(D3DRS_LIGHTING, FALSE);
	STATEMANAGER.SetRenderState(D3DRS_COLORVERTEX, TRUE);

	UINT uiCount;
	TTreeMap::const_iterator itor = m_pMainTreeMap.begin();

	while (itor != m_pMainTreeMap.end())
	{
		CSpeedTreeWrapper* pMainTree = (itor++)->second;
		CSpeedTreeWrapper** ppInstances = pMainTree->GetInstances(uiCount);

		for (UINT i = 0; i < uiCount; ++i)
		{
			ppInstances[i]->Advance();
		}
	}

	STATEMANAGER.SetVertexShaderConstant(c_nVertexShader_Light, m_afLighting, 3);
	STATEMANAGER.SetVertexShaderConstant(c_nVertexShader_Fog, m_afFog, 1);

	if (ulRenderBitVector & Forest_RenderToShadow)
	{
		STATEMANAGER.SetTextureStageState(0, D3DTSS_ALPHAARG1, D3DTA_TEXTURE);
		STATEMANAGER.SetTextureStageState(0, D3DTSS_ALPHAARG2, D3DTA_DIFFUSE);
		STATEMANAGER.SetTextureStageState(0, D3DTSS_ALPHAOP, D3DTOP_MODULATE);
	}
	else
	{
		STATEMANAGER.SetTextureStageState(0, D3DTSS_COLORARG1, D3DTA_TEXTURE);
		STATEMANAGER.SetTextureStageState(0, D3DTSS_COLORARG2, D3DTA_DIFFUSE);
		STATEMANAGER.SetTextureStageState(0, D3DTSS_COLOROP, D3DTOP_MODULATE);
		STATEMANAGER.SetTextureStageState(0, D3DTSS_ALPHAARG1, D3DTA_TEXTURE);
		STATEMANAGER.SetTextureStageState(0, D3DTSS_ALPHAARG2, D3DTA_DIFFUSE);
		STATEMANAGER.SetTextureStageState(0, D3DTSS_ALPHAOP, D3DTOP_MODULATE);
		STATEMANAGER.SetSamplerState(0, D3DSAMP_MINFILTER, D3DTEXF_LINEAR);
		STATEMANAGER.SetSamplerState(0, D3DSAMP_MAGFILTER, D3DTEXF_LINEAR);
		STATEMANAGER.SetSamplerState(0, D3DSAMP_MIPFILTER, D3DTEXF_ANISOTROPIC);
		// SpeedTree texture coordinates are returned in the flipped (Direct3D)
		// convention and may be negative; they rely on wrapping.
		STATEMANAGER.SetSamplerState(0, D3DSAMP_ADDRESSU, D3DTADDRESS_WRAP);
		STATEMANAGER.SetSamplerState(0, D3DSAMP_ADDRESSV, D3DTADDRESS_WRAP);

		STATEMANAGER.SetTextureStageState(1, D3DTSS_COLORARG1, D3DTA_TEXTURE);
		STATEMANAGER.SetTextureStageState(1, D3DTSS_COLORARG2, D3DTA_CURRENT);
		STATEMANAGER.SetTextureStageState(1, D3DTSS_COLOROP, D3DTOP_MODULATE);
		STATEMANAGER.SetTextureStageState(1, D3DTSS_ALPHAOP, D3DTOP_DISABLE);
		STATEMANAGER.SetSamplerState(1, D3DSAMP_ADDRESSU, D3DTADDRESS_WRAP);
		STATEMANAGER.SetSamplerState(1, D3DSAMP_ADDRESSV, D3DTADDRESS_WRAP);
	}

	STATEMANAGER.SaveRenderState(D3DRS_ALPHATESTENABLE, TRUE);
	STATEMANAGER.SaveRenderState(D3DRS_ALPHAFUNC, D3DCMP_GREATER);
	STATEMANAGER.SaveRenderState(D3DRS_CULLMODE, D3DCULL_CW);

	if (STATEMANAGER.GetRenderState(D3DRS_FOGENABLE)){}

	STATEMANAGER.SetFVF(D3DFVF_SPEEDTREE_BRANCH_VERTEX);
	STATEMANAGER.SetVertexShader(m_dwBranchVertexShader);

	if (ulRenderBitVector & Forest_RenderBranches)
	{
		itor = m_pMainTreeMap.begin();

		while (itor != m_pMainTreeMap.end())
		{
			CSpeedTreeWrapper* pMainTree = (itor++)->second;
			CSpeedTreeWrapper** ppInstances = pMainTree->GetInstances(uiCount);

			pMainTree->SetupBranchForTreeType();

			for (UINT i = 0; i < uiCount; ++i)
				if (ppInstances[i]->isShow())
					ppInstances[i]->RenderBranches();
		}
	}

	STATEMANAGER.SetRenderState(D3DRS_CULLMODE, D3DCULL_NONE);

	if (ulRenderBitVector & Forest_RenderFronds)
	{
		itor = m_pMainTreeMap.begin();

		while (itor != m_pMainTreeMap.end())
		{
			CSpeedTreeWrapper* pMainTree = (itor++)->second;
			CSpeedTreeWrapper** ppInstances = pMainTree->GetInstances(uiCount);

			pMainTree->SetupFrondForTreeType();

			for (UINT i = 0; i < uiCount; ++i)
				if (ppInstances[i]->isShow())
					ppInstances[i]->RenderFronds();
		}
	}

	if (ulRenderBitVector & Forest_RenderLeaves)
	{
		STATEMANAGER.SetFVF(D3DFVF_SPEEDTREE_LEAF_VERTEX);
		STATEMANAGER.SetVertexShader(m_dwLeafVertexShader);

		if (STATEMANAGER.GetRenderState(D3DRS_FOGENABLE)){}
		if (ulRenderBitVector & Forest_RenderToShadow || ulRenderBitVector & Forest_RenderToMiniMap)
		{
			STATEMANAGER.SetRenderState(D3DRS_ALPHAFUNC, D3DCMP_NOTEQUAL);
			STATEMANAGER.SaveRenderState(D3DRS_ALPHAREF, 0x00000000);
		}

		itor = m_pMainTreeMap.begin();

		while (itor != m_pMainTreeMap.end())
		{
			CSpeedTreeWrapper* pMainTree = (itor++)->second;
			CSpeedTreeWrapper** ppInstances = pMainTree->GetInstances(uiCount);

			pMainTree->SetupLeafForTreeType();

			for (UINT i = 0; i < uiCount; ++i)
				if (ppInstances[i]->isShow())
					ppInstances[i]->RenderLeaves();
		}

		if (ulRenderBitVector & Forest_RenderToShadow || ulRenderBitVector & Forest_RenderToMiniMap)
		{
			STATEMANAGER.SetRenderState(D3DRS_ALPHAFUNC, D3DCMP_GREATER);
			STATEMANAGER.RestoreRenderState(D3DRS_ALPHAREF);
		}
	}

	if (ulRenderBitVector & Forest_RenderBillboards)
	{
		STATEMANAGER.SetVertexShader(NULL);
		STATEMANAGER.SetFVF(D3DFVF_SPEEDTREE_BILLBOARD_VERTEX);
		STATEMANAGER.SetRenderState(D3DRS_LIGHTING, FALSE);
		STATEMANAGER.SetRenderState(D3DRS_COLORVERTEX, FALSE);

		itor = m_pMainTreeMap.begin();

		while (itor != m_pMainTreeMap.end())
		{
			CSpeedTreeWrapper* pMainTree = (itor++)->second;
			CSpeedTreeWrapper** ppInstances = pMainTree->GetInstances(uiCount);

			pMainTree->SetupBranchForTreeType();

			for (UINT i = 0; i < uiCount; ++i)
				if (ppInstances[i]->isShow())
					ppInstances[i]->RenderBillboards();
		}
	}

	STATEMANAGER.SetRenderState(D3DRS_LIGHTING, dwLightState);
	STATEMANAGER.SetRenderState(D3DRS_COLORVERTEX, dwColorVertexState);
	STATEMANAGER.SetRenderState(D3DRS_FOGVERTEXMODE, dwFogVertexMode);

	if (!(ulRenderBitVector & Forest_RenderToShadow))
	{
		STATEMANAGER.SetTextureStageState(1, D3DTSS_COLOROP, D3DTOP_DISABLE);
		STATEMANAGER.SetTextureStageState(1, D3DTSS_ALPHAOP, D3DTOP_DISABLE);

	}

	STATEMANAGER.RestoreRenderState(D3DRS_ALPHATESTENABLE);
	STATEMANAGER.RestoreRenderState(D3DRS_ALPHAFUNC);
	STATEMANAGER.RestoreRenderState(D3DRS_CULLMODE);
}

bool CSpeedTreeDirectX::InitVertexShaders(void)
{
	m_dwBranchVertexShader = LoadBranchShader(ms_lpd3dDevice);
	m_dwLeafVertexShader = LoadLeafShader(ms_lpd3dDevice);

	return true;
}
