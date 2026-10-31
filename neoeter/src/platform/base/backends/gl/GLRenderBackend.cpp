// OpenGL backend (T1.7).
//
// This is the only file in EterBase that issues OpenGL calls (invariant R1).
// The code below is the pre-seam renderer with the D3D9 façade removed: state
// application follows src/Direct3DDevice9Compat.cpp's previous SetupGLState(),
// vertex-array setup and draw order verbatim so Linux output is unchanged.

#if defined(OPENMT2_RENDERER_GL)

#include "GLRenderBackend.h"

#include <Platform/DirectXMathCompat.h>
#include <Platform/GLCompat.h>

#include <algorithm>
#include <cstring>

#ifndef GL_COMPRESSED_RGB_S3TC_DXT1_EXT
#define GL_COMPRESSED_RGB_S3TC_DXT1_EXT 0x83F0
#endif
#ifndef GL_COMPRESSED_RGBA_S3TC_DXT1_EXT
#define GL_COMPRESSED_RGBA_S3TC_DXT1_EXT 0x83F1
#endif
#ifndef GL_COMPRESSED_RGBA_S3TC_DXT3_EXT
#define GL_COMPRESSED_RGBA_S3TC_DXT3_EXT 0x83F2
#endif
#ifndef GL_COMPRESSED_RGBA_S3TC_DXT5_EXT
#define GL_COMPRESSED_RGBA_S3TC_DXT5_EXT 0x83F3
#endif
#ifndef GL_BGRA
#define GL_BGRA 0x80E1
#endif
#ifndef GL_CLAMP_TO_EDGE
#define GL_CLAMP_TO_EDGE 0x812F
#endif
#ifndef GL_CLAMP_TO_BORDER
#define GL_CLAMP_TO_BORDER 0x812D
#endif
#ifndef GL_TEXTURE_BORDER_COLOR
#define GL_TEXTURE_BORDER_COLOR 0x1004
#endif
#ifndef GL_TEXTURE0
#define GL_TEXTURE0 0x84C0
#endif
#ifndef GL_CONSTANT
#define GL_CONSTANT 0x8576
#endif
#ifndef GL_PRIMARY_COLOR
#define GL_PRIMARY_COLOR 0x8577
#endif
#ifndef GL_PREVIOUS
#define GL_PREVIOUS 0x8578
#endif
#ifndef GL_COMBINE
#define GL_COMBINE 0x8570
#define GL_COMBINE_RGB 0x8571
#define GL_COMBINE_ALPHA 0x8572
#define GL_SRC0_RGB 0x8580
#define GL_SRC1_RGB 0x8581
#define GL_SRC2_RGB 0x8582
#define GL_SRC0_ALPHA 0x8588
#define GL_SRC1_ALPHA 0x8589
#define GL_SRC2_ALPHA 0x858A
#define GL_OPERAND0_RGB 0x8590
#define GL_OPERAND1_RGB 0x8591
#define GL_OPERAND2_RGB 0x8592
#define GL_OPERAND0_ALPHA 0x8598
#define GL_OPERAND1_ALPHA 0x8599
#define GL_OPERAND2_ALPHA 0x859A
#define GL_RGB_SCALE 0x8573
#define GL_ALPHA_SCALE 0x0D1C
#define GL_ADD_SIGNED 0x8574
#define GL_INTERPOLATE 0x8575
#define GL_SUBTRACT 0x84E7
#endif

