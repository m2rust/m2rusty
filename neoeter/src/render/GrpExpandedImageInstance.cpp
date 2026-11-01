#include <Neoeter/Render/StdAfx.h>
#include <Neoeter/Platform/Base/CRC32.h>
#ifdef OPENMT2_PLATFORM_LINUX
#include <Platform/GLCompat.h>
#endif
#include <Neoeter/Render/GrpExpandedImageInstance.h>
#include <Neoeter/Render/StateManager.h>

CDynamicPool<CGraphicExpandedImageInstance> CGraphicExpandedImageInstance::ms_kPool;

void CGraphicExpandedImageInstance::CreateSystem(UINT uCapacity)
{
    ms_kPool.Create(uCapacity);
}

void CGraphicExpandedImageInstance::DestroySystem()
{
    ms_kPool.Destroy();
}

CGraphicExpandedImageInstance *CGraphicExpandedImageInstance::New()
{
    return ms_kPool.Alloc();
}

void CGraphicExpandedImageInstance::Delete(CGraphicExpandedImageInstance *pkImgInst)
{
    pkImgInst->Destroy();
    ms_kPool.Free(pkImgInst);
}

void CGraphicExpandedImageInstance::OnRender()
{
    CGraphicImage *pImage = m_roImage.GetPointer();
    CGraphicTexture *pTexture = pImage->GetTexturePointer();

    const RECT &c_rRect = pImage->GetRectReference();
    float const texReverseWidth = 1.0f / static_cast<float>(pTexture->GetWidth());
    float const texReverseHeight = 1.0f / static_cast<float>(pTexture->GetHeight());
    float const su = (c_rRect.left - m_RenderingRect.left) * texReverseWidth;
    float const sv = (c_rRect.top - m_RenderingRect.top) * texReverseHeight;
    float const eu = (c_rRect.left + m_RenderingRect.right + (c_rRect.right - c_rRect.left)) * texReverseWidth;
    float const ev = (c_rRect.top + m_RenderingRect.bottom + (c_rRect.bottom - c_rRect.top)) * texReverseHeight;

    TPDTVertex vertices[4];
    vertices[0].position.x = m_v2Position.x - 0.5f;
    vertices[0].position.y = m_v2Position.y - 0.5f;
    vertices[0].position.z = m_fDepth;
    vertices[0].texCoord = TTextureCoordinate(su, sv);
    vertices[0].diffuse = m_DiffuseColor;

    vertices[1].position.x = m_v2Position.x - 0.5f;
    vertices[1].position.y = m_v2Position.y - 0.5f;
    vertices[1].position.z = m_fDepth;
    vertices[1].texCoord = TTextureCoordinate(eu, sv);
    vertices[1].diffuse = m_DiffuseColor;

    vertices[2].position.x = m_v2Position.x - 0.5f;
    vertices[2].position.y = m_v2Position.y - 0.5f;
    vertices[2].position.z = m_fDepth;
    vertices[2].texCoord = TTextureCoordinate(su, ev);
    vertices[2].diffuse = m_DiffuseColor;

    vertices[3].position.x = m_v2Position.x - 0.5f;
    vertices[3].position.y = m_v2Position.y - 0.5f;
    vertices[3].position.z = m_fDepth;
    vertices[3].texCoord = TTextureCoordinate(eu, ev);
    vertices[3].diffuse = m_DiffuseColor;

    if (0.0f == m_fRotation)
    {
        float const fimgWidth = static_cast<float>(pImage->GetWidth()) * m_v2Scale.x;
        float const fimgHeight = static_cast<float>(pImage->GetHeight()) * m_v2Scale.y;

        vertices[0].position.x -= m_RenderingRect.left;
        vertices[0].position.y -= m_RenderingRect.top;
        vertices[1].position.x += fimgWidth + m_RenderingRect.right;
        vertices[1].position.y -= m_RenderingRect.top;
        vertices[2].position.x -= m_RenderingRect.left;
        vertices[2].position.y += fimgHeight + m_RenderingRect.bottom;
        vertices[3].position.x += fimgWidth + m_RenderingRect.right;
        vertices[3].position.y += fimgHeight + m_RenderingRect.bottom;
        if ((0.0f < m_v2Scale.x && 0.0f > m_v2Scale.y) || (0.0f > m_v2Scale.x && 0.0f < m_v2Scale.y))
        {
            STATEMANAGER.SetRenderState(D3DRS_CULLMODE, D3DCULL_CCW);
        }
    }
    else
    {
        float const fimgHalfWidth = static_cast<float>(pImage->GetWidth()) / 2.0f * m_v2Scale.x;
        float const fimgHalfHeight = static_cast<float>(pImage->GetHeight()) / 2.0f * m_v2Scale.y;

        for (auto &vertice : vertices)
        {
            vertice.position.x += m_v2Origin.x;
            vertice.position.y += m_v2Origin.y;
        }

        float const fRadian = D3DXToRadian(m_fRotation);
        vertices[0].position.x += (-fimgHalfWidth * cosf(fRadian)) - (-fimgHalfHeight * sinf(fRadian));
        vertices[0].position.y += (-fimgHalfWidth * sinf(fRadian)) + (-fimgHalfHeight * cosf(fRadian));
        vertices[1].position.x += (+fimgHalfWidth * cosf(fRadian)) - (-fimgHalfHeight * sinf(fRadian));
        vertices[1].position.y += (+fimgHalfWidth * sinf(fRadian)) + (-fimgHalfHeight * cosf(fRadian));
        vertices[2].position.x += (-fimgHalfWidth * cosf(fRadian)) - (+fimgHalfHeight * sinf(fRadian));
        vertices[2].position.y += (-fimgHalfWidth * sinf(fRadian)) + (+fimgHalfHeight * cosf(fRadian));
        vertices[3].position.x += (+fimgHalfWidth * cosf(fRadian)) - (+fimgHalfHeight * sinf(fRadian));
        vertices[3].position.y += (+fimgHalfWidth * sinf(fRadian)) + (+fimgHalfHeight * cosf(fRadian));
    }

    switch (m_iRenderingMode)
    {
    case RENDERING_MODE_SCREEN:
    case RENDERING_MODE_COLOR_DODGE:
        STATEMANAGER.SaveRenderState(D3DRS_SRCBLEND, D3DBLEND_INVDESTCOLOR);
        STATEMANAGER.SaveRenderState(D3DRS_DESTBLEND, D3DBLEND_ONE);
        break;
    case RENDERING_MODE_MODULATE:
        STATEMANAGER.SaveRenderState(D3DRS_SRCBLEND, D3DBLEND_ZERO);
        STATEMANAGER.SaveRenderState(D3DRS_DESTBLEND, D3DBLEND_SRCCOLOR);
        break;
    }

    // 2004.11.18.myevan.ctrl+alt+del 반복 사용시 튕기는 문제
    if (CGraphicBase::SetPDTStream(vertices, 4))
    {
        CGraphicBase::SetDefaultIndexBuffer(CGraphicBase::DEFAULT_IB_FILL_RECT);

        STATEMANAGER.SetTexture(0, pTexture->GetD3DTexture());
        STATEMANAGER.SetTexture(1, nullptr);
        STATEMANAGER.SetFVF(D3DFVF_XYZ | D3DFVF_DIFFUSE | D3DFVF_TEX1);

#ifdef OPENMT2_PLATFORM_LINUX
        // Set wrap modes dynamically: clamp non-repeating axes to avoid 1px
        // border bleed
        GLint const wrapS = (su < -0.001f || eu > 1.001f) ? GL_REPEAT : GL_CLAMP_TO_EDGE;
        GLint const wrapT = (sv < -0.001f || ev > 1.001f) ? GL_REPEAT : GL_CLAMP_TO_EDGE;
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, wrapS);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, wrapT);
#endif

        STATEMANAGER.DrawIndexedPrimitive(D3DPT_TRIANGLELIST, 0, 4, 0, 2);

