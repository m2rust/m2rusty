// Direct3D 9 compatibility façade (T1.7).
//
// State tracking and resource bookkeeping only: every OpenGL call now lives
// behind the renderer seam in src/backends/gl/ (invariant R1). The façade
// records D3D9 state and hands the backend value snapshots (R3); handles are
// opaque (R2) and all drawing goes through IRenderBackend::Draw (R4).

#include <Platform/DirectXMathCompat.h>

// Force link ImageLoaderCompat.cpp from static libEterBase.a
extern bool Platform_GetImageInfo(const void* data, size_t size, uint32_t* outW, uint32_t* outH, uint32_t* outFormat);
extern bool Platform_DecodeImage(const void* data, size_t size, uint32_t* outW, uint32_t* outH, std::vector<uint8_t>& outPixels);
void* g_pfnForceLinkImageLoader[] = {
    (void*)&Platform_GetImageInfo,
    (void*)&Platform_DecodeImage
};

// The renderer, not the platform, selects this translation unit (R8): the GL
// compatibility facade is the Windows path too (T1.5).
#if defined(OPENMT2_RENDERER_GL)
#include <Platform/RenderBackend.h>
#include <algorithm>
#include <cstring>

namespace {

IRenderBackend& Backend()
{
    return Platform_GetRenderBackend();
}

UINT PrimitiveCountToVertexCount(D3DPRIMITIVETYPE pt, UINT count)
{
    switch (pt) {
        case D3DPT_POINTLIST: return count;
        case D3DPT_LINELIST: return count * 2;
        case D3DPT_LINESTRIP: return count + 1;
        case D3DPT_TRIANGLELIST: return count * 3;
        case D3DPT_TRIANGLESTRIP: return count + 2;
        case D3DPT_TRIANGLEFAN: return count + 2;
        default: return count * 3;
    }
}

void CopyMatrix(float (&dst)[16], const D3DXMATRIX& src)
{
    std::memcpy(dst, &src, sizeof(float) * 16);
}

} // namespace

// =============================================================
// IDirect3DTexture9 Implementation
// =============================================================

IDirect3DTexture9::IDirect3DTexture9(UINT w, UINT h, D3DFORMAT fmt, UINT /*levels*/)
    : m_width(w ? w : 1), m_height(h ? h : 1), m_format(fmt), m_handle{0, 0}, m_dirty(false)
{
    size_t sz = GetLevelDataSize(0);
    m_data.resize(sz ? sz : 4, 0);
}

IDirect3DTexture9::~IDirect3DTexture9()
{
    if (m_handle.IsValid()) {
        Backend().Destroy(m_handle);
        m_handle = RenderHandle{0, 0};
    }
}

RenderHandle IDirect3DTexture9::GetTextureHandle()
{
    if (!m_handle.IsValid() && !m_data.empty()) {
        UploadToBackend();
    }
    return m_handle;
}

size_t IDirect3DTexture9::GetLevelDataSize(UINT level) const
{
    UINT mipW = std::max(1u, m_width >> level);
    UINT mipH = std::max(1u, m_height >> level);
    if (m_format == D3DFMT_DXT1) {
        return (size_t)std::max(1u, (mipW + 3) / 4) * std::max(1u, (mipH + 3) / 4) * 8;
    }
    if (m_format == D3DFMT_DXT3 || m_format == D3DFMT_DXT5) {
        return (size_t)std::max(1u, (mipW + 3) / 4) * std::max(1u, (mipH + 3) / 4) * 16;
    }
    if (m_format == D3DFMT_A4R4G4B4 || m_format == D3DFMT_A1R5G5B5 || m_format == D3DFMT_R5G6B5) {
        return (size_t)mipW * mipH * 2;
    }
    return (size_t)mipW * mipH * 4;
}

INT IDirect3DTexture9::GetPitch(UINT level) const
{
    UINT mipW = std::max(1u, m_width >> level);
    if (m_format == D3DFMT_DXT1) return (INT)(std::max(1u, (mipW + 3) / 4) * 8);
    if (m_format == D3DFMT_DXT3 || m_format == D3DFMT_DXT5) return (INT)(std::max(1u, (mipW + 3) / 4) * 16);
    if (m_format == D3DFMT_A4R4G4B4 || m_format == D3DFMT_A1R5G5B5 || m_format == D3DFMT_R5G6B5) return (INT)(mipW * 2);
    return (INT)(mipW * 4);
}

HRESULT IDirect3DTexture9::LockRect(UINT Level, D3DLOCKED_RECT* pLockedRect, const RECT*, DWORD)
{
    if (!pLockedRect) return E_INVALIDARG;
    if (Level == 0) {
        size_t sz = GetLevelDataSize(0);
        if (m_data.size() < sz) m_data.resize(sz, 0);
        pLockedRect->Pitch = GetPitch(0);
        pLockedRect->pBits = m_data.data();
    } else {
        size_t sz = GetLevelDataSize(Level);
        auto& mip = m_mipLevels[Level];
        if (mip.size() < sz) mip.resize(sz, 0);
        pLockedRect->Pitch = GetPitch(Level);
        pLockedRect->pBits = mip.data();
    }
    m_dirty = true;
    return S_OK;
}

HRESULT IDirect3DTexture9::UnlockRect(UINT)
{
    UploadToBackend();
    return S_OK;
}