namespace eterbase {
namespace gl {
namespace {

// Number of stages the GL fixed-function emulation applies. Stages 2-7 are
// carried in the snapshot for a future program-synthesising backend.
constexpr uint32_t kAppliedStages = 2;

inline GLenum ToGLBlend(DWORD d3dBlend)
{
    switch (d3dBlend) {
        case D3DBLEND_ZERO: return GL_ZERO;
        case D3DBLEND_ONE: return GL_ONE;
        case D3DBLEND_SRCCOLOR: return GL_SRC_COLOR;
        case D3DBLEND_INVSRCCOLOR: return GL_ONE_MINUS_SRC_COLOR;
        case D3DBLEND_SRCALPHA: return GL_SRC_ALPHA;
        case D3DBLEND_INVSRCALPHA: return GL_ONE_MINUS_SRC_ALPHA;
        case D3DBLEND_DESTALPHA: return GL_DST_ALPHA;
        case D3DBLEND_INVDESTALPHA: return GL_ONE_MINUS_DST_ALPHA;
        case D3DBLEND_DESTCOLOR: return GL_DST_COLOR;
        case D3DBLEND_INVDESTCOLOR: return GL_ONE_MINUS_DST_COLOR;
        default: return GL_ONE;
    }
}

inline GLenum ToGLCmpFunc(DWORD d3dCmp)
{
    switch (d3dCmp) {
        case D3DCMP_NEVER: return GL_NEVER;
        case D3DCMP_LESS: return GL_LESS;
        case D3DCMP_EQUAL: return GL_EQUAL;
        case D3DCMP_LESSEQUAL: return GL_LEQUAL;
        case D3DCMP_GREATER: return GL_GREATER;
        case D3DCMP_NOTEQUAL: return GL_NOTEQUAL;
        case D3DCMP_GREATEREQUAL: return GL_GEQUAL;
        case D3DCMP_ALWAYS: return GL_ALWAYS;
        default: return GL_LEQUAL;
    }
}

inline GLenum ToGLPrimitive(D3DPRIMITIVETYPE pt)
{
    switch (pt) {
        case D3DPT_POINTLIST: return GL_POINTS;
        case D3DPT_LINELIST: return GL_LINES;
        case D3DPT_LINESTRIP: return GL_LINE_STRIP;
        case D3DPT_TRIANGLELIST: return GL_TRIANGLES;
        case D3DPT_TRIANGLESTRIP: return GL_TRIANGLE_STRIP;
        case D3DPT_TRIANGLEFAN: return GL_TRIANGLE_FAN;
        default: return GL_TRIANGLES;
    }
}

inline GLenum ToGLStageSource(DWORD arg)
{
    switch (arg & D3DTA_SELECTMASK) {
        case D3DTA_TEXTURE:  return GL_TEXTURE;
        case D3DTA_DIFFUSE:  return GL_PRIMARY_COLOR;
        case D3DTA_CURRENT:  return GL_PREVIOUS;
        case D3DTA_TFACTOR:  return GL_CONSTANT;
        case D3DTA_SPECULAR: return GL_PRIMARY_COLOR;
        case D3DTA_TEMP:     return GL_PREVIOUS;
        case D3DTA_CONSTANT: return GL_CONSTANT;
        default:             return GL_TEXTURE;
    }
}

inline GLenum ToGLColorOperand(DWORD arg)
{
    if (arg & D3DTA_ALPHAREPLICATE) return GL_SRC_ALPHA;
    if (arg & D3DTA_COMPLEMENT)     return GL_ONE_MINUS_SRC_COLOR;
    return GL_SRC_COLOR;
}

inline GLenum ToGLAlphaOperand(DWORD arg)
{
    if (arg & D3DTA_COMPLEMENT) return GL_ONE_MINUS_SRC_ALPHA;
    return GL_SRC_ALPHA;
}

void SetStageColorCombine(DWORD op, DWORD arg1, DWORD arg2)
{
    const GLenum s0 = ToGLStageSource(arg1);
    const GLenum s1 = ToGLStageSource(arg2);

    glTexEnvi(GL_TEXTURE_ENV, GL_TEXTURE_ENV_MODE, GL_COMBINE);
    glTexEnvi(GL_TEXTURE_ENV, GL_OPERAND0_RGB, ToGLColorOperand(arg1));
    glTexEnvi(GL_TEXTURE_ENV, GL_OPERAND1_RGB, ToGLColorOperand(arg2));
    glTexEnvi(GL_TEXTURE_ENV, GL_OPERAND2_RGB, GL_SRC_ALPHA);
    glTexEnvi(GL_TEXTURE_ENV, GL_RGB_SCALE, 1);
    switch (op) {
        case D3DTOP_DISABLE:
            // Color passes through from the previous stage.
            glTexEnvi(GL_TEXTURE_ENV, GL_COMBINE_RGB, GL_REPLACE);
            glTexEnvi(GL_TEXTURE_ENV, GL_SRC0_RGB, GL_PREVIOUS);
            break;
        case D3DTOP_SELECTARG1:
            glTexEnvi(GL_TEXTURE_ENV, GL_COMBINE_RGB, GL_REPLACE);
            glTexEnvi(GL_TEXTURE_ENV, GL_SRC0_RGB, s0);
            break;
        case D3DTOP_SELECTARG2:
            glTexEnvi(GL_TEXTURE_ENV, GL_COMBINE_RGB, GL_REPLACE);
            glTexEnvi(GL_TEXTURE_ENV, GL_SRC0_RGB, s1);
            break;
        case D3DTOP_ADD:
            glTexEnvi(GL_TEXTURE_ENV, GL_COMBINE_RGB, GL_ADD);
            glTexEnvi(GL_TEXTURE_ENV, GL_SRC0_RGB, s0);
            glTexEnvi(GL_TEXTURE_ENV, GL_SRC1_RGB, s1);
            break;
        case D3DTOP_ADDSIGNED:
            glTexEnvi(GL_TEXTURE_ENV, GL_COMBINE_RGB, GL_ADD_SIGNED);
            glTexEnvi(GL_TEXTURE_ENV, GL_SRC0_RGB, s0);
            glTexEnvi(GL_TEXTURE_ENV, GL_SRC1_RGB, s1);
            break;
        case D3DTOP_SUBTRACT:
            glTexEnvi(GL_TEXTURE_ENV, GL_COMBINE_RGB, GL_SUBTRACT);
            glTexEnvi(GL_TEXTURE_ENV, GL_SRC0_RGB, s0);
            glTexEnvi(GL_TEXTURE_ENV, GL_SRC1_RGB, s1);
            break;
        case D3DTOP_MODULATE2X:
        case D3DTOP_MODULATE4X:
            glTexEnvi(GL_TEXTURE_ENV, GL_COMBINE_RGB, GL_MODULATE);
            glTexEnvi(GL_TEXTURE_ENV, GL_SRC0_RGB, s0);
            glTexEnvi(GL_TEXTURE_ENV, GL_SRC1_RGB, s1);
            glTexEnvi(GL_TEXTURE_ENV, GL_RGB_SCALE, (op == D3DTOP_MODULATE2X) ? 2 : 4);
            break;
        case D3DTOP_BLENDTEXTUREALPHA:
            glTexEnvi(GL_TEXTURE_ENV, GL_COMBINE_RGB, GL_INTERPOLATE);
            glTexEnvi(GL_TEXTURE_ENV, GL_SRC0_RGB, s0);
            glTexEnvi(GL_TEXTURE_ENV, GL_SRC1_RGB, s1);
            glTexEnvi(GL_TEXTURE_ENV, GL_SRC2_RGB, GL_TEXTURE);
            break;
        case D3DTOP_BLENDCURRENTALPHA:
            glTexEnvi(GL_TEXTURE_ENV, GL_COMBINE_RGB, GL_INTERPOLATE);
            glTexEnvi(GL_TEXTURE_ENV, GL_SRC0_RGB, s0);
            glTexEnvi(GL_TEXTURE_ENV, GL_SRC1_RGB, s1);
            glTexEnvi(GL_TEXTURE_ENV, GL_SRC2_RGB, GL_PREVIOUS);
            break;
        case D3DTOP_BLENDFACTORALPHA:
            glTexEnvi(GL_TEXTURE_ENV, GL_COMBINE_RGB, GL_INTERPOLATE);
            glTexEnvi(GL_TEXTURE_ENV, GL_SRC0_RGB, s0);
            glTexEnvi(GL_TEXTURE_ENV, GL_SRC1_RGB, s1);
            glTexEnvi(GL_TEXTURE_ENV, GL_SRC2_RGB, GL_CONSTANT);
            break;
        case D3DTOP_MODULATEALPHA_ADDCOLOR:
        case D3DTOP_MODULATECOLOR_ADDALPHA:
        case D3DTOP_MODULATEINVALPHA_ADDCOLOR:
        case D3DTOP_MODULATEINVCOLOR_ADDALPHA:
            // Approximation of the specular "add" stages: current + texture.
            glTexEnvi(GL_TEXTURE_ENV, GL_COMBINE_RGB, GL_ADD);
            glTexEnvi(GL_TEXTURE_ENV, GL_SRC0_RGB, GL_PREVIOUS);
            glTexEnvi(GL_TEXTURE_ENV, GL_SRC1_RGB, s1);
            break;
        default:
            glTexEnvi(GL_TEXTURE_ENV, GL_COMBINE_RGB, GL_MODULATE);
            glTexEnvi(GL_TEXTURE_ENV, GL_SRC0_RGB, s0);
            glTexEnvi(GL_TEXTURE_ENV, GL_SRC1_RGB, s1);
            break;
    }
}

void SetStageAlphaCombine(DWORD op, DWORD arg1, DWORD arg2)
{
    const GLenum s0 = ToGLStageSource(arg1);
    const GLenum s1 = ToGLStageSource(arg2);

    glTexEnvi(GL_TEXTURE_ENV, GL_TEXTURE_ENV_MODE, GL_COMBINE);
    glTexEnvi(GL_TEXTURE_ENV, GL_OPERAND0_ALPHA, ToGLAlphaOperand(arg1));
    glTexEnvi(GL_TEXTURE_ENV, GL_OPERAND1_ALPHA, ToGLAlphaOperand(arg2));
    glTexEnvi(GL_TEXTURE_ENV, GL_ALPHA_SCALE, 1);

    switch (op) {
        case D3DTOP_DISABLE:
            // Alpha is passed through unchanged.
            glTexEnvi(GL_TEXTURE_ENV, GL_COMBINE_ALPHA, GL_REPLACE);
            glTexEnvi(GL_TEXTURE_ENV, GL_SRC0_ALPHA, GL_PREVIOUS);
            break;
        case D3DTOP_SELECTARG1:
            glTexEnvi(GL_TEXTURE_ENV, GL_COMBINE_ALPHA, GL_REPLACE);
            glTexEnvi(GL_TEXTURE_ENV, GL_SRC0_ALPHA, s0);
            break;
        case D3DTOP_SELECTARG2:
            glTexEnvi(GL_TEXTURE_ENV, GL_COMBINE_ALPHA, GL_REPLACE);
            glTexEnvi(GL_TEXTURE_ENV, GL_SRC0_ALPHA, s1);
            break;
        case D3DTOP_ADD:
        case D3DTOP_ADDSIGNED:
            glTexEnvi(GL_TEXTURE_ENV, GL_COMBINE_ALPHA, GL_ADD);
            glTexEnvi(GL_TEXTURE_ENV, GL_SRC0_ALPHA, s0);
            glTexEnvi(GL_TEXTURE_ENV, GL_SRC1_ALPHA, s1);
            break;
        case D3DTOP_SUBTRACT:
            glTexEnvi(GL_TEXTURE_ENV, GL_COMBINE_ALPHA, GL_SUBTRACT);
            glTexEnvi(GL_TEXTURE_ENV, GL_SRC0_ALPHA, s0);
            glTexEnvi(GL_TEXTURE_ENV, GL_SRC1_ALPHA, s1);
            break;
        default:
            glTexEnvi(GL_TEXTURE_ENV, GL_COMBINE_ALPHA, GL_MODULATE);
            glTexEnvi(GL_TEXTURE_ENV, GL_SRC0_ALPHA, s0);
            glTexEnvi(GL_TEXTURE_ENV, GL_SRC1_ALPHA, s1);
            break;
    }
}

// Emulates D3DTSS_TCI_CAMERASPACEPOSITION with GL_EYE_LINEAR.
// The D3D texture matrix is baked into the eye plane equations so that the
// generated (s,t) coordinates are identical to the D3D ones, including the
// eye-space Z and translation terms (which are essential for stable
// world-space texture coordinates on the terrain).
void ApplyCameraSpacePositionTexGen(const D3DXMATRIX* pTexMatrix, bool bProjected)
{
    GLfloat s[4], t[4], r[4], q[4];
    if (pTexMatrix) {
        s[0] = pTexMatrix->_11; s[1] = pTexMatrix->_21; s[2] = pTexMatrix->_31; s[3] = pTexMatrix->_41;
        t[0] = pTexMatrix->_12; t[1] = pTexMatrix->_22; t[2] = pTexMatrix->_32; t[3] = pTexMatrix->_42;
        r[0] = pTexMatrix->_13; r[1] = pTexMatrix->_23; r[2] = pTexMatrix->_33; r[3] = pTexMatrix->_43;
        q[0] = pTexMatrix->_14; q[1] = pTexMatrix->_24; q[2] = pTexMatrix->_34; q[3] = pTexMatrix->_44;
    } else {
        s[0] = 1.0f; s[1] = 0.0f; s[2] = 0.0f; s[3] = 0.0f;
        t[0] = 0.0f; t[1] = 1.0f; t[2] = 0.0f; t[3] = 0.0f;
        r[0] = 0.0f; r[1] = 0.0f; r[2] = 1.0f; r[3] = 0.0f;
        q[0] = 0.0f; q[1] = 0.0f; q[2] = 0.0f; q[3] = 1.0f;
    }

    // GL transforms the supplied eye plane by the inverse of the modelview
    // matrix. Specify the planes with an identity modelview so they are stored
    // verbatim in eye space.
    glMatrixMode(GL_MODELVIEW);
    glPushMatrix();
    glLoadIdentity();

    glTexGeni(GL_S, GL_TEXTURE_GEN_MODE, GL_EYE_LINEAR);
    glTexGeni(GL_T, GL_TEXTURE_GEN_MODE, GL_EYE_LINEAR);
    glTexGenfv(GL_S, GL_EYE_PLANE, s);
    glTexGenfv(GL_T, GL_EYE_PLANE, t);

    if (bProjected) {
        glTexGeni(GL_R, GL_TEXTURE_GEN_MODE, GL_EYE_LINEAR);
        glTexGeni(GL_Q, GL_TEXTURE_GEN_MODE, GL_EYE_LINEAR);
        glTexGenfv(GL_R, GL_EYE_PLANE, r);
        glTexGenfv(GL_Q, GL_EYE_PLANE, q);
        glEnable(GL_TEXTURE_GEN_R);
        glEnable(GL_TEXTURE_GEN_Q);
    } else {
        glDisable(GL_TEXTURE_GEN_R);
        glDisable(GL_TEXTURE_GEN_Q);
    }

    glPopMatrix();

    glEnable(GL_TEXTURE_GEN_S);
    glEnable(GL_TEXTURE_GEN_T);
}

void SetupVertexArrays(const BYTE* vbData, UINT stride, DWORD fvf, const GLfloat* primaryColor)
{
    DWORD posType = fvf & D3DFVF_POSITION_MASK;
    size_t offset = 0;

    glEnableClientState(GL_VERTEX_ARRAY);
    glVertexPointer(3, GL_FLOAT, stride, vbData + offset);
    if (posType == D3DFVF_XYZRHW) {
        offset += 16;
    } else {
        offset += 12;
    }

    if (fvf & D3DFVF_NORMAL) {
        glEnableClientState(GL_NORMAL_ARRAY);
        glNormalPointer(GL_FLOAT, stride, vbData + offset);
        offset += 12;
    } else {
        glDisableClientState(GL_NORMAL_ARRAY);
    }

    if (fvf & D3DFVF_PSIZE) {
        offset += 4;
    }

    if (fvf & D3DFVF_DIFFUSE) {
        glEnableClientState(GL_COLOR_ARRAY);
        glColorPointer(GL_BGRA, GL_UNSIGNED_BYTE, stride, vbData + offset);
        offset += 4;
    } else {
        glDisableClientState(GL_COLOR_ARRAY);
        glColor4fv(primaryColor);
    }

    if (fvf & D3DFVF_SPECULAR) {
        offset += 4;
    }

    UINT numTex = (fvf & D3DFVF_TEXCOUNT_MASK) >> D3DFVF_TEXCOUNT_SHIFT;
    if (numTex >= 1) {
        glClientActiveTexture(GL_TEXTURE0);
        glEnableClientState(GL_TEXTURE_COORD_ARRAY);
        glTexCoordPointer(2, GL_FLOAT, stride, vbData + offset);
        offset += 8;
    } else {
        glClientActiveTexture(GL_TEXTURE0);
        glDisableClientState(GL_TEXTURE_COORD_ARRAY);
    }

    if (numTex >= 2) {
        glClientActiveTexture(GL_TEXTURE1);
        glEnableClientState(GL_TEXTURE_COORD_ARRAY);
        glTexCoordPointer(2, GL_FLOAT, stride, vbData + offset);
        offset += 8;
    } else {
        glClientActiveTexture(GL_TEXTURE1);
        glDisableClientState(GL_TEXTURE_COORD_ARRAY);
    }
    glClientActiveTexture(GL_TEXTURE0);
}

void TeardownVertexArrays(DWORD fvf)
{
    glDisableClientState(GL_VERTEX_ARRAY);
    if (fvf & D3DFVF_NORMAL) glDisableClientState(GL_NORMAL_ARRAY);
    if (fvf & D3DFVF_DIFFUSE) glDisableClientState(GL_COLOR_ARRAY);
    UINT numTex = (fvf & D3DFVF_TEXCOUNT_MASK) >> D3DFVF_TEXCOUNT_SHIFT;
    if (numTex >= 1) {
        glClientActiveTexture(GL_TEXTURE0);
        glDisableClientState(GL_TEXTURE_COORD_ARRAY);
    }
    if (numTex >= 2) {
        glClientActiveTexture(GL_TEXTURE1);
        glDisableClientState(GL_TEXTURE_COORD_ARRAY);
    }
    glClientActiveTexture(GL_TEXTURE0);
    glDisable(GL_TEXTURE_GEN_S);
    glDisable(GL_TEXTURE_GEN_T);
}

} // namespace

GLRenderBackend::~GLRenderBackend()
{
    for (auto& slot : m_resources) {
        if (!slot.used || !slot.glName) continue;
        if (slot.kind == ResourceKind::Texture) {
            GLuint name = slot.glName;
            glDeleteTextures(1, &name);
        } else {
            GLuint name = slot.glName;
            glDeleteRenderbuffers(1, &name);
        }
    }
    if (m_framebuffer) {
        GLuint fbo = m_framebuffer;
        glDeleteFramebuffers(1, &fbo);
        m_framebuffer = 0;
    }
}

RenderCaps GLRenderBackend::Caps() const
{
    EnsureCaps();
    return m_caps;
}

void GLRenderBackend::EnsureCaps() const
{
    if (m_capsLoaded) return;
    m_capsLoaded = true;

    GLint maxTexture = 0;
    glGetIntegerv(GL_MAX_TEXTURE_SIZE, &maxTexture);
    if (maxTexture < 0) maxTexture = 0;

    const GLubyte* extensions = glGetString(GL_EXTENSIONS);
    const char* ext = extensions ? reinterpret_cast<const char*>(extensions) : "";
    const bool s3tc = std::strstr(ext, "GL_EXT_texture_compression_s3tc") != nullptr;

    m_caps.maxTextureSize = static_cast<uint32_t>(maxTexture);
    m_caps.dxt1 = s3tc;
    m_caps.dxt5 = s3tc;
    m_caps.twoSidedStencil = true;
    m_caps.separateAlphaBlend = true;
    // The GL backend is the fixed-function reference implementation.
    m_caps.fixedFunction = true;
}

GLRenderBackend::ResourceSlot* GLRenderBackend::Lookup(RenderHandle handle, ResourceKind kind)
{
    if (!handle.IsValid() || handle.id > m_resources.size()) return nullptr;
    ResourceSlot& slot = m_resources[handle.id - 1];
    if (!slot.used || slot.kind != kind || slot.generation != handle.generation) return nullptr;
    return &slot;
}

RenderHandle GLRenderBackend::CreateTexture()
{
    uint32_t index = 0;
    while (index < m_resources.size() && m_resources[index].used) ++index;
    if (index == m_resources.size()) m_resources.emplace_back();
    ResourceSlot& slot = m_resources[index];
    slot.used = true;
    slot.kind = ResourceKind::Texture;
    slot.glName = 0;
    ++slot.generation;
    return RenderHandle{ index + 1, slot.generation };
}

RenderHandle GLRenderBackend::CreateDepthStencil(uint32_t width, uint32_t height)
{
    uint32_t index = 0;
    while (index < m_resources.size() && m_resources[index].used) ++index;
    if (index == m_resources.size()) m_resources.emplace_back();
    ResourceSlot& slot = m_resources[index];
    slot.used = true;
    slot.kind = ResourceKind::DepthStencil;
    slot.glName = 0;
    ++slot.generation;

    GLuint name = 0;
    glGenRenderbuffers(1, &name);
    glBindRenderbuffer(GL_RENDERBUFFER, name);
    glRenderbufferStorage(GL_RENDERBUFFER, GL_DEPTH_COMPONENT16, width, height);
    glBindRenderbuffer(GL_RENDERBUFFER, 0);
    slot.glName = name;

    return RenderHandle{ index + 1, slot.generation };
}

void GLRenderBackend::Destroy(RenderHandle handle)
{
    if (!handle.IsValid() || handle.id > m_resources.size()) return;
    ResourceSlot& slot = m_resources[handle.id - 1];
    if (!slot.used || slot.generation != handle.generation) return;

    if (slot.glName) {
        if (slot.kind == ResourceKind::Texture) {
            GLuint name = slot.glName;
            glDeleteTextures(1, &name);
        } else {
            GLuint name = slot.glName;
            glDeleteRenderbuffers(1, &name);
        }
    }
    slot.used = false;
    slot.glName = 0;
    ++slot.generation;
}

void GLRenderBackend::UploadTextureLevel(RenderHandle texture, uint32_t level, uint32_t format,
                                         uint32_t width, uint32_t height, const void* data,
                                         size_t dataSize)
{
    ResourceSlot* slot = Lookup(texture, ResourceKind::Texture);
    if (!slot || !data || dataSize == 0) return;

    if (!slot->glName) {
        GLuint name = 0;
        glGenTextures(1, &name);
        if (!name) return;
        slot->glName = name;
    }
    glBindTexture(GL_TEXTURE_2D, slot->glName);

    const GLsizei mipW = static_cast<GLsizei>(std::max(1u, width));
    const GLsizei mipH = static_cast<GLsizei>(std::max(1u, height));
    const D3DFORMAT fmt = static_cast<D3DFORMAT>(format);

    if (fmt == D3DFMT_DXT1) {
        GLsizei size = (GLsizei)(std::max(1, (mipW + 3) / 4) * std::max(1, (mipH + 3) / 4) * 8);
        if ((size_t)size > dataSize) size = (GLsizei)dataSize;
        glCompressedTexImage2D(GL_TEXTURE_2D, level, GL_COMPRESSED_RGBA_S3TC_DXT1_EXT, mipW, mipH, 0, size, data);
    } else if (fmt == D3DFMT_DXT3) {
        GLsizei size = (GLsizei)(std::max(1, (mipW + 3) / 4) * std::max(1, (mipH + 3) / 4) * 16);
        if ((size_t)size > dataSize) size = (GLsizei)dataSize;
        glCompressedTexImage2D(GL_TEXTURE_2D, level, GL_COMPRESSED_RGBA_S3TC_DXT3_EXT, mipW, mipH, 0, size, data);
    } else if (fmt == D3DFMT_DXT5) {
        GLsizei size = (GLsizei)(std::max(1, (mipW + 3) / 4) * std::max(1, (mipH + 3) / 4) * 16);
        if ((size_t)size > dataSize) size = (GLsizei)dataSize;
        glCompressedTexImage2D(GL_TEXTURE_2D, level, GL_COMPRESSED_RGBA_S3TC_DXT5_EXT, mipW, mipH, 0, size, data);
    } else if (fmt == D3DFMT_A4R4G4B4) {
        std::vector<uint32_t> rgba((size_t)mipW * mipH);
        const uint16_t* src = (const uint16_t*)data;
        for (size_t i = 0; i < rgba.size(); ++i) {
            uint16_t val = src[i];
            uint8_t a = (val >> 12) & 0xF; a = (a << 4) | a;
            uint8_t r = (val >> 8) & 0xF;  r = (r << 4) | r;
            uint8_t g = (val >> 4) & 0xF;  g = (g << 4) | g;
            uint8_t b = val & 0xF;         b = (b << 4) | b;
            rgba[i] = (a << 24) | (b << 16) | (g << 8) | r;
        }
        glTexImage2D(GL_TEXTURE_2D, level, GL_RGBA, mipW, mipH, 0, GL_RGBA, GL_UNSIGNED_BYTE, rgba.data());
    } else if (fmt == D3DFMT_R5G6B5) {
        // 16 bit 5-6-5: must be expanded explicitly. Passing the 2-byte source
        // to a GL_BGRA 32-bit upload reads out of bounds and produces garbage
        // (this is the character shadow map format).
        std::vector<uint32_t> rgba((size_t)mipW * mipH);
        const uint16_t* src = (const uint16_t*)data;
        for (size_t i = 0; i < rgba.size(); ++i) {
            uint16_t val = src[i];
            uint8_t r = (val >> 11) & 0x1F; r = (r << 3) | (r >> 2);
            uint8_t g = (val >> 5) & 0x3F;  g = (g << 2) | (g >> 4);
            uint8_t b = val & 0x1F;         b = (b << 3) | (b >> 2);
            rgba[i] = (0xFFu << 24) | ((uint32_t)b << 16) | ((uint32_t)g << 8) | r;
        }
        glTexImage2D(GL_TEXTURE_2D, level, GL_RGBA, mipW, mipH, 0, GL_RGBA, GL_UNSIGNED_BYTE, rgba.data());
    } else if (fmt == D3DFMT_A1R5G5B5) {
        std::vector<uint32_t> rgba((size_t)mipW * mipH);
        const uint16_t* src = (const uint16_t*)data;
        for (size_t i = 0; i < rgba.size(); ++i) {
            uint16_t val = src[i];
            uint8_t a = (val & 0x8000) ? 0xFF : 0x00;
            uint8_t r = (val >> 10) & 0x1F; r = (r << 3) | (r >> 2);
            uint8_t g = (val >> 5) & 0x1F;  g = (g << 3) | (g >> 2);
            uint8_t b = val & 0x1F;         b = (b << 3) | (b >> 2);
            rgba[i] = ((uint32_t)a << 24) | ((uint32_t)b << 16) | ((uint32_t)g << 8) | r;
        }
        glTexImage2D(GL_TEXTURE_2D, level, GL_RGBA, mipW, mipH, 0, GL_RGBA, GL_UNSIGNED_BYTE, rgba.data());
    } else {
        glTexImage2D(GL_TEXTURE_2D, level, GL_RGBA, mipW, mipH, 0, GL_BGRA, GL_UNSIGNED_BYTE, data);
    }
}

void GLRenderBackend::FinalizeTexture(RenderHandle texture, uint32_t maxLevel)
{
    ResourceSlot* slot = Lookup(texture, ResourceKind::Texture);
    if (!slot || !slot->glName) return;

    glBindTexture(GL_TEXTURE_2D, slot->glName);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAX_LEVEL, (GLint)maxLevel);
    if (maxLevel > 0) {
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR_MIPMAP_LINEAR);
    } else {
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
    }
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_REPEAT);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_REPEAT);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
}

