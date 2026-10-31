#pragma once
#include <Platform/DirectXMathCompat.h>
#include <Neoeter/Model/SpeedTree/SpeedTreeConfig.h>
#include <map>
#include <string>

static DWORD D3DFVF_SPEEDTREE_BRANCH_VERTEX = D3DFVF_XYZ | D3DFVF_DIFFUSE | D3DFVF_TEX2 | D3DFVF_TEXCOORDSIZE2(0) | D3DFVF_TEXCOORDSIZE2(1);

struct SFVFBranchVertex
{
    D3DXVECTOR3     m_vPosition;                         
    DWORD           m_dwDiffuseColor;
    FLOAT           m_fTexCoords[2];
    FLOAT			m_fShadowCoords[2];
};

static const char g_achSimpleVertexProgram[] =
{
    "vs.1.1\n"
    "dcl_position       v0\n"
    "dcl_color          v5\n"
    "dcl_texcoord0      v7\n"
    "dcl_texcoord1      v8\n"
    "mov        oT0.xy, v7\n"
    "mov		oT1.xy,		v8\n"
    "mov        a0.x,   v8.x\n"
    "m4x4       r1,     v0,         c[54+a0.x]\n"
    "sub        r2,     r1,         v0\n"
    "mov        r3.x,   v8.y\n"
    "mad        r1,     r2,         r3.x,       v0\n"
    "add        r2,     c[52],      r1\n"
    "m4x4       oPos,   r2,         c[0]\n"
#ifdef WRAPPER_USE_FOG
    "dp4        r1,         r2,         c[2]\n"
    "sub        r2.x,       c[85].y,    r1.z\n"
    "mul        oFog,       r2.x,       c[85].z\n"
#endif
    "mov        oD0,    v5\n"
};

inline LPDIRECT3DVERTEXSHADER9 LoadBranchShader(LPDIRECT3DDEVICE9 pDx)
{
    UNREFERENCED_PARAMETER(pDx);
    return NULL;
}

static DWORD D3DFVF_SPEEDTREE_LEAF_VERTEX = D3DFVF_XYZ | D3DFVF_DIFFUSE | D3DFVF_TEXCOORDSIZE2(0) | D3DFVF_TEXCOORDSIZE2(1) | D3DFVF_TEXCOORDSIZE2(2) | D3DFVF_TEX1;

struct SFVFLeafVertex
{
    D3DXVECTOR3     m_vPosition;
    DWORD           m_dwDiffuseColor;                                     
    FLOAT           m_fTexCoords[2];
};

static const char g_achLeafVertexProgram[] =
{
        "vs.1.1\n"

        "dcl_position       v0\n"
        "dcl_color          v5\n"
        "dcl_texcoord0      v7\n"
        "dcl_texcoord1      v8\n"
        "dcl_texcoord2      v9\n"
        "mov        oT0.xy, v7\n"
    "mov        r0,     v0\n"
    "add        r0,     c[52],      r0\n"
    "m4x4       oPos,   r0,         c[0]\n"

#ifdef WRAPPER_USE_FOG
    "dp4        r1,         r0,         c[2]\n"
    "sub        r2.x,       c[85].y,    r1.z\n"
    "mul        oFog,       r2.x,       c[85].z\n"
#endif
    "mov        oD0,    v5\n"
};

inline LPDIRECT3DVERTEXSHADER9 LoadLeafShader(LPDIRECT3DDEVICE9 pDx)
{
    LPDIRECT3DVERTEXSHADER9 dwShader = NULL;
    return dwShader;
}
static DWORD D3DFVF_SPEEDTREE_BILLBOARD_VERTEX = D3DFVF_XYZ | D3DFVF_DIFFUSE | D3DFVF_TEX1;