void IDirect3DTexture9::UploadLevelToBackend(UINT level, const BYTE* pBytes, size_t dataSize)
{
    if (!pBytes || dataSize <= 0) return;
    UINT mipW = std::max(1u, m_width >> level);
    UINT mipH = std::max(1u, m_height >> level);
    Backend().UploadTextureLevel(m_handle, level, (uint32_t)m_format, mipW, mipH, pBytes, dataSize);
}

void IDirect3DTexture9::Fill(const DWORD color)
{
    if (m_format == D3DFMT_DXT1 || m_format == D3DFMT_DXT3 || m_format == D3DFMT_DXT5)
        return;
    if (GetFormatBytesPerPixel(m_format) == 2) {
        WORD v = (WORD)(color & 0xFFFF);
        for (size_t i = 0; i + 1 < m_data.size(); i += 2) {
            m_data[i] = (BYTE)(v & 0xFF);
            m_data[i + 1] = (BYTE)(v >> 8);
        }
    } else {
        for (size_t i = 0; i + 3 < m_data.size(); i += 4) {
            m_data[i] = (BYTE)(color & 0xFF);
            m_data[i + 1] = (BYTE)((color >> 8) & 0xFF);
            m_data[i + 2] = (BYTE)((color >> 16) & 0xFF);
            m_data[i + 3] = (BYTE)((color >> 24) & 0xFF);
        }
    }
    m_dirty = true;
}

void IDirect3DTexture9::UploadToBackend()
{
    if (m_data.empty()) return;
    if (!m_handle.IsValid()) {
        m_handle = Backend().CreateTexture();
    }
    if (!m_handle.IsValid()) return;

    // Upload Level 0
    UploadLevelToBackend(0, m_data.data(), m_data.size());

    // Upload any other mip levels
    UINT maxLevel = 0;
    for (const auto& kv : m_mipLevels) {
        UINT level = kv.first;
        if (level > 0 && !kv.second.empty()) {
            UploadLevelToBackend(level, kv.second.data(), kv.second.size());
            if (level > maxLevel) maxLevel = level;
        }
    }

    Backend().FinalizeTexture(m_handle, maxLevel);
    m_dirty = false;
}

size_t IDirect3DTexture9::GetFormatBytesPerPixel(D3DFORMAT fmt)
{
    if (fmt == D3DFMT_DXT1) return 1;
    if (fmt == D3DFMT_DXT3 || fmt == D3DFMT_DXT5) return 1;
    if (fmt == D3DFMT_A4R4G4B4 || fmt == D3DFMT_A1R5G5B5 || fmt == D3DFMT_R5G6B5) return 2;
    return 4;
}

HRESULT IDirect3DTexture9::GetSurfaceLevel(UINT, IDirect3DSurface9** ppSurfaceLevel)
{
    if (!ppSurfaceLevel) return E_INVALIDARG;
    *ppSurfaceLevel = new IDirect3DSurface9(m_width, m_height, this);
    return S_OK;
}

// =============================================================
// IDirect3DSurface9 Implementation
// =============================================================

IDirect3DSurface9::IDirect3DSurface9(UINT w, UINT h, IDirect3DTexture9* pTexture,
                                     bool bDepthStencil, bool bDefault)
    : m_width(w ? w : 1), m_height(h ? h : 1),
      m_texture(pTexture), m_bDepthStencil(bDepthStencil), m_bDefault(bDefault),
      m_depthStencil{0, 0}
{
    // Only plain CPU surfaces need backing storage (LockRect / GetDesc users).
    // Colour render-target surfaces and depth-stencil surfaces live on the GPU.
    if (!m_bDepthStencil && !m_texture)
        m_data.resize((size_t)m_width * m_height * 4, 0);
}

IDirect3DSurface9::~IDirect3DSurface9()
{
    if (m_depthStencil.IsValid()) {
        Backend().Destroy(m_depthStencil);
        m_depthStencil = RenderHandle{0, 0};
    }
}

RenderHandle IDirect3DSurface9::GetDepthStencil()
{
    if (!m_depthStencil.IsValid()) {
        m_depthStencil = Backend().CreateDepthStencil(m_width, m_height);
    }
    return m_depthStencil;
}

// =============================================================
// IDirect3DDevice9 Implementation
// =============================================================