void GLRenderBackend::EnsureFramebuffer()
{
    if (!m_framebuffer)
        glGenFramebuffers(1, &m_framebuffer);
}

void GLRenderBackend::BindDefaultRenderTarget()
{
    glBindFramebuffer(GL_FRAMEBUFFER, 0);
}

void GLRenderBackend::BindRenderTarget(RenderHandle colorTexture)
{
    ResourceSlot* slot = Lookup(colorTexture, ResourceKind::Texture);
    if (!slot) {
        BindDefaultRenderTarget();
        return;
    }

    EnsureFramebuffer();
    glBindFramebuffer(GL_FRAMEBUFFER, m_framebuffer);
    glFramebufferTexture2D(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, slot->glName, 0);
}

void GLRenderBackend::BindDepthStencil(RenderHandle depthStencil)
{
    ResourceSlot* slot = Lookup(depthStencil, ResourceKind::DepthStencil);
    if (!slot) return;

    EnsureFramebuffer();
    glBindFramebuffer(GL_FRAMEBUFFER, m_framebuffer);
    glFramebufferRenderbuffer(GL_FRAMEBUFFER, GL_DEPTH_ATTACHMENT, GL_RENDERBUFFER, slot->glName);
}

void GLRenderBackend::SetViewport(int32_t x, int32_t y, uint32_t width, uint32_t height)
{
    glViewport(x, y, (GLsizei)width, (GLsizei)height);
}

