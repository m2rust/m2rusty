#pragma once

// OpenGL backend (T1.7).
//
// Everything in EterBase that touches an OpenGL symbol lives under
// src/backends/gl/ (invariant R1). The class is a pass-through: it translates
// the D3D9 vocabulary of the snapshot into the fixed-function GL calls the
// pre-seam renderer issued, in the same order.

#include <Platform/RenderBackend.h>

#include <vector>

namespace eterbase {
namespace gl {

class GLRenderBackend final : public IRenderBackend {
public:
    GLRenderBackend() = default;
    ~GLRenderBackend() override;

    RenderCaps Caps() const override;

    RenderHandle CreateTexture() override;
    RenderHandle CreateDepthStencil(uint32_t width, uint32_t height) override;
    void Destroy(RenderHandle handle) override;

    void UploadTextureLevel(RenderHandle texture, uint32_t level, uint32_t format,
                            uint32_t width, uint32_t height, const void* data,
                            size_t size) override;
    void FinalizeTexture(RenderHandle texture, uint32_t maxLevel) override;

    void BindDefaultRenderTarget() override;
    void BindRenderTarget(RenderHandle colorTexture) override;
    void BindDepthStencil(RenderHandle depthStencil) override;
    void SetViewport(int32_t x, int32_t y, uint32_t width, uint32_t height) override;
    void Clear(uint32_t flags, uint32_t color, float z, uint32_t stencil) override;

    void BeginFrame() override;
    void Present() override;

    void BindStageTexture(uint32_t stage, RenderHandle texture) override;
    void ApplyState(const RenderState& state) override;
    void Draw(const DrawCall& call) override;

    bool Readback(uint32_t x, uint32_t y, uint32_t width, uint32_t height,
                  uint32_t* outArgb) override;

private:
    enum class ResourceKind : uint32_t { Texture = 1, DepthStencil = 2 };

    struct ResourceSlot {
        uint32_t glName = 0;
        uint32_t generation = 0;
        ResourceKind kind = ResourceKind::Texture;
        bool used = false;
    };

    ResourceSlot* Lookup(RenderHandle handle, ResourceKind kind);
    void EnsureFramebuffer();
    void EnsureCaps() const;

    std::vector<ResourceSlot> m_resources;
    uint32_t m_framebuffer = 0;
    mutable bool m_capsLoaded = false;
    mutable RenderCaps m_caps{};
};

} // namespace gl
} // namespace eterbase