IDirect3DDevice9::IDirect3DDevice9()
    : m_pIndexBuffer(nullptr), m_fvf(D3DFVF_XYZ),
      m_srcBlend(D3DBLEND_SRCALPHA), m_destBlend(D3DBLEND_INVSRCALPHA),
      m_zEnable(false), m_zWriteEnable(true), m_zFunc(D3DCMP_LESSEQUAL),
      m_alphaBlendEnable(false), m_alphaTestEnable(false),
      m_alphaRef(0), m_alphaFunc(D3DCMP_ALWAYS),
      m_cullMode(D3DCULL_CCW), m_fillMode(D3DFILL_SOLID),
      m_textureFactor(0xFFFFFFFF), m_lighting(0), m_colorVertex(0), m_ambient(0),
      m_fogEnable(false), m_fogColor(0), m_fogVertexMode(D3DFOG_NONE), m_fogTableMode(D3DFOG_NONE),
      m_fogStart(0.0f), m_fogEnd(0.0f), m_fogDensity(1.0f), m_rangeFogEnable(false),
      m_inRenderTarget(false)
{
    std::memset(&m_viewport, 0, sizeof(m_viewport));
    m_viewport.Width = 1024;
    m_viewport.Height = 768;
    m_viewport.MaxZ = 1.0f;
    std::memset(&m_material, 0, sizeof(m_material));
    m_material.Diffuse.r = m_material.Diffuse.g = m_material.Diffuse.b = m_material.Diffuse.a = 1.0f;
    m_material.Ambient.r = m_material.Ambient.g = m_material.Ambient.b = m_material.Ambient.a = 1.0f;
    for (int i = 0; i < 8; ++i) {
        m_boundTextures[i] = nullptr;
        m_texTransformFlags[i] = D3DTTFF_DISABLE;
        m_texCoordIndex[i] = (DWORD)i;
        m_samplerStates[i].addressU = D3DTADDRESS_WRAP;
        m_samplerStates[i].addressV = D3DTADDRESS_WRAP;
        m_samplerStates[i].minFilter = D3DTEXF_LINEAR;
        m_samplerStates[i].magFilter = D3DTEXF_LINEAR;
        m_samplerStates[i].mipFilter = D3DTEXF_LINEAR;
        m_samplerStates[i].borderColor = 0;
        m_texStageStates[i].colorOp = (i == 0) ? D3DTOP_MODULATE : D3DTOP_DISABLE;
        m_texStageStates[i].colorArg0 = D3DTA_CURRENT;
        m_texStageStates[i].colorArg1 = D3DTA_TEXTURE;
        m_texStageStates[i].colorArg2 = D3DTA_CURRENT;
        m_texStageStates[i].alphaOp = (i == 0) ? D3DTOP_SELECTARG1 : D3DTOP_DISABLE;
        m_texStageStates[i].alphaArg0 = D3DTA_CURRENT;
        m_texStageStates[i].alphaArg1 = D3DTA_TEXTURE;
        m_texStageStates[i].alphaArg2 = D3DTA_CURRENT;
        m_texStageStates[i].resultArg = 0;
    }
    for (int i = 0; i < 512; ++i) D3DXMatrixIdentity(&m_transforms[i]);
}

IDirect3DDevice9::~IDirect3DDevice9()
{
    // Rendering resources are owned by the backend and released by their own
    // destructors; the device holds no GL object any more.
}

HRESULT IDirect3DDevice9::BeginScene()
{
    Backend().BeginFrame();
    return S_OK;
}

HRESULT IDirect3DDevice9::EndScene()
{
    return S_OK;
}

// Direct3D applies a texture stage's colour operation even when no texture is
// bound to that stage (an unbound texture behaves as opaque white). The GL
// fixed-function pipeline instead disables texturing entirely, so the stage
// result has to be folded into the primary colour. This matters for the
// character shadow map pass, which draws untextured geometry with
// COLOROP = SELECTARG1 and COLORARG1 = TFACTOR (a flat grey silhouette).
bool IDirect3DDevice9::ComputeUntexturedStage0Color(const float* current, float* out) const
{
    if (m_boundTextures[0])
        return false;

    const TextureStageState& ss = m_texStageStates[0];
    if (ss.colorOp == D3DTOP_DISABLE)
        return false;

    auto readArg = [&](DWORD arg, float* v) {
        switch (arg & D3DTA_SELECTMASK) {
            case D3DTA_TFACTOR:
                v[0] = ((m_textureFactor >> 16) & 0xFF) / 255.0f;
                v[1] = ((m_textureFactor >> 8) & 0xFF) / 255.0f;
                v[2] = (m_textureFactor & 0xFF) / 255.0f;
                v[3] = ((m_textureFactor >> 24) & 0xFF) / 255.0f;
                break;
            case D3DTA_TEXTURE:
                // Unbound texture reads as opaque white.
                v[0] = v[1] = v[2] = v[3] = 1.0f;
                break;
            default:
                v[0] = current[0];
                v[1] = current[1];
                v[2] = current[2];
                v[3] = current[3];
                break;
        }
        if (arg & D3DTA_COMPLEMENT) {
            v[0] = 1.0f - v[0];
            v[1] = 1.0f - v[1];
            v[2] = 1.0f - v[2];
            v[3] = 1.0f - v[3];
        }
        if (arg & D3DTA_ALPHAREPLICATE) {
            v[0] = v[1] = v[2] = v[3];
        }
    };

    float a1[4], a2[4];
    readArg(ss.colorArg1, a1);
    readArg(ss.colorArg2, a2);

    switch (ss.colorOp) {
        case D3DTOP_SELECTARG1:
            for (int i = 0; i < 4; ++i) out[i] = a1[i];
            break;
        case D3DTOP_SELECTARG2:
            for (int i = 0; i < 4; ++i) out[i] = a2[i];
            break;
        case D3DTOP_ADD:
            for (int i = 0; i < 4; ++i) out[i] = a1[i] + a2[i];
            break;
        case D3DTOP_ADDSIGNED:
            for (int i = 0; i < 4; ++i) out[i] = a1[i] + a2[i] - 0.5f;
            break;
        case D3DTOP_SUBTRACT:
            for (int i = 0; i < 4; ++i) out[i] = a1[i] - a2[i];
            break;
        case D3DTOP_MODULATE2X:
            for (int i = 0; i < 4; ++i) out[i] = a1[i] * a2[i] * 2.0f;
            break;
        case D3DTOP_MODULATE4X:
            for (int i = 0; i < 4; ++i) out[i] = a1[i] * a2[i] * 4.0f;
            break;
        case D3DTOP_BLENDTEXTUREALPHA:
            // Unbound texture alpha is 1: the result is ARG2.
            for (int i = 0; i < 4; ++i) out[i] = a2[i];
            break;
        case D3DTOP_BLENDDIFFUSEALPHA:
        case D3DTOP_BLENDCURRENTALPHA:
            for (int i = 0; i < 4; ++i) out[i] = a1[i] + (a2[i] - a1[i]) * current[3];
            break;
        case D3DTOP_BLENDFACTORALPHA:
            for (int i = 0; i < 4; ++i) out[i] = a1[i] + (a2[i] - a1[i]) * current[3];
            break;
        case D3DTOP_MODULATE:
        default:
            for (int i = 0; i < 4; ++i) out[i] = a1[i] * a2[i];
            break;
    }

    for (int i = 0; i < 4; ++i) {
        if (out[i] < 0.0f) out[i] = 0.0f;
        if (out[i] > 1.0f) out[i] = 1.0f;
    }
    return true;
}

