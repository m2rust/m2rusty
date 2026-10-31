#pragma once

// ============================================================================
// OffscreenGL.h - EGL surfaceless target for the headless render goldens
// ============================================================================
// The renderer under test is the D3D9 compatibility façade, which draws with
// the fixed-function OpenGL pipeline. This helper gives it a framebuffer
// without a window system:
//
//   EGL_MESA_platform_surfaceless + llvmpipe -> FBO (RGBA8 + depth24)
//
// No Xvfb, no display server and no GPU are required, so `ctest` stays
// headless in CI. The caller keeps the context current and owns the D3D9
// device; Capture() only reads the colour attachment back.

#include <string>

#include "support/Png.h"

namespace render
{

class OffscreenGL
{
public:
    OffscreenGL();
    ~OffscreenGL();

    OffscreenGL(const OffscreenGL&) = delete;
    OffscreenGL& operator=(const OffscreenGL&) = delete;

    bool Create(int width, int height, std::string* error);
    void Destroy();

    bool Capture(Image* image, std::string* error);

    int Width() const { return m_width; }
    int Height() const { return m_height; }
    const std::string& Renderer() const { return m_renderer; }

private:
    struct Impl;
    Impl* m_impl = nullptr;
    int m_width = 0;
    int m_height = 0;
    std::string m_renderer;
};

} // namespace render
