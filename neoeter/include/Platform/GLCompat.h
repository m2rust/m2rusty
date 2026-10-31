#pragma once

// GL entry-point compatibility layer (T1.4).
//
// Linux links the real libGL and gets every prototype from <GL/glext.h>.
// Windows only exports GL 1.1 from opengl32.dll, so the newer entry points
// used by the D3D9-compatibility renderer are loaded at runtime through
// Platform_LoadGLFunctions() (wired to SDL_GL_GetProcAddress after the
// context is created).

#if defined(_WIN32)

#include <windows.h>
#include <GL/gl.h>

// Windows ships an OpenGL 1.1 header: the enums introduced by GL 1.2/1.3 and by
// EXT_framebuffer_object are missing even though the driver exposes the entry
// points (loaded below). Values are the canonical ones.
#ifndef GL_TEXTURE1
#define GL_TEXTURE1 0x84C1
#define GL_TEXTURE2 0x84C2
#define GL_TEXTURE3 0x84C3
#define GL_TEXTURE4 0x84C4
#define GL_TEXTURE5 0x84C5
#define GL_TEXTURE6 0x84C6
#define GL_TEXTURE7 0x84C7
#endif
#ifndef GL_TEXTURE_MAX_LEVEL
#define GL_TEXTURE_MAX_LEVEL 0x813D
#endif
#ifndef GL_FRAMEBUFFER
#define GL_FRAMEBUFFER 0x8D40
#endif
#ifndef GL_RENDERBUFFER
#define GL_RENDERBUFFER 0x8D41
#endif
#ifndef GL_COLOR_ATTACHMENT0
#define GL_COLOR_ATTACHMENT0 0x8CE0
#endif
#ifndef GL_DEPTH_ATTACHMENT
#define GL_DEPTH_ATTACHMENT 0x8D00
#endif
#ifndef GL_DEPTH_COMPONENT16
#define GL_DEPTH_COMPONENT16 0x81A5
#endif

typedef void (APIENTRY *PFNGLACTIVETEXTUREPROC)(GLenum texture);
typedef void (APIENTRY *PFNGLCLIENTACTIVETEXTUREPROC)(GLenum texture);
typedef void (APIENTRY *PFNGLCOMPRESSEDTEXIMAGE2DPROC)(GLenum target, GLint level, GLenum internalformat, GLsizei width, GLsizei height, GLint border, GLsizei imageSize, const void* data);
typedef void (APIENTRY *PFNGLBINDFRAMEBUFFERPROC)(GLenum target, GLuint framebuffer);
typedef void (APIENTRY *PFNGLGENFRAMEBUFFERSPROC)(GLsizei n, GLuint* framebuffers);
typedef void (APIENTRY *PFNGLDELETEFRAMEBUFFERSPROC)(GLsizei n, const GLuint* framebuffers);
typedef GLenum (APIENTRY *PFNGLCHECKFRAMEBUFFERSTATUSPROC)(GLenum target);
typedef void (APIENTRY *PFNGLFRAMEBUFFERTEXTURE2DPROC)(GLenum target, GLenum attachment, GLenum textarget, GLuint texture, GLint level);
typedef void (APIENTRY *PFNGLFRAMEBUFFERRENDERBUFFERPROC)(GLenum target, GLenum attachment, GLenum renderbuffertarget, GLuint renderbuffer);
typedef void (APIENTRY *PFNGLBINDRENDERBUFFERPROC)(GLenum target, GLuint renderbuffer);
typedef void (APIENTRY *PFNGLGENRENDERBUFFERSPROC)(GLsizei n, GLuint* renderbuffers);
typedef void (APIENTRY *PFNGLDELETERENDERBUFFERSPROC)(GLsizei n, const GLuint* renderbuffers);
typedef void (APIENTRY *PFNGLRENDERBUFFERSTORAGEPROC)(GLenum target, GLenum internalformat, GLsizei width, GLsizei height);
typedef void (APIENTRY *PFNGLCLIPCONTROLPROC)(GLenum origin, GLenum depth);

extern PFNGLACTIVETEXTUREPROC glActiveTexture;
extern PFNGLCLIENTACTIVETEXTUREPROC glClientActiveTexture;
extern PFNGLCOMPRESSEDTEXIMAGE2DPROC glCompressedTexImage2D;
extern PFNGLBINDFRAMEBUFFERPROC glBindFramebuffer;
extern PFNGLGENFRAMEBUFFERSPROC glGenFramebuffers;
extern PFNGLDELETEFRAMEBUFFERSPROC glDeleteFramebuffers;
extern PFNGLCHECKFRAMEBUFFERSTATUSPROC glCheckFramebufferStatus;
extern PFNGLFRAMEBUFFERTEXTURE2DPROC glFramebufferTexture2D;
extern PFNGLFRAMEBUFFERRENDERBUFFERPROC glFramebufferRenderbuffer;
extern PFNGLBINDRENDERBUFFERPROC glBindRenderbuffer;
extern PFNGLGENRENDERBUFFERSPROC glGenRenderbuffers;
extern PFNGLDELETERENDERBUFFERSPROC glDeleteRenderbuffers;
extern PFNGLRENDERBUFFERSTORAGEPROC glRenderbufferStorage;
extern PFNGLCLIPCONTROLPROC glClipControl;

// Resolves every pointer above. Returns false when one is missing; the caller
// should log the failure and fall back (T1.5 decides what the fallback is).
bool Platform_LoadGLFunctions(void* (*getProcAddress)(const char*));

#elif defined(__APPLE__)

// macOS ships the legacy (compatibility) profile in OpenGL.framework, with
// every prototype declared; it is deprecated but still functional.
#ifndef GL_SILENCE_DEPRECATION
#define GL_SILENCE_DEPRECATION 1
#endif
#ifndef GL_GLEXT_PROTOTYPES
#define GL_GLEXT_PROTOTYPES 1
#endif
#include <OpenGL/gl.h>
#include <OpenGL/glext.h>

inline bool Platform_LoadGLFunctions(void* (*)(const char*)) { return true; }

#else

#ifndef GL_GLEXT_PROTOTYPES
#define GL_GLEXT_PROTOTYPES 1
#endif
#include <GL/gl.h>
#include <GL/glext.h>

inline bool Platform_LoadGLFunctions(void* (*)(const char*)) { return true; }

#endif