void IDirect3DDevice9::FillRenderState(RenderState& state) const
{
    std::memset(&state, 0, sizeof(state));

    state.fvf = m_fvf;
    state.viewportWidth = m_viewport.Width;
    state.viewportHeight = m_viewport.Height;
    state.zEnable = m_zEnable;
    state.zWriteEnable = m_zWriteEnable;
    state.zFunc = m_zFunc;
    state.alphaBlendEnable = m_alphaBlendEnable;
    state.srcBlend = m_srcBlend;
    state.destBlend = m_destBlend;
    state.alphaTestEnable = m_alphaTestEnable;
    state.alphaRef = m_alphaRef;
    state.alphaFunc = m_alphaFunc;
    state.cullMode = m_cullMode;
    state.fillMode = m_fillMode;
    state.fogEnable = m_fogEnable;
    state.fogColor = m_fogColor;
    state.fogVertexMode = m_fogVertexMode;
    state.fogTableMode = m_fogTableMode;
    state.fogStart = m_fogStart;
    state.fogEnd = m_fogEnd;
    state.fogDensity = m_fogDensity;
    state.rangeFogEnable = m_rangeFogEnable;
    state.textureFactor = m_textureFactor;
    state.lighting = m_lighting;
    state.colorVertex = m_colorVertex;
    state.ambient = m_ambient;
    state.inRenderTarget = m_inRenderTarget;

    for (uint32_t stage = 0; stage < kRenderMaxTextureStages; ++stage) {
        IDirect3DTexture9* texture = m_boundTextures[stage];
        state.textures[stage] = texture ? texture->GetTextureHandle() : RenderHandle{0, 0};
        state.textureHasMips[stage] = texture ? texture->HasMipMaps() : false;

        const TextureStageState& ss = m_texStageStates[stage];
        state.stages[stage].colorOp = ss.colorOp;
        state.stages[stage].colorArg0 = ss.colorArg0;
        state.stages[stage].colorArg1 = ss.colorArg1;
        state.stages[stage].colorArg2 = ss.colorArg2;
        state.stages[stage].alphaOp = ss.alphaOp;
        state.stages[stage].alphaArg0 = ss.alphaArg0;
        state.stages[stage].alphaArg1 = ss.alphaArg1;
        state.stages[stage].alphaArg2 = ss.alphaArg2;
        state.stages[stage].resultArg = ss.resultArg;
        state.stages[stage].texCoordIndex = m_texCoordIndex[stage];
        state.stages[stage].texTransformFlags = m_texTransformFlags[stage];

        state.samplers[stage].addressU = m_samplerStates[stage].addressU;
        state.samplers[stage].addressV = m_samplerStates[stage].addressV;
        state.samplers[stage].minFilter = m_samplerStates[stage].minFilter;
        state.samplers[stage].magFilter = m_samplerStates[stage].magFilter;
        state.samplers[stage].mipFilter = m_samplerStates[stage].mipFilter;
        state.samplers[stage].borderColor = m_samplerStates[stage].borderColor;

        CopyMatrix(state.textureMatrices[stage], m_transforms[D3DTS_TEXTURE0 + stage]);
    }

    CopyMatrix(state.projection, m_transforms[D3DTS_PROJECTION]);
    CopyMatrix(state.view, m_transforms[D3DTS_VIEW]);
    CopyMatrix(state.world, m_transforms[D3DTS_WORLD]);
}

HRESULT IDirect3DDevice9::SetRenderTarget(DWORD RenderTargetIndex, IDirect3DSurface9* pRenderTarget)
{
    if (RenderTargetIndex != 0)
        return S_OK;

    // Default framebuffer (window back buffer).
    if (!pRenderTarget || pRenderTarget->IsDefault()) {
        Backend().BindDefaultRenderTarget();
        m_inRenderTarget = false;
        return S_OK;
    }

    if (pRenderTarget->IsDepthStencil())
        return S_OK;

    IDirect3DTexture9* pTexture = pRenderTarget->GetTexture();
    if (!pTexture) {
        Backend().BindDefaultRenderTarget();
        m_inRenderTarget = false;
        return S_OK;
    }

    Backend().BindRenderTarget(pTexture->GetTextureHandle());
    m_inRenderTarget = true;
    return S_OK;
}

