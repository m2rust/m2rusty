#include "support/OffscreenGL.h"

#ifndef GL_GLEXT_PROTOTYPES
#define GL_GLEXT_PROTOTYPES 1
#endif
#include <GL/gl.h>
#include <GL/glext.h>

#include <EGL/egl.h>
#include <EGL/eglext.h>

#include <cstdio>
#include <cstring>

#ifndef GL_CLIP_ORIGIN
#define GL_CLIP_ORIGIN 0x935C
#endif
#ifndef GL_LOWER_LEFT
#define GL_LOWER_LEFT 0x8CA1
#endif
#ifndef GL_ZERO_TO_ONE
#define GL_ZERO_TO_ONE 0x935F
#endif

namespace render
{

struct OffscreenGL::Impl
{
    EGLDisplay display = EGL_NO_DISPLAY;
    EGLContext context = EGL_NO_CONTEXT;
    GLuint framebuffer = 0;
    GLuint colorTexture = 0;
    GLuint depthRenderbuffer = 0;
};

namespace
{

const char* EglErrorString(EGLint error)
{
    switch (error)
    {
        case EGL_SUCCESS: return "EGL_SUCCESS";
        case EGL_NOT_INITIALIZED: return "EGL_NOT_INITIALIZED";
        case EGL_BAD_ACCESS: return "EGL_BAD_ACCESS";
        case EGL_BAD_ALLOC: return "EGL_BAD_ALLOC";
        case EGL_BAD_ATTRIBUTE: return "EGL_BAD_ATTRIBUTE";
        case EGL_BAD_CONFIG: return "EGL_BAD_CONFIG";
        case EGL_BAD_CONTEXT: return "EGL_BAD_CONTEXT";
        case EGL_BAD_CURRENT_SURFACE: return "EGL_BAD_CURRENT_SURFACE";
        case EGL_BAD_DISPLAY: return "EGL_BAD_DISPLAY";
        case EGL_BAD_MATCH: return "EGL_BAD_MATCH";
        case EGL_BAD_NATIVE_PIXMAP: return "EGL_BAD_NATIVE_PIXMAP";
        case EGL_BAD_NATIVE_WINDOW: return "EGL_BAD_NATIVE_WINDOW";
        case EGL_BAD_PARAMETER: return "EGL_BAD_PARAMETER";
        case EGL_BAD_SURFACE: return "EGL_BAD_SURFACE";
        case EGL_CONTEXT_LOST: return "EGL_CONTEXT_LOST";
        default: return "unknown EGL error";
    }
}

void Fail(std::string* error, const char* what)
{
    if (!error)
        return;
    *error = std::string(what) + ": " + EglErrorString(eglGetError());
}

} // namespace

OffscreenGL::OffscreenGL() = default;

OffscreenGL::~OffscreenGL()
{
    Destroy();
}

bool OffscreenGL::Create(int width, int height, std::string* error)
{
    Destroy();
    if (width <= 0 || height <= 0)
    {
        if (error)
            *error = "invalid offscreen target size";
        return false;
    }

    m_impl = new Impl();

    auto getPlatformDisplay = reinterpret_cast<PFNEGLGETPLATFORMDISPLAYEXTPROC>(
        eglGetProcAddress("eglGetPlatformDisplayEXT"));

    if (getPlatformDisplay)
    {
        m_impl->display = getPlatformDisplay(EGL_PLATFORM_SURFACELESS_MESA,
                                             EGL_DEFAULT_DISPLAY, nullptr);
    }
    else
    {
        // Older loaders without the platform extension: fall back to the
        // default display; surfaceless still works when EGL_PLATFORM is set.
        m_impl->display = eglGetDisplay(EGL_DEFAULT_DISPLAY);
    }

    if (m_impl->display == EGL_NO_DISPLAY)
    {
        Fail(error, "eglGetPlatformDisplayEXT(EGL_PLATFORM_SURFACELESS_MESA) failed");
        Destroy();
        return false;
    }

    EGLint eglMajor = 0;
    EGLint eglMinor = 0;
    if (!eglInitialize(m_impl->display, &eglMajor, &eglMinor))
    {
        Fail(error, "eglInitialize failed");
        Destroy();
        return false;
    }

    if (!eglBindAPI(EGL_OPENGL_API))
    {
        Fail(error, "eglBindAPI(EGL_OPENGL_API) failed");
        Destroy();
        return false;
    }

    const EGLint configAttributes[] = {
        EGL_SURFACE_TYPE, 0, // surfaceless: rendering goes to an FBO
        EGL_RENDERABLE_TYPE, EGL_OPENGL_BIT,
        EGL_RED_SIZE, 8,
        EGL_GREEN_SIZE, 8,
        EGL_BLUE_SIZE, 8,
        EGL_ALPHA_SIZE, 8,
        EGL_DEPTH_SIZE, 24,
        EGL_NONE,
    };

    EGLConfig config = nullptr;
    EGLint configCount = 0;
    if (!eglChooseConfig(m_impl->display, configAttributes, &config, 1, &configCount) ||
        configCount < 1)
    {
        Fail(error, "eglChooseConfig found no OpenGL configuration");
        Destroy();
        return false;
    }

    // Match CMSWindow (MSWindow.cpp): the renderer is a GL 2.1 compatibility
    // pipeline, not a core profile.
    const EGLint contextAttributes[] = {
        EGL_CONTEXT_MAJOR_VERSION, 2,
        EGL_CONTEXT_MINOR_VERSION, 1,
        EGL_NONE,
    };

    m_impl->context = eglCreateContext(m_impl->display, config, EGL_NO_CONTEXT, contextAttributes);
    if (m_impl->context == EGL_NO_CONTEXT)
    {
        Fail(error, "eglCreateContext failed");
        Destroy();
        return false;
    }

    if (!eglMakeCurrent(m_impl->display, EGL_NO_SURFACE, EGL_NO_SURFACE, m_impl->context))
    {
        Fail(error, "eglMakeCurrent failed");
        Destroy();
        return false;
    }

    const GLubyte* version = glGetString(GL_VERSION);
    const GLubyte* renderer = glGetString(GL_RENDERER);
    m_renderer = std::string(renderer ? reinterpret_cast<const char*>(renderer) : "unknown") +
                 " / GL " + (version ? reinterpret_cast<const char*>(version) : "unknown");

    glGenFramebuffers(1, &m_impl->framebuffer);
    glBindFramebuffer(GL_FRAMEBUFFER, m_impl->framebuffer);

    glGenTextures(1, &m_impl->colorTexture);
    glBindTexture(GL_TEXTURE_2D, m_impl->colorTexture);
    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA8, width, height, 0, GL_RGBA, GL_UNSIGNED_BYTE, nullptr);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_NEAREST);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_NEAREST);
    glBindTexture(GL_TEXTURE_2D, 0);
    glFramebufferTexture2D(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D,
                           m_impl->colorTexture, 0);

    glGenRenderbuffers(1, &m_impl->depthRenderbuffer);
    glBindRenderbuffer(GL_RENDERBUFFER, m_impl->depthRenderbuffer);
    glRenderbufferStorage(GL_RENDERBUFFER, GL_DEPTH_COMPONENT24, width, height);
    glFramebufferRenderbuffer(GL_FRAMEBUFFER, GL_DEPTH_ATTACHMENT, GL_RENDERBUFFER,
                              m_impl->depthRenderbuffer);
    glBindRenderbuffer(GL_RENDERBUFFER, 0);

    const GLenum status = glCheckFramebufferStatus(GL_FRAMEBUFFER);
    if (status != GL_FRAMEBUFFER_COMPLETE)
    {
        if (error)
        {
            char message[96];
            std::snprintf(message, sizeof(message),
                          "offscreen framebuffer incomplete (0x%04x)", status);
            *error = message;
        }
        Destroy();
        return false;
    }

    // D3D9 depth is [0, 1] and the client enables clip control in MSWindow.
    // Mirror that here so depth-tested scenes exercise the same pipeline.
    auto clipControl = reinterpret_cast<PFNGLCLIPCONTROLPROC>(eglGetProcAddress("glClipControl"));
    if (clipControl)
        clipControl(GL_LOWER_LEFT, GL_ZERO_TO_ONE);

    glDepthRange(0.0, 1.0);
    glDisable(GL_MULTISAMPLE);
    glDisable(GL_DITHER);
    glPixelStorei(GL_PACK_ALIGNMENT, 1);
    glViewport(0, 0, width, height);

    m_width = width;
    m_height = height;
    return true;
}

