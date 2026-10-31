#include <Platform/GLCompat.h>

#if defined(_WIN32)

PFNGLACTIVETEXTUREPROC glActiveTexture = nullptr;
PFNGLCLIENTACTIVETEXTUREPROC glClientActiveTexture = nullptr;
PFNGLCOMPRESSEDTEXIMAGE2DPROC glCompressedTexImage2D = nullptr;
PFNGLBINDFRAMEBUFFERPROC glBindFramebuffer = nullptr;
PFNGLGENFRAMEBUFFERSPROC glGenFramebuffers = nullptr;
PFNGLDELETEFRAMEBUFFERSPROC glDeleteFramebuffers = nullptr;
PFNGLCHECKFRAMEBUFFERSTATUSPROC glCheckFramebufferStatus = nullptr;
PFNGLFRAMEBUFFERTEXTURE2DPROC glFramebufferTexture2D = nullptr;
PFNGLFRAMEBUFFERRENDERBUFFERPROC glFramebufferRenderbuffer = nullptr;
PFNGLBINDRENDERBUFFERPROC glBindRenderbuffer = nullptr;
PFNGLGENRENDERBUFFERSPROC glGenRenderbuffers = nullptr;
PFNGLDELETERENDERBUFFERSPROC glDeleteRenderbuffers = nullptr;
PFNGLRENDERBUFFERSTORAGEPROC glRenderbufferStorage = nullptr;
PFNGLCLIPCONTROLPROC glClipControl = nullptr;

bool Platform_LoadGLFunctions(void* (*getProcAddress)(const char*)) {
    if (!getProcAddress) return false;

    glActiveTexture = (PFNGLACTIVETEXTUREPROC)getProcAddress("glActiveTexture");
    glClientActiveTexture = (PFNGLCLIENTACTIVETEXTUREPROC)getProcAddress("glClientActiveTexture");
    glCompressedTexImage2D = (PFNGLCOMPRESSEDTEXIMAGE2DPROC)getProcAddress("glCompressedTexImage2D");
    glBindFramebuffer = (PFNGLBINDFRAMEBUFFERPROC)getProcAddress("glBindFramebuffer");
    glGenFramebuffers = (PFNGLGENFRAMEBUFFERSPROC)getProcAddress("glGenFramebuffers");
    glDeleteFramebuffers = (PFNGLDELETEFRAMEBUFFERSPROC)getProcAddress("glDeleteFramebuffers");
    glCheckFramebufferStatus = (PFNGLCHECKFRAMEBUFFERSTATUSPROC)getProcAddress("glCheckFramebufferStatus");
    glFramebufferTexture2D = (PFNGLFRAMEBUFFERTEXTURE2DPROC)getProcAddress("glFramebufferTexture2D");
    glFramebufferRenderbuffer = (PFNGLFRAMEBUFFERRENDERBUFFERPROC)getProcAddress("glFramebufferRenderbuffer");
    glBindRenderbuffer = (PFNGLBINDRENDERBUFFERPROC)getProcAddress("glBindRenderbuffer");
    glGenRenderbuffers = (PFNGLGENRENDERBUFFERSPROC)getProcAddress("glGenRenderbuffers");
    glDeleteRenderbuffers = (PFNGLDELETERENDERBUFFERSPROC)getProcAddress("glDeleteRenderbuffers");
    glRenderbufferStorage = (PFNGLRENDERBUFFERSTORAGEPROC)getProcAddress("glRenderbufferStorage");
    glClipControl = (PFNGLCLIPCONTROLPROC)getProcAddress("glClipControl");

    return glActiveTexture && glClientActiveTexture && glCompressedTexImage2D &&
           glBindFramebuffer && glGenFramebuffers && glDeleteFramebuffers &&
           glCheckFramebufferStatus && glFramebufferTexture2D && glFramebufferRenderbuffer &&
           glBindRenderbuffer && glGenRenderbuffers && glDeleteRenderbuffers &&
           glRenderbufferStorage && glClipControl;
}

#endif // _WIN32