HRESULT IDirect3DDevice9::SetDepthStencilSurface(IDirect3DSurface9* pZStencilSurface)
{
    // The default depth buffer is owned by the window framebuffer.
    if (!pZStencilSurface || pZStencilSurface->IsDefault())
        return S_OK;

    if (!pZStencilSurface->IsDepthStencil())
        return S_OK;

    Backend().BindDepthStencil(pZStencilSurface->GetDepthStencil());
    m_inRenderTarget = true;
    return S_OK;
}

HRESULT IDirect3DDevice9::Clear(DWORD /*Count*/, const RECT* /*pRects*/, DWORD Flags, D3DCOLOR Color, float Z, DWORD Stencil)
{
    Backend().Clear((uint32_t)Flags, (uint32_t)Color, Z, (uint32_t)Stencil);
    return S_OK;
}

HRESULT IDirect3DDevice9::SetViewport(const D3DVIEWPORT9* pViewport)
{
    if (pViewport) {
        m_viewport = *pViewport;
        Backend().SetViewport((int32_t)m_viewport.X, (int32_t)m_viewport.Y,
                              m_viewport.Width, m_viewport.Height);
    }
    return S_OK;
}

HRESULT IDirect3DDevice9::SetRenderState(D3DRENDERSTATETYPE State, DWORD Value)
{
    switch (State) {
        case D3DRS_ALPHABLENDENABLE:
            m_alphaBlendEnable = (Value != 0);
            break;
        case D3DRS_SRCBLEND:
            m_srcBlend = Value;
            break;
        case D3DRS_DESTBLEND:
            m_destBlend = Value;
            break;
        case D3DRS_ZENABLE:
            m_zEnable = (Value != 0);
            break;
        case D3DRS_ZWRITEENABLE:
            m_zWriteEnable = (Value != 0);
            break;
        case D3DRS_ZFUNC:
            m_zFunc = Value;
            break;
        case D3DRS_ALPHATESTENABLE:
            m_alphaTestEnable = (Value != 0);
            break;
        case D3DRS_ALPHAREF:
            m_alphaRef = Value;
            break;
        case D3DRS_ALPHAFUNC:
            m_alphaFunc = Value;
            break;
        case D3DRS_CULLMODE:
            m_cullMode = Value;
            break;
        case D3DRS_FILLMODE:
            m_fillMode = Value;
            break;
        case D3DRS_TEXTUREFACTOR:
            m_textureFactor = Value;
            break;
        case D3DRS_LIGHTING:
            m_lighting = Value;
            break;
        case D3DRS_COLORVERTEX:
            m_colorVertex = Value;
            break;
        case D3DRS_AMBIENT:
            m_ambient = Value;
            break;
        case D3DRS_FOGENABLE:
            m_fogEnable = (Value != 0);
            break;
        case D3DRS_FOGCOLOR:
            m_fogColor = Value;
            break;
        case D3DRS_FOGVERTEXMODE:
            m_fogVertexMode = Value;
            break;
        case D3DRS_FOGTABLEMODE:
            m_fogTableMode = Value;
            break;
        case D3DRS_FOGSTART:
            std::memcpy(&m_fogStart, &Value, sizeof(float));
            break;
        case D3DRS_FOGEND:
            std::memcpy(&m_fogEnd, &Value, sizeof(float));
            break;
        case D3DRS_FOGDENSITY:
            std::memcpy(&m_fogDensity, &Value, sizeof(float));
            break;
        case D3DRS_RANGEFOGENABLE:
            m_rangeFogEnable = (Value != 0);
            break;
        default:
            break;
    }
    return S_OK;
}

HRESULT IDirect3DDevice9::GetRenderState(D3DRENDERSTATETYPE State, DWORD* pValue)
{
    if (!pValue) return E_INVALIDARG;
    switch (State) {
        case D3DRS_ALPHABLENDENABLE: *pValue = m_alphaBlendEnable ? 1 : 0; break;
        case D3DRS_SRCBLEND: *pValue = m_srcBlend; break;
        case D3DRS_DESTBLEND: *pValue = m_destBlend; break;
        case D3DRS_ZENABLE: *pValue = m_zEnable ? 1 : 0; break;
        case D3DRS_ZWRITEENABLE: *pValue = m_zWriteEnable ? 1 : 0; break;
        case D3DRS_ZFUNC: *pValue = m_zFunc; break;
        case D3DRS_ALPHATESTENABLE: *pValue = m_alphaTestEnable ? 1 : 0; break;
        case D3DRS_ALPHAREF: *pValue = m_alphaRef; break;
        case D3DRS_ALPHAFUNC: *pValue = m_alphaFunc; break;
        case D3DRS_CULLMODE: *pValue = m_cullMode; break;
        case D3DRS_FILLMODE: *pValue = m_fillMode; break;
        case D3DRS_TEXTUREFACTOR: *pValue = m_textureFactor; break;
        case D3DRS_LIGHTING: *pValue = m_lighting; break;
        case D3DRS_COLORVERTEX: *pValue = m_colorVertex; break;
        case D3DRS_AMBIENT: *pValue = m_ambient; break;
        case D3DRS_FOGENABLE: *pValue = m_fogEnable ? 1 : 0; break;
        case D3DRS_FOGCOLOR: *pValue = m_fogColor; break;
        case D3DRS_FOGVERTEXMODE: *pValue = m_fogVertexMode; break;
        case D3DRS_FOGTABLEMODE: *pValue = m_fogTableMode; break;
        case D3DRS_FOGSTART: std::memcpy(pValue, &m_fogStart, sizeof(float)); break;
        case D3DRS_FOGEND: std::memcpy(pValue, &m_fogEnd, sizeof(float)); break;
        case D3DRS_FOGDENSITY: std::memcpy(pValue, &m_fogDensity, sizeof(float)); break;
        case D3DRS_RANGEFOGENABLE: *pValue = m_rangeFogEnable ? 1 : 0; break;
        default: *pValue = 0; break;
    }
    return S_OK;
}