void OffscreenGL::Destroy()
{
    if (!m_impl)
        return;

    if (m_impl->display != EGL_NO_DISPLAY && m_impl->context != EGL_NO_CONTEXT)
    {
        eglMakeCurrent(m_impl->display, EGL_NO_SURFACE, EGL_NO_SURFACE, EGL_NO_CONTEXT);
        eglDestroyContext(m_impl->display, m_impl->context);
    }

    if (m_impl->display != EGL_NO_DISPLAY)
        eglTerminate(m_impl->display);

    delete m_impl;
    m_impl = nullptr;
    m_width = 0;
    m_height = 0;
    m_renderer.clear();
}

bool OffscreenGL::Capture(Image* image, std::string* error)
{
    if (!m_impl || !image)
    {
        if (error)
            *error = "offline target is not initialized";
        return false;
    }

    glFinish();
    glBindFramebuffer(GL_FRAMEBUFFER, m_impl->framebuffer);
    glReadBuffer(GL_COLOR_ATTACHMENT0);

    std::vector<std::uint8_t> bottomUp(static_cast<std::size_t>(m_width) * m_height * 4);
    glReadPixels(0, 0, m_width, m_height, GL_RGBA, GL_UNSIGNED_BYTE, bottomUp.data());
    const GLenum glError = glGetError();
    if (glError != GL_NO_ERROR)
    {
        if (error)
        {
            char message[64];
            std::snprintf(message, sizeof(message), "glReadPixels failed (0x%04x)", glError);
            *error = message;
        }
        return false;
    }

    // PNG and stb_image are top-down; glReadPixels starts at the bottom row.
    image->width = m_width;
    image->height = m_height;
    image->rgba.resize(bottomUp.size());
    const std::size_t rowBytes = static_cast<std::size_t>(m_width) * 4;
    for (int y = 0; y < m_height; ++y)
    {
        const std::uint8_t* source = bottomUp.data() + static_cast<std::size_t>(m_height - 1 - y) * rowBytes;
        std::memcpy(image->rgba.data() + static_cast<std::size_t>(y) * rowBytes, source, rowBytes);
    }
    return image->Valid();
}

} // namespace render