void GLRenderBackend::Clear(uint32_t flags, uint32_t color, float z, uint32_t stencil)
{
    GLbitfield mask = 0;
    if (flags & D3DCLEAR_TARGET) {
        float a = ((color >> 24) & 0xFF) / 255.0f;
        float r = ((color >> 16) & 0xFF) / 255.0f;
        float g = ((color >> 8) & 0xFF) / 255.0f;
        float b = (color & 0xFF) / 255.0f;
        glClearColor(r, g, b, a);
        mask |= GL_COLOR_BUFFER_BIT;
    }
    if (flags & D3DCLEAR_ZBUFFER) {
        glDepthMask(GL_TRUE);
        glClearDepth(z);
        mask |= GL_DEPTH_BUFFER_BIT;
    }
    if (flags & D3DCLEAR_STENCIL) {
        glClearStencil((GLint)stencil);
        mask |= GL_STENCIL_BUFFER_BIT;
    }
    if (mask) {
        glClear(mask);
    }
}

void GLRenderBackend::BeginFrame()
{
    // Frame boundaries stay with the window layer until the swapchain
    // abstraction lands (plan §4.4 item 4); the seam still exposes the hook
    // so the façade's BeginScene/EndScene have a single owner.
}

void GLRenderBackend::Present()
{
    // Presentation is SDL_GL_SwapWindow in EterLib (GrpScreen.cpp); the GL
    // backend does not own the swapchain yet.
}