HRESULT IDirect3DDevice9::SetTextureStageState(DWORD Stage, D3DTEXTURESTAGESTATETYPE Type, DWORD Value)
{
    if (Stage < 8) {
        if (Type == D3DTSS_TEXTURETRANSFORMFLAGS) {
            m_texTransformFlags[Stage] = Value;
        } else if (Type == D3DTSS_TEXCOORDINDEX) {
            m_texCoordIndex[Stage] = Value;
        } else if (Type == D3DTSS_COLOROP) {
            m_texStageStates[Stage].colorOp = Value;
        } else if (Type == D3DTSS_COLORARG0) {
            m_texStageStates[Stage].colorArg0 = Value;
        } else if (Type == D3DTSS_COLORARG1) {
            m_texStageStates[Stage].colorArg1 = Value;
        } else if (Type == D3DTSS_COLORARG2) {
            m_texStageStates[Stage].colorArg2 = Value;
        } else if (Type == D3DTSS_ALPHAOP) {
            m_texStageStates[Stage].alphaOp = Value;
        } else if (Type == D3DTSS_ALPHAARG0) {
            m_texStageStates[Stage].alphaArg0 = Value;
        } else if (Type == D3DTSS_ALPHAARG1) {
            m_texStageStates[Stage].alphaArg1 = Value;
        } else if (Type == D3DTSS_ALPHAARG2) {
            m_texStageStates[Stage].alphaArg2 = Value;
        } else if (Type == D3DTSS_RESULTARG) {
            m_texStageStates[Stage].resultArg = Value;
        }
    }
    return S_OK;
}

HRESULT IDirect3DDevice9::GetTextureStageState(DWORD Stage, D3DTEXTURESTAGESTATETYPE Type, DWORD* pValue)
{
    if (!pValue) return E_INVALIDARG;
    if (Stage < 8) {
        if (Type == D3DTSS_TEXTURETRANSFORMFLAGS) {
            *pValue = m_texTransformFlags[Stage];
            return S_OK;
        } else if (Type == D3DTSS_TEXCOORDINDEX) {
            *pValue = m_texCoordIndex[Stage];
            return S_OK;
        } else if (Type == D3DTSS_COLOROP) {
            *pValue = m_texStageStates[Stage].colorOp;
            return S_OK;
        } else if (Type == D3DTSS_COLORARG0) {
            *pValue = m_texStageStates[Stage].colorArg0;
            return S_OK;
        } else if (Type == D3DTSS_COLORARG1) {
            *pValue = m_texStageStates[Stage].colorArg1;
            return S_OK;
        } else if (Type == D3DTSS_COLORARG2) {
            *pValue = m_texStageStates[Stage].colorArg2;
            return S_OK;
        } else if (Type == D3DTSS_ALPHAOP) {
            *pValue = m_texStageStates[Stage].alphaOp;
            return S_OK;
        } else if (Type == D3DTSS_ALPHAARG0) {
            *pValue = m_texStageStates[Stage].alphaArg0;
            return S_OK;
        } else if (Type == D3DTSS_ALPHAARG1) {
            *pValue = m_texStageStates[Stage].alphaArg1;
            return S_OK;
        } else if (Type == D3DTSS_ALPHAARG2) {
            *pValue = m_texStageStates[Stage].alphaArg2;
            return S_OK;
        } else if (Type == D3DTSS_RESULTARG) {
            *pValue = m_texStageStates[Stage].resultArg;
            return S_OK;
        }
    }
    *pValue = 0;
    return S_OK;
}

HRESULT IDirect3DDevice9::SetSamplerState(DWORD Stage, D3DSAMPLERSTATETYPE Type, DWORD Value)
{
    if (Stage < 8) {
        if (Type == D3DSAMP_ADDRESSU) {
            m_samplerStates[Stage].addressU = Value;
        } else if (Type == D3DSAMP_ADDRESSV) {
            m_samplerStates[Stage].addressV = Value;
        } else if (Type == D3DSAMP_MAGFILTER) {
            m_samplerStates[Stage].magFilter = Value;
        } else if (Type == D3DSAMP_MINFILTER) {
            m_samplerStates[Stage].minFilter = Value;
        } else if (Type == D3DSAMP_MIPFILTER) {
            m_samplerStates[Stage].mipFilter = Value;
        } else if (Type == D3DSAMP_BORDERCOLOR) {
            m_samplerStates[Stage].borderColor = Value;
        }
    }
    return S_OK;
}