#ifdef OPENMT2_PLATFORM_LINUX
        if (wrapS == GL_REPEAT)
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
        if (wrapT == GL_REPEAT)
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
#endif
    }
    // STATEMANAGER.DrawIndexedPrimitiveUP(D3DPT_TRIANGLELIST, 0, 4, 2,
    // c_FillRectIndices, D3DFMT_INDEX16, vertices, sizeof(TPDTVertex));
    /////////////////////////////////////////////////////////////

    switch (m_iRenderingMode)
    {
    case RENDERING_MODE_SCREEN:
    case RENDERING_MODE_COLOR_DODGE:
    case RENDERING_MODE_MODULATE:
        STATEMANAGER.RestoreRenderState(D3DRS_SRCBLEND);
        STATEMANAGER.RestoreRenderState(D3DRS_DESTBLEND);
        break;
    }
    STATEMANAGER.SetRenderState(D3DRS_CULLMODE, D3DCULL_CW);
}

void CGraphicExpandedImageInstance::SetDepth(float fDepth)
{
    m_fDepth = fDepth;
}

void CGraphicExpandedImageInstance::SetOrigin()
{
    SetOrigin(static_cast<float>(GetWidth()) / 2.0f, static_cast<float>(GetHeight()) / 2.0f);
}

void CGraphicExpandedImageInstance::SetOrigin(float fx, float fy)
{
    m_v2Origin.x = fx;
    m_v2Origin.y = fy;
}

