#include <Neoeter/Render/StdAfx.h>
#include <Neoeter/Render/GrpPixelShader.h>
#include <Neoeter/Render/GrpD3DXBuffer.h>
#include <Neoeter/Render/StateManager.h>
#include <Neoeter/Render/ShaderStats.h>

CPixelShader::CPixelShader()
{
	Initialize();
}

CPixelShader::~CPixelShader()
{
	Destroy();
}

void CPixelShader::Initialize()
{
	m_handle=0;
}

void CPixelShader::Destroy()
{
	if (m_handle)
		m_handle=0;
}

bool CPixelShader::CreateFromDiskFile(const char* c_szFileName)
{
	Destroy();

	LPD3DXBUFFER lpd3dxShaderBuffer;
	LPD3DXBUFFER lpd3dxErrorBuffer;

	if (FAILED(D3DXAssembleShaderFromFileA(c_szFileName, 0, NULL, 0, &lpd3dxShaderBuffer, &lpd3dxErrorBuffer)))
		return false;

	if (FAILED(ms_lpd3dDevice->CreatePixelShader((const DWORD*)lpd3dxShaderBuffer->GetBufferPointer(), &m_handle)))
		return false;

	// D13: the GL path returns a dummy handle; record the parity delta.
	CShaderStats::OnCreated(false, c_szFileName);

	return true;
}

void CPixelShader::Set()
{
	STATEMANAGER.SetPixelShader(m_handle);
}
