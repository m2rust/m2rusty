#pragma once

// Renderer seam (T1.7, decision D11).
//
// The contract between the Direct3D 9 compatibility façade
// (src/Direct3DDevice9Compat.cpp) and a rendering backend. The GL backend in
// src/backends/gl/ is the only place in EterBase allowed to call gl* (R1);
// resources are named by opaque RenderHandle values (R2) and the façade only
// ever hands the backend state snapshots (R3) through a single Draw() entry
// point (R4).
//
// The seam is deliberately pass-through: D3D9 enum values (D3DRS_*, D3DTSS_*,
// D3DFMT_*, …) are carried verbatim in the snapshot and translated inside the
// backend. The D3D9 façade is the only stable contract above the seam, so
// translation lives below it where a Vulkan backend will need the same
// information in its pipeline-state key.

#include <cstddef>
#include <cstdint>

// Capability queries replace #ifdef feature tests (R5). A backend reports what
// this particular device/driver supports; the façade and the game above it must
// ask instead of assuming.
struct RenderCaps {
    uint32_t maxTextureSize;
    bool dxt1;
    bool dxt5;
    bool twoSidedStencil;
    bool separateAlphaBlend;
    bool fixedFunction;
};

// Opaque backend-owned resource name. Vulkan handles are not integers, so the
// façade never inspects `id`; `generation` lets the backend detect a stale
// handle after a slot is recycled.
struct RenderHandle {
    uint32_t id;
    uint32_t generation;

    bool IsValid() const { return id != 0; }
};

// Number of texture stages the D3D9 fixed-function emulation models. The GL
// backend only *applies* the first two (matching the pre-seam renderer); the
// remaining slots are carried so a synthesising backend can key on them.
enum : uint32_t { kRenderMaxTextureStages = 8 };

// A full fixed-function snapshot. It is a value type on purpose: the backend
// must be able to derive its pipeline state from the snapshot alone (R3), and a
// Vulkan backend will hash it into its PSO cache.
struct RenderState {
    struct TextureStage {
        uint32_t colorOp;
        uint32_t colorArg0;
        uint32_t colorArg1;
        uint32_t colorArg2;
        uint32_t alphaOp;
        uint32_t alphaArg0;
        uint32_t alphaArg1;
        uint32_t alphaArg2;
        uint32_t resultArg;
        uint32_t texCoordIndex;       // D3DTSS_TEXCOORDINDEX (low word) | TCI mode (high word)
        uint32_t texTransformFlags;   // D3DTSS_TEXTURETRANSFORMFLAGS
    };

    struct SamplerStage {
        uint32_t addressU;
        uint32_t addressV;
        uint32_t minFilter;
        uint32_t magFilter;
        uint32_t mipFilter;
        uint32_t borderColor;
    };

    // Render states.
    uint32_t fvf;
    uint32_t viewportWidth;
    uint32_t viewportHeight;
    bool zEnable;
    bool zWriteEnable;
    uint32_t zFunc;
    bool alphaBlendEnable;
    uint32_t srcBlend;
    uint32_t destBlend;
    bool alphaTestEnable;
    uint32_t alphaRef;
    uint32_t alphaFunc;
    uint32_t cullMode;
    uint32_t fillMode;
    bool fogEnable;
    uint32_t fogColor;
    uint32_t fogVertexMode;
    uint32_t fogTableMode;
    float fogStart;
    float fogEnd;
    float fogDensity;
    bool rangeFogEnable;
    uint32_t textureFactor;
    uint32_t lighting;
    uint32_t colorVertex;
    uint32_t ambient;

    // Render targets: the client's D3D render-target textures are stored top
    // row first, so the backend flips clip-space Y while this is set.
    bool inRenderTarget;

    RenderHandle textures[kRenderMaxTextureStages];
    bool textureHasMips[kRenderMaxTextureStages];
    TextureStage stages[kRenderMaxTextureStages];
    SamplerStage samplers[kRenderMaxTextureStages];

    // Transforms (D3DTS_PROJECTION, D3DTS_VIEW, D3DTS_WORLD, D3DTS_TEXTURE0…).
    float projection[16];
    float view[16];
    float world[16];
    float textureMatrices[kRenderMaxTextureStages][16];
};

// One draw with the state it must execute against (R4). Buffers stay CPU-side
// today: the client locks them immediately, so the seam passes resolved
// pointers plus the ranges a backend would need for a real vertex buffer.
struct DrawCall {
    RenderState state;

    uint32_t primitiveType;   // D3DPRIMITIVETYPE
    const void* vertexData;   // stream 0, already offset by base vertex
    uint32_t stride;
    uint32_t fvf;

    uint32_t startVertex;     // first vertex for non-indexed draws
    uint32_t vertexCount;     // resolved vertex count
    const void* indexData;    // null for non-indexed draws
    uint32_t indexCount;
    bool index32;

    int32_t baseVertex;
    uint32_t minVertexIndex;
    uint32_t numVertices;
    uint32_t startIndex;
    uint32_t primitiveCount;

    // Primary colour used when the FVF carries no diffuse channel.
    float primaryColor[4];
};

class IRenderBackend {
public:
    virtual ~IRenderBackend() = default;

    // Capabilities (R5).
    virtual RenderCaps Caps() const = 0;

    // Resources. Names are opaque and backend-owned (R2).
    virtual RenderHandle CreateTexture() = 0;
    virtual RenderHandle CreateDepthStencil(uint32_t width, uint32_t height) = 0;
    virtual void Destroy(RenderHandle handle) = 0;

    // Uploads one mip level. `format` is a D3DFORMAT value; the backend owns the
    // format translation (DXT1/3/5, 16-bit expansion, BGRA).
    virtual void UploadTextureLevel(RenderHandle texture, uint32_t level,
                                    uint32_t format, uint32_t width, uint32_t height,
                                    const void* data, size_t size) = 0;
    // Applies the mip/filter defaults a texture has before any sampler state.
    virtual void FinalizeTexture(RenderHandle texture, uint32_t maxLevel) = 0;

    // Render targets and frame (R6).
    virtual void BindDefaultRenderTarget() = 0;
    virtual void BindRenderTarget(RenderHandle colorTexture) = 0;
    virtual void BindDepthStencil(RenderHandle depthStencil) = 0;
    virtual void SetViewport(int32_t x, int32_t y, uint32_t width, uint32_t height) = 0;
    virtual void Clear(uint32_t flags, uint32_t color, float z, uint32_t stencil) = 0;

    virtual void BeginFrame() = 0;
    virtual void Present() = 0;

    // Binds a stage texture immediately, preserving the D3D9 façade's
    // SetTexture timing.
    virtual void BindStageTexture(uint32_t stage, RenderHandle texture) = 0;

    // Applies a full snapshot. The GL backend is immediate and issues the
    // corresponding GL state now; a compiled backend records the key instead.
    virtual void ApplyState(const RenderState& state) = 0;

    virtual void Draw(const DrawCall& call) = 0;

    // The only readback entry point (R7): screenshots and conformance.
    virtual bool Readback(uint32_t x, uint32_t y, uint32_t width, uint32_t height,
                          uint32_t* outArgb) = 0;
};

// Process-wide backend, selected at configure time (R8). Created on first use
// so headless tests that never draw do not need a GL context.
IRenderBackend& Platform_GetRenderBackend();