void GLRenderBackend::BindStageTexture(uint32_t stage, RenderHandle texture)
{
    if (stage >= kRenderMaxTextureStages) return;

    ResourceSlot* slot = Lookup(texture, ResourceKind::Texture);
    glActiveTexture(GL_TEXTURE0 + stage);
    if (slot && slot->glName) {
        glEnable(GL_TEXTURE_2D);
        glBindTexture(GL_TEXTURE_2D, slot->glName);
    } else {
        glBindTexture(GL_TEXTURE_2D, 0);
        glDisable(GL_TEXTURE_2D);
    }
}

void GLRenderBackend::ApplyState(const RenderState& state)
{
    // 1. Matrix Setup
    // When rendering into an FBO we flip the clip-space Y axis. Direct3D render
    // targets store row 0 at the top, while OpenGL stores the bottom row first;
    // flipping keeps the resulting texture sampleable with the client's D3D
    // texture matrices (character shadow map, snow blur, ...).
    const bool flipY = state.inRenderTarget;
    // Negate the projection's Y output row (S * M, including translation) so
    // that D3D's top row lands at texture row 0.
    auto applyClipYFlip = [flipY]() {
        if (!flipY) return;
        GLfloat proj[16];
        glGetFloatv(GL_PROJECTION_MATRIX, proj);
        proj[1] = -proj[1];
        proj[5] = -proj[5];
        proj[9] = -proj[9];
        proj[13] = -proj[13];
        glLoadMatrixf(proj);
    };
    DWORD posType = state.fvf & D3DFVF_POSITION_MASK;
    bool isPerspective = false;
    if (posType == D3DFVF_XYZRHW) {
        // Pre-transformed 2D screen vertices
        glMatrixMode(GL_PROJECTION);
        glLoadIdentity();
        glOrtho(0.0, (double)state.viewportWidth, (double)state.viewportHeight, 0.0, -1000.0, 1000.0);
        applyClipYFlip();
        glMatrixMode(GL_MODELVIEW);
        glLoadIdentity();
    } else {
        // Direct3D 3D perspective projections have _44 == 0.0f (perspective divide).
        // The client's 2D interface projection (SetOrtho2D) is an off-centre
        // orthographic matrix with _41/_42 == -1/+1; it needs the half-pixel
        // screen-space replacement. Centred orthographic projections (shadow
        // map, SetOrtho3D) must use the matrix supplied by the client.
        const float* proj = state.projection;
        isPerspective = (proj[15] == 0.0f);
        const bool isScreenOrtho = !isPerspective && (proj[12] != 0.0f || proj[13] != 0.0f);
        glMatrixMode(GL_PROJECTION);
        if (isPerspective || !isScreenOrtho) {
            glLoadMatrixf(proj);
        } else {
            // 2D Orthographic Pipeline (UI with half-pixel texel offset)
            glLoadIdentity();
            glOrtho(-0.5, (double)state.viewportWidth - 0.5, (double)state.viewportHeight - 0.5, -0.5, -1000.0, 1000.0);
        }
        applyClipYFlip();
        glMatrixMode(GL_MODELVIEW);
        glLoadMatrixf(state.view);
        glMultMatrixf(state.world);
    }

    // 2. Depth Test & Write
    if (state.zEnable) {
        glEnable(GL_DEPTH_TEST);
        glDepthFunc(ToGLCmpFunc(state.zFunc));
    } else {
        glDisable(GL_DEPTH_TEST);
    }
    glDepthMask(state.zWriteEnable ? GL_TRUE : GL_FALSE);

    // 3. Alpha Blending
    if (state.alphaBlendEnable) {
        glEnable(GL_BLEND);
        glBlendFunc(ToGLBlend(state.srcBlend), ToGLBlend(state.destBlend));
    } else {
        glDisable(GL_BLEND);
    }

    // 4. Alpha Testing
    if (state.alphaTestEnable) {
        glEnable(GL_ALPHA_TEST);
        glAlphaFunc(ToGLCmpFunc(state.alphaFunc), (GLclampf)(state.alphaRef / 255.0f));
    } else {
        glDisable(GL_ALPHA_TEST);
    }

    // 5. Fog (D3D9 vertex/table fog emulation)
    // The client enables fog per environment (CMapManager::BeginEnvironment) and
    // temporarily switches the fog colour to white while drawing multiplicative
    // shadow passes. GL's fixed-function fog uses the eye-space depth as the fog
    // coordinate, which matches D3D's non-range vertex fog closely enough.
    const DWORD fogMode = (state.fogTableMode != D3DFOG_NONE) ? state.fogTableMode : state.fogVertexMode;
    if (state.fogEnable && isPerspective && fogMode != D3DFOG_NONE &&
        (fogMode != D3DFOG_LINEAR || state.fogEnd > state.fogStart)) {
        glEnable(GL_FOG);
        const GLfloat fogColor[4] = {
            ((state.fogColor >> 16) & 0xFF) / 255.0f,
            ((state.fogColor >> 8) & 0xFF) / 255.0f,
            (state.fogColor & 0xFF) / 255.0f,
            ((state.fogColor >> 24) & 0xFF) / 255.0f
        };
        glFogfv(GL_FOG_COLOR, fogColor);
        switch (fogMode) {
            case D3DFOG_LINEAR:
                glFogi(GL_FOG_MODE, GL_LINEAR);
                glFogf(GL_FOG_START, state.fogStart);
                glFogf(GL_FOG_END, state.fogEnd);
                break;
            case D3DFOG_EXP:
                glFogi(GL_FOG_MODE, GL_EXP);
                glFogf(GL_FOG_DENSITY, state.fogDensity);
                break;
            case D3DFOG_EXP2:
                glFogi(GL_FOG_MODE, GL_EXP2);
                glFogf(GL_FOG_DENSITY, state.fogDensity);
                break;
            default:
                break;
        }
        glHint(GL_FOG_HINT, GL_NICEST);
    } else {
        glDisable(GL_FOG);
    }

    // 6. Culling
    if (state.cullMode == D3DCULL_NONE) {
        glDisable(GL_CULL_FACE);
    } else {
        glEnable(GL_CULL_FACE);
        // D3D's front face is clockwise in screen space. The clip-space Y flip
        // used for render targets reverses the window-space winding.
        glFrontFace(flipY ? GL_CCW : GL_CW);
        glCullFace((state.cullMode == D3DCULL_CW) ? GL_FRONT : GL_BACK);
    }

    // 7. Fill Mode
    if (state.fillMode == D3DFILL_WIREFRAME) {
        glPolygonMode(GL_FRONT_AND_BACK, GL_LINE);
    } else if (state.fillMode == D3DFILL_POINT) {
        glPolygonMode(GL_FRONT_AND_BACK, GL_POINT);
    } else {
        glPolygonMode(GL_FRONT_AND_BACK, GL_FILL);
    }

    // 8. Textures & Transforms
    const UINT numTex = (state.fvf & D3DFVF_TEXCOUNT_MASK) >> D3DFVF_TEXCOUNT_SHIFT;
    const GLfloat texFactor[4] = {
        ((state.textureFactor >> 16) & 0xFF) / 255.0f,
        ((state.textureFactor >> 8) & 0xFF) / 255.0f,
        (state.textureFactor & 0xFF) / 255.0f,
        ((state.textureFactor >> 24) & 0xFF) / 255.0f
    };

    for (uint32_t stage = 0; stage < kAppliedStages; ++stage) {
        glActiveTexture(GL_TEXTURE0 + stage);
        glDisable(GL_TEXTURE_GEN_R);
        glDisable(GL_TEXTURE_GEN_Q);

        const RenderState::TextureStage& ss = state.stages[stage];
        const RenderState::SamplerStage& sampler = state.samplers[stage];

        const ResourceSlot* texture = Lookup(state.textures[stage], ResourceKind::Texture);

        // If this texture stage is disabled or no texture is bound, disable it
        if ((ss.colorOp == D3DTOP_DISABLE && ss.alphaOp == D3DTOP_DISABLE) ||
            !texture || !texture->glName) {
            glBindTexture(GL_TEXTURE_2D, 0);
            glDisable(GL_TEXTURE_2D);
            glDisable(GL_TEXTURE_GEN_S);
            glDisable(GL_TEXTURE_GEN_T);
            glMatrixMode(GL_TEXTURE);
            glLoadIdentity();
            glMatrixMode(GL_MODELVIEW);
            continue;
        }

        glEnable(GL_TEXTURE_2D);
        glBindTexture(GL_TEXTURE_2D, texture->glName);

        // Texture factor (D3DTA_TFACTOR maps to GL_CONSTANT)
        glTexEnvfv(GL_TEXTURE_ENV, GL_TEXTURE_ENV_COLOR, texFactor);

        // Fixed-function combine emulation (color + alpha)
        SetStageColorCombine(ss.colorOp, ss.colorArg1, ss.colorArg2);
        SetStageAlphaCombine(ss.alphaOp, ss.alphaArg1, ss.alphaArg2);

        // Apply texture sampler states
        const bool hasMips = state.textureHasMips[stage];
        if (sampler.addressU == D3DTADDRESS_CLAMP) {
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
        } else if (sampler.addressU == D3DTADDRESS_BORDER) {
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_BORDER);
        } else {
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_REPEAT);
        }
        if (sampler.addressV == D3DTADDRESS_CLAMP) {
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
        } else if (sampler.addressV == D3DTADDRESS_BORDER) {
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_BORDER);
        } else {
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_REPEAT);
        }
        if (sampler.addressU == D3DTADDRESS_BORDER ||
            sampler.addressV == D3DTADDRESS_BORDER) {
            const DWORD c = sampler.borderColor;
            const GLfloat border[4] = {
                ((c >> 16) & 0xFF) / 255.0f,
                ((c >> 8) & 0xFF) / 255.0f,
                (c & 0xFF) / 255.0f,
                ((c >> 24) & 0xFF) / 255.0f
            };
            glTexParameterfv(GL_TEXTURE_2D, GL_TEXTURE_BORDER_COLOR, border);
        }

        GLint mag = (sampler.magFilter == D3DTEXF_POINT) ? GL_NEAREST : GL_LINEAR;
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, mag);

        if (hasMips) {
            GLint min = (sampler.minFilter == D3DTEXF_POINT) ? GL_NEAREST_MIPMAP_NEAREST : GL_LINEAR_MIPMAP_LINEAR;
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, min);
        } else {
            GLint min = (sampler.minFilter == D3DTEXF_POINT) ? GL_NEAREST : GL_LINEAR;
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, min);
        }

        // Texture coordinate generation / texture matrix.
        // In Direct3D, a cameraspace-position TCI produces the full camera-space
        // position (x, y, z, 1), which is then multiplied by the texture matrix.
        // Baking that matrix into GL_EYE_LINEAR planes reproduces it exactly.
        const DWORD tci = ss.texCoordIndex;
        const DWORD tciMode = tci & 0xFFFF0000u;
        const bool hasTransform = (ss.texTransformFlags & ~D3DTTFF_PROJECTED) != D3DTTFF_DISABLE;
        const bool isProjected = (ss.texTransformFlags & D3DTTFF_PROJECTED) != 0;
        const D3DXMATRIX* pTexMatrix = hasTransform
            ? reinterpret_cast<const D3DXMATRIX*>(state.textureMatrices[stage])
            : nullptr;

        if (tciMode == D3DTSS_TCI_CAMERASPACEPOSITION) {
            ApplyCameraSpacePositionTexGen(pTexMatrix, isProjected);
            glMatrixMode(GL_TEXTURE);
            glLoadIdentity();
            glMatrixMode(GL_MODELVIEW);
        } else if (tciMode == D3DTSS_TCI_PASSTHRU && stage >= (int)numTex) {
            // No per-vertex texture coordinates for this stage: preserve the
            // legacy eye-linear behaviour applied through the texture matrix.
            ApplyCameraSpacePositionTexGen(nullptr, false);
            glMatrixMode(GL_TEXTURE);
            if (pTexMatrix) glLoadMatrixf((const GLfloat*)pTexMatrix);
            else glLoadIdentity();
            glMatrixMode(GL_MODELVIEW);
        } else {
            glDisable(GL_TEXTURE_GEN_S);
            glDisable(GL_TEXTURE_GEN_T);
            glMatrixMode(GL_TEXTURE);
            if (pTexMatrix) glLoadMatrixf((const GLfloat*)pTexMatrix);
            else glLoadIdentity();
            glMatrixMode(GL_MODELVIEW);
        }
    }
    glActiveTexture(GL_TEXTURE0);
}