void CGraphicExpandedImageInstance::SetRotation(float fRotation)
{
    m_fRotation = fRotation;
}

void CGraphicExpandedImageInstance::SetScale(float fx, float fy)
{
    m_v2Scale.x = fx;
    m_v2Scale.y = fy;
}

void CGraphicExpandedImageInstance::SetRenderingRect(float fLeft, float fTop, float fRight, float fBottom)
{
    if (IsEmpty())
        return;

    auto fWidth = static_cast<float>(GetWidth());
    auto fHeight = static_cast<float>(GetHeight());

    m_RenderingRect.left = fWidth * fLeft;
    m_RenderingRect.top = fHeight * fTop;
    m_RenderingRect.right = fWidth * fRight;
    m_RenderingRect.bottom = fHeight * fBottom;
}

void CGraphicExpandedImageInstance::SetRenderingMode(int iMode)
{
    m_iRenderingMode = iMode;
}

DWORD CGraphicExpandedImageInstance::Type()
{
    static DWORD const s_dwType = GetCRC32("CGraphicExpandedImageInstance", strlen("CGraphicExpandedImageInstance"));
    return (s_dwType);
}

void CGraphicExpandedImageInstance::OnSetImagePointer()
{
    if (IsEmpty())
        return;

    SetOrigin(static_cast<float>(GetWidth()) / 2.0f, static_cast<float>(GetHeight()) / 2.0f);
}

BOOL CGraphicExpandedImageInstance::OnIsType(DWORD dwType)
{
    if (CGraphicExpandedImageInstance::Type() == dwType)
        return TRUE;

    return CGraphicImageInstance::IsType(dwType);
}

void CGraphicExpandedImageInstance::Initialize()
{
    m_iRenderingMode = RENDERING_MODE_NORMAL;
    m_fDepth = 0.0f;
    m_v2Origin.x = m_v2Origin.y = 0.0f;
    m_v2Scale.x = m_v2Scale.y = 1.0f;
    m_fRotation = 0.0f;
    memset(&m_RenderingRect, 0, sizeof(RECT));
}

void CGraphicExpandedImageInstance::Destroy()
{
    CGraphicImageInstance::Destroy();
    Initialize();
}

CGraphicExpandedImageInstance::CGraphicExpandedImageInstance()
{
    Initialize();
}

CGraphicExpandedImageInstance::~CGraphicExpandedImageInstance()
{
    Destroy();
}