HRESULT IDirect3DDevice9::GetSamplerState(DWORD Stage, D3DSAMPLERSTATETYPE Type, DWORD* pValue)
{
    if (!pValue) return E_INVALIDARG;
    if (Stage < 8) {
        if (Type == D3DSAMP_ADDRESSU) {
            *pValue = m_samplerStates[Stage].addressU;
            return S_OK;
        } else if (Type == D3DSAMP_ADDRESSV) {
            *pValue = m_samplerStates[Stage].addressV;
            return S_OK;
        } else if (Type == D3DSAMP_MAGFILTER) {
            *pValue = m_samplerStates[Stage].magFilter;
            return S_OK;
        } else if (Type == D3DSAMP_MINFILTER) {
            *pValue = m_samplerStates[Stage].minFilter;
            return S_OK;
        } else if (Type == D3DSAMP_MIPFILTER) {
            *pValue = m_samplerStates[Stage].mipFilter;
            return S_OK;
        } else if (Type == D3DSAMP_BORDERCOLOR) {
            *pValue = m_samplerStates[Stage].borderColor;
            return S_OK;
        }
    }
    *pValue = 0;
    return S_OK;
}

HRESULT IDirect3DDevice9::SetTexture(DWORD Stage, IDirect3DBaseTexture9* pTexture)
{
    if (Stage < 8) {
        m_boundTextures[Stage] = static_cast<IDirect3DTexture9*>(pTexture);
        RenderHandle handle{0, 0};
        if (m_boundTextures[Stage])
            handle = m_boundTextures[Stage]->GetTextureHandle();
        Backend().BindStageTexture(Stage, handle);
    }
    return S_OK;
}

HRESULT IDirect3DDevice9::GetTexture(DWORD Stage, IDirect3DBaseTexture9** ppTexture)
{
    if (ppTexture && Stage < 8) *ppTexture = m_boundTextures[Stage];
    return S_OK;
}

HRESULT IDirect3DDevice9::DrawPrimitive(D3DPRIMITIVETYPE PrimitiveType, UINT StartVertex, UINT PrimitiveCount)
{
    if (!m_streams[0].vb || !m_streams[0].vb->GetData()) return S_OK;

    UINT stride = m_streams[0].stride ? m_streams[0].stride : D3DXGetFVFVertexSize(m_fvf);
    if (!stride) stride = 24;
    const BYTE* vbData = (const BYTE*)m_streams[0].vb->GetData() + m_streams[0].offset;

    DrawCall call;
    FillRenderState(call.state);

    float primaryColor[4] = { 1.0f, 1.0f, 1.0f, 1.0f };
    if (m_lighting && !(m_fvf & D3DFVF_DIFFUSE)) {
        primaryColor[0] = m_material.Diffuse.r;
        primaryColor[1] = m_material.Diffuse.g;
        primaryColor[2] = m_material.Diffuse.b;
        primaryColor[3] = m_material.Diffuse.a;
    }
    if (!(m_fvf & D3DFVF_DIFFUSE)) {
        float stageColor[4];
        if (ComputeUntexturedStage0Color(primaryColor, stageColor)) {
            for (int i = 0; i < 4; ++i)
                primaryColor[i] = stageColor[i];
        }
    }

    call.primitiveType = (uint32_t)PrimitiveType;
    call.vertexData = vbData;
    call.stride = stride;
    call.fvf = m_fvf;
    call.startVertex = StartVertex;
    call.vertexCount = PrimitiveCountToVertexCount(PrimitiveType, PrimitiveCount);
    call.indexData = nullptr;
    call.indexCount = 0;
    call.index32 = false;
    call.baseVertex = 0;
    call.minVertexIndex = 0;
    call.numVertices = call.vertexCount;
    call.startIndex = 0;
    call.primitiveCount = PrimitiveCount;
    std::memcpy(call.primaryColor, primaryColor, sizeof(primaryColor));

    Backend().Draw(call);
    return S_OK;
}

HRESULT IDirect3DDevice9::DrawIndexedPrimitive(D3DPRIMITIVETYPE PrimitiveType, INT BaseVertexIndex, UINT /*MinIndex*/, UINT NumVertices, UINT StartIndex, UINT PrimitiveCount)
{
    if (!m_streams[0].vb || !m_pIndexBuffer) return S_OK;
    if (!m_streams[0].vb->GetData() || !m_pIndexBuffer->GetData()) return S_OK;

    UINT stride = m_streams[0].stride ? m_streams[0].stride : D3DXGetFVFVertexSize(m_fvf);
    if (!stride) stride = 24;
    const BYTE* vbData = (const BYTE*)m_streams[0].vb->GetData() + m_streams[0].offset + (BaseVertexIndex * stride);
    const WORD* ibData = (const WORD*)m_pIndexBuffer->GetData() + StartIndex;

    DrawCall call;
    FillRenderState(call.state);

    float primaryColor[4] = { 1.0f, 1.0f, 1.0f, 1.0f };
    if (m_lighting && !(m_fvf & D3DFVF_DIFFUSE)) {
        primaryColor[0] = m_material.Diffuse.r;
        primaryColor[1] = m_material.Diffuse.g;
        primaryColor[2] = m_material.Diffuse.b;
        primaryColor[3] = m_material.Diffuse.a;
    }
    if (!(m_fvf & D3DFVF_DIFFUSE)) {
        float stageColor[4];
        if (ComputeUntexturedStage0Color(primaryColor, stageColor)) {
            for (int i = 0; i < 4; ++i)
                primaryColor[i] = stageColor[i];
        }
    }

    call.primitiveType = (uint32_t)PrimitiveType;
    call.vertexData = vbData;
    call.stride = stride;
    call.fvf = m_fvf;
    call.startVertex = 0;
    call.vertexCount = PrimitiveCountToVertexCount(PrimitiveType, PrimitiveCount);
    call.indexData = ibData;
    call.indexCount = call.vertexCount;
    call.index32 = false;
    call.baseVertex = BaseVertexIndex;
    call.minVertexIndex = 0;
    call.numVertices = NumVertices;
    call.startIndex = StartIndex;
    call.primitiveCount = PrimitiveCount;
    std::memcpy(call.primaryColor, primaryColor, sizeof(primaryColor));

    Backend().Draw(call);
    return S_OK;
}