void GLRenderBackend::Draw(const DrawCall& call)
{
    ApplyState(call.state);

    const BYTE* vbData = static_cast<const BYTE*>(call.vertexData);
    SetupVertexArrays(vbData, call.stride, call.fvf, call.primaryColor);

    const GLenum mode = ToGLPrimitive((D3DPRIMITIVETYPE)call.primitiveType);
    if (call.indexData) {
        const GLenum indexType = call.index32 ? GL_UNSIGNED_INT : GL_UNSIGNED_SHORT;
        glDrawElements(mode, (GLsizei)call.indexCount, indexType, call.indexData);
    } else {
        glDrawArrays(mode, (GLint)call.startVertex, (GLsizei)call.vertexCount);
    }

    TeardownVertexArrays(call.fvf);
}

bool GLRenderBackend::Readback(uint32_t x, uint32_t y, uint32_t width, uint32_t height,
                               uint32_t* outArgb)
{
    if (!outArgb || !width || !height) return false;
    glPixelStorei(GL_PACK_ALIGNMENT, 1);
    glReadPixels((GLint)x, (GLint)y, (GLsizei)width, (GLsizei)height,
                 GL_BGRA, GL_UNSIGNED_BYTE, outArgb);
    return glGetError() == GL_NO_ERROR;
}

} // namespace gl
} // namespace eterbase

#endif // OPENMT2_RENDERER_GL
