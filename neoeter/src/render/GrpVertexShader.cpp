#include <Neoeter/Render/StdAfx.h>
#include <Neoeter/Render/GrpVertexShader.h>
#include <Neoeter/Render/GrpD3DXBuffer.h>
#include <Neoeter/Render/StateManager.h>
#include <Neoeter/Render/ShaderStats.h>

CVertexShader::CVertexShader()
{
	Initialize();
}

CVertexShader::~CVertexShader()
{
	Destroy();
}

void CVertexShader::Initialize()
{
	m_handle=0;
}

void CVertexShader::Destroy()
{
	if (m_handle)
	{
		m_handle->Release();
		m_handle = 0;
	}
}

bool CVertexShader::CreateFromDiskFile(const char* c_szFileName, const DWORD* c_pdwVertexDecl)
{
	Destroy();

	LPD3DXBUFFER lpd3dxShaderBuffer;
	LPD3DXBUFFER lpd3dxErrorBuffer;
	if (FAILED(D3DXAssembleShaderFromFile(c_szFileName, 0, NULL, 0, &lpd3dxShaderBuffer, &lpd3dxErrorBuffer)))
		return false;

	if (FAILED(ms_lpd3dDevice->CreateVertexShader((const DWORD*)lpd3dxShaderBuffer->GetBufferPointer(), &m_handle)))
		return false;

	// D13: the GL path returns a dummy handle; record the parity delta.
	CShaderStats::OnCreated(true, c_szFileName);

	return true;
}

void CVertexShader::Set()
{
	STATEMANAGER.SetVertexShader(m_handle);
}