HRESULT IDirect3DDevice9::DrawPrimitiveUP(D3DPRIMITIVETYPE PrimitiveType, UINT PrimitiveCount, const void* pVertexStreamZeroData, UINT VertexStreamZeroStride)
{
    if (!pVertexStreamZeroData) return S_OK;

    UINT stride = VertexStreamZeroStride ? VertexStreamZeroStride : D3DXGetFVFVertexSize(m_fvf);
    if (!stride) stride = 24;
    const BYTE* vbData = (const BYTE*)pVertexStreamZeroData;

    DrawCall call;
    FillRenderState(call.state);

    float primaryColor[4] = { 1.0f, 1.0f, 1.0f, 1.0f };
    if (m_lighting && !(m_fvf & D3DFVF_DIFFUSE)) {
        primaryColor[0] = m_material.Diffuse.r;
        primaryColor[1] = m_material.Diffuse.g;
        primaryColor[2] = m_material.Diffuse.b;
        primaryColor[3] = m_material.Diffuse.a;
    }
    if (!(m_fvf & D3DFVF_DIFFUSE)) {
        float stageColor[4];
        if (ComputeUntexturedStage0Color(primaryColor, stageColor)) {
            for (int i = 0; i < 4; ++i)
                primaryColor[i] = stageColor[i];
        }
    }

    call.primitiveType = (uint32_t)PrimitiveType;
    call.vertexData = vbData;
    call.stride = stride;
    call.fvf = m_fvf;
    call.startVertex = 0;
    call.vertexCount = PrimitiveCountToVertexCount(PrimitiveType, PrimitiveCount);
    call.indexData = nullptr;
    call.indexCount = 0;
    call.index32 = false;
    call.baseVertex = 0;
    call.minVertexIndex = 0;
    call.numVertices = call.vertexCount;
    call.startIndex = 0;
    call.primitiveCount = PrimitiveCount;
    std::memcpy(call.primaryColor, primaryColor, sizeof(primaryColor));

    Backend().Draw(call);
    return S_OK;
}

HRESULT IDirect3DDevice9::DrawIndexedPrimitiveUP(D3DPRIMITIVETYPE PrimitiveType, UINT /*MinVertexIndex*/, UINT NumVertexIndices, UINT PrimitiveCount, const void* pIndexData, D3DFORMAT IndexDataFormat, const void* pVertexStreamZeroData, UINT VertexStreamZeroStride)
{
    if (!pVertexStreamZeroData || !pIndexData) return S_OK;

    UINT stride = VertexStreamZeroStride ? VertexStreamZeroStride : D3DXGetFVFVertexSize(m_fvf);
    if (!stride) stride = 24;
    const BYTE* vbData = (const BYTE*)pVertexStreamZeroData;

    DrawCall call;
    FillRenderState(call.state);

    float primaryColor[4] = { 1.0f, 1.0f, 1.0f, 1.0f };
    if (m_lighting && !(m_fvf & D3DFVF_DIFFUSE)) {
        primaryColor[0] = m_material.Diffuse.r;
        primaryColor[1] = m_material.Diffuse.g;
        primaryColor[2] = m_material.Diffuse.b;
        primaryColor[3] = m_material.Diffuse.a;
    }
    if (!(m_fvf & D3DFVF_DIFFUSE)) {
        float stageColor[4];
        if (ComputeUntexturedStage0Color(primaryColor, stageColor)) {
            for (int i = 0; i < 4; ++i)
                primaryColor[i] = stageColor[i];
        }
    }

    call.primitiveType = (uint32_t)PrimitiveType;
    call.vertexData = vbData;
    call.stride = stride;
    call.fvf = m_fvf;
    call.startVertex = 0;
    call.vertexCount = PrimitiveCountToVertexCount(PrimitiveType, PrimitiveCount);
    call.indexData = pIndexData;
    call.indexCount = call.vertexCount;
    call.index32 = (IndexDataFormat == D3DFMT_INDEX32);
    call.baseVertex = 0;
    call.minVertexIndex = 0;
    call.numVertices = NumVertexIndices;
    call.startIndex = 0;
    call.primitiveCount = PrimitiveCount;
    std::memcpy(call.primaryColor, primaryColor, sizeof(primaryColor));

    Backend().Draw(call);
    return S_OK;
}

#endif // OPENMT2_RENDERER_GL
