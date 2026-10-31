#pragma once

#include "Platform.h"
#include <cmath>
#include <vector>
#include <map>
#include <cstring>
#include <algorithm>

#if defined(OPENMT2_PLATFORM_LINUX)
#if defined(__GNUC__) || defined(__clang__)
bool Platform_GetImageInfo(const void* data, size_t size, uint32_t* outW, uint32_t* outH, uint32_t* outFormat);
bool Platform_DecodeImage(const void* data, size_t size, uint32_t* outW, uint32_t* outH, std::vector<uint8_t>& outPixels);
#else
bool Platform_GetImageInfo(const void* data, size_t size, uint32_t* outW, uint32_t* outH, uint32_t* outFormat);
bool Platform_DecodeImage(const void* data, size_t size, uint32_t* outW, uint32_t* outH, std::vector<uint8_t>& outPixels);
#endif
#endif

#if defined(_WIN32) && !defined(USE_PLATFORM_D3D_COMPAT)
    // Legacy Windows branch (D1/D2 retired the native path). The DirectX SDK
    // headers are gone: the compat vocabulary below is the only contract, so a
    // Windows build defines USE_PLATFORM_D3D_COMPAT and takes the else branch.
    #include <d3d9.h>
#else

// Renderer seam contract (T1.7): resources are named by opaque handles so no
// GL type leaks above the backend (invariant R2).
#include "RenderBackend.h"

// d3d9.h defines this; <windows.h> alone does not. The compat vocabulary needs
// it for the DXT formats (T1.5 drops the DirectX SDK headers).
#ifndef MAKEFOURCC
#define MAKEFOURCC(ch0, ch1, ch2, ch3) \
    ((DWORD)(BYTE)(ch0) | ((DWORD)(BYTE)(ch1) << 8) | \
     ((DWORD)(BYTE)(ch2) << 16) | ((DWORD)(BYTE)(ch3) << 24))
#endif

// =============================================================
// 1. D3D9 Basic Types, Constants & Enums
// =============================================================
typedef DWORD D3DCOLOR;

#ifndef D3DCOLOR_ARGB
#define D3DCOLOR_ARGB(a,r,g,b) \
    ((D3DCOLOR)((((a)&0xff)<<24)|(((r)&0xff)<<16)|(((g)&0xff)<<8)|((b)&0xff)))
#endif

#ifndef D3DCOLOR_RGBA
#define D3DCOLOR_RGBA(r,g,b,a) D3DCOLOR_ARGB(a,r,g,b)
#endif

#ifndef D3DCOLOR_XRGB
#define D3DCOLOR_XRGB(r,g,b)   D3DCOLOR_ARGB(0xff,r,g,b)
#endif

#ifndef D3DCOLOR_COLORVALUE
#define D3DCOLOR_COLORVALUE(r,g,b,a) \
    D3DCOLOR_RGBA((DWORD)((r)*255.f),(DWORD)((g)*255.f),(DWORD)((b)*255.f),(DWORD)((a)*255.f))
#endif

#define D3D_SDK_VERSION   32
#define D3DADAPTER_DEFAULT 0
#define D3DENUM_WHQL_LEVEL 0x00000002L

#define D3DX_PI           3.141592654f
#define D3DXToRadian(degree) ((degree) * (D3DX_PI / 180.0f))
#define D3DXToDegree(radian) ((radian) * (180.0f / D3DX_PI))

#define D3DX_DEFAULT       ((UINT)-1)
#define D3DX_FILTER_NONE   0x00000001
#define D3DX_FILTER_POINT  0x00000002
#define D3DX_FILTER_LINEAR 0x00000003

// D3D9 Error Codes
#define D3D_OK                    S_OK
#define D3DERR_OUTOFVIDEOMEMORY   ((HRESULT)0x8876017CL)
#define D3DERR_DEVICELOST         ((HRESULT)0x88760868L)
#define D3DERR_DEVICENOTRESET     ((HRESULT)0x88760869L)
#define D3DERR_NOTAVAILABLE       ((HRESULT)0x8876086AL)
#define D3DERR_INVALIDCALL        ((HRESULT)0x8876086CL)

// D3DCREATE Flags
#define D3DCREATE_FPU_PRESERVE              0x00000002L
#define D3DCREATE_MULTITHREADED             0x00000004L
#define D3DCREATE_PUREDEVICE                0x00000010L
#define D3DCREATE_SOFTWARE_VERTEXPROCESSING 0x00000020L
#define D3DCREATE_HARDWARE_VERTEXPROCESSING 0x00000040L
#define D3DCREATE_MIXED_VERTEXPROCESSING    0x00000080L

// D3DCAPS Flags
#define D3DDEVCAPS_PUREDEVICE            0x00100000L
#define D3DDEVCAPS_HWTRANSFORMANDLIGHT   0x00010000L
#define D3DPMISCCAPS_CLIPTLVERTS         0x00000200L
#define D3DVTXPCAPS_TEXGEN               0x00000001L
#define D3DVTXPCAPS_MATERIALSOURCE7      0x00000002L
#define D3DVTXPCAPS_DIRECTIONALLIGHTS    0x00000008L
#define D3DVTXPCAPS_POSITIONALLIGHTS     0x00000010L
#define D3DVTXPCAPS_LOCALVIEWER          0x00000020L
#define D3DPTADDRESSCAPS_BORDER          0x00000008L
#define D3DPTFILTERCAPS_MINFANISOTROPIC  0x00000400L
#define D3DPTFILTERCAPS_MAGFANISOTROPIC  0x04000000L
#define D3DVS_VERSION(_Major,_Minor)     (0xFFFE0000|((_Major)<<8)|(_Minor))
#define D3DPS_VERSION(_Major,_Minor)     (0xFFFF0000|((_Major)<<8)|(_Minor))

// D3DCLEAR Flags
#define D3DCLEAR_TARGET  0x00000001L
#define D3DCLEAR_ZBUFFER 0x00000002L
#define D3DCLEAR_STENCIL 0x00000004L

// D3DFVF Flags
#define D3DFVF_XYZ              0x002
#define D3DFVF_XYZRHW           0x004
#define D3DFVF_XYZB1            0x006
#define D3DFVF_XYZB2            0x008
#define D3DFVF_XYZB3            0x00a
#define D3DFVF_XYZB4            0x00c
#define D3DFVF_XYZB5            0x00e
#define D3DFVF_POSITION_MASK    0x400E
#define D3DFVF_NORMAL           0x010
#define D3DFVF_PSIZE            0x020
#define D3DFVF_DIFFUSE          0x040
#define D3DFVF_SPECULAR         0x080
#define D3DFVF_TEX0             0x000
#define D3DFVF_TEX1             0x100
#define D3DFVF_TEX2             0x200
#define D3DFVF_TEX3             0x300
#define D3DFVF_TEX4             0x400
#define D3DFVF_TEX5             0x500
#define D3DFVF_TEX6             0x600
#define D3DFVF_TEX7             0x700
#define D3DFVF_TEX8             0x800
#define D3DFVF_TEXCOUNT_MASK    0xf00
#define D3DFVF_TEXCOUNT_SHIFT   8

#define D3DFVF_TEXTUREFORMAT2 0
#define D3DFVF_TEXTUREFORMAT1 3
#define D3DFVF_TEXTUREFORMAT3 1
#define D3DFVF_TEXTUREFORMAT4 2

#define D3DFVF_TEXCOORDSIZE2(CoordIndex) (D3DFVF_TEXTUREFORMAT2 << (CoordIndex*2 + 16))
#define D3DFVF_TEXCOORDSIZE3(CoordIndex) (D3DFVF_TEXTUREFORMAT3 << (CoordIndex*2 + 16))
#define D3DFVF_TEXCOORDSIZE4(CoordIndex) (D3DFVF_TEXTUREFORMAT4 << (CoordIndex*2 + 16))
#define D3DFVF_TEXCOORDSIZE1(CoordIndex) (D3DFVF_TEXTUREFORMAT1 << (CoordIndex*2 + 16))

// D3DUSAGE & D3DLOCK Flags
#define D3DUSAGE_RENDERTARGET       0x00000001L
#define D3DUSAGE_DEPTHSTENCIL       0x00000002L
#define D3DUSAGE_WRITEONLY          0x00000008L
#define D3DUSAGE_SOFTWAREPROCESSING 0x00000010L
#define D3DUSAGE_DYNAMIC            0x00000200L

#define D3DLOCK_READONLY        0x00000010L
#define D3DLOCK_DISCARD         0x00002000L
#define D3DLOCK_NOOVERWRITE     0x00001000L
#define D3DLOCK_NOSYSLOCK       0x00000800L
#define D3DLOCK_NO_DIRTY_UPDATE 0x00008000L

#define D3DPRESENTFLAG_LOCKABLE_BACKBUFFER 0x00000001
#define D3DPRESENT_INTERVAL_DEFAULT        0x00000000L
#define D3DPRESENT_INTERVAL_ONE            0x00000001L
#define D3DPRESENT_INTERVAL_IMMEDIATE      0x80000000L

// D3DTA Texture Arguments
#define D3DTA_SELECTMASK        0x0000000f
#define D3DTA_DIFFUSE           0x00000000
#define D3DTA_CURRENT           0x00000001
#define D3DTA_TEXTURE           0x00000002
#define D3DTA_TFACTOR           0x00000003
#define D3DTA_SPECULAR          0x00000004
#define D3DTA_TEMP              0x00000005
#define D3DTA_CONSTANT          0x00000006
#define D3DTA_COMPLEMENT        0x00000010
#define D3DTA_ALPHAREPLICATE    0x00000020

typedef enum _D3DDEVTYPE {
    D3DDEVTYPE_HAL = 1,
    D3DDEVTYPE_REF = 2,
    D3DDEVTYPE_SW  = 3
} D3DDEVTYPE;

typedef enum _D3DRESOURCETYPE {
    D3DRTYPE_SURFACE       = 1,
    D3DRTYPE_VOLUME        = 2,
    D3DRTYPE_TEXTURE       = 3,
    D3DRTYPE_VOLUMETEXTURE = 4,
    D3DRTYPE_CUBETEXTURE   = 5,
    D3DRTYPE_VERTEXBUFFER  = 6,
    D3DRTYPE_INDEXBUFFER   = 7
} D3DRESOURCETYPE;

typedef enum _D3DFORMAT {
    D3DFMT_UNKNOWN      = 0,
    D3DFMT_R8G8B8       = 20,
    D3DFMT_A8R8G8B8     = 21,
    D3DFMT_X8R8G8B8     = 22,
    D3DFMT_R5G6B5       = 23,
    D3DFMT_X1R5G5B5     = 24,
    D3DFMT_A1R5G5B5     = 25,
    D3DFMT_A4R4G4B4     = 26,
    D3DFMT_R3G3B2       = 27,
    D3DFMT_A8           = 28,
    D3DFMT_A8R3G3B2     = 29,
    D3DFMT_X4R4G4B4     = 30,
    D3DFMT_A2B10G10R10  = 31,
    D3DFMT_DXT1         = MAKEFOURCC('D', 'X', 'T', '1'),
    D3DFMT_DXT2         = MAKEFOURCC('D', 'X', 'T', '2'),
    D3DFMT_DXT3         = MAKEFOURCC('D', 'X', 'T', '3'),
    D3DFMT_DXT4         = MAKEFOURCC('D', 'X', 'T', '4'),
    D3DFMT_DXT5         = MAKEFOURCC('D', 'X', 'T', '5'),
    D3DFMT_D16_LOCKABLE = 70,
    D3DFMT_D32          = 71,
    D3DFMT_D15S1        = 73,
    D3DFMT_D24S8        = 75,
    D3DFMT_D24X8        = 77,
    D3DFMT_D24X4S4      = 79,
    D3DFMT_D16          = 80,
    D3DFMT_INDEX16      = 101,
    D3DFMT_INDEX32      = 102,
    D3DFMT_R32F         = 114
} D3DFORMAT;

typedef enum _D3DPOOL {
    D3DPOOL_DEFAULT   = 0,
    D3DPOOL_MANAGED   = 1,
    D3DPOOL_SYSTEMMEM = 2,
    D3DPOOL_SCRATCH   = 3
} D3DPOOL;

typedef enum _D3DMULTISAMPLE_TYPE {
    D3DMULTISAMPLE_NONE       = 0,
    D3DMULTISAMPLE_NONMASKABLE = 1,
    D3DMULTISAMPLE_2_SAMPLES  = 2,
    D3DMULTISAMPLE_4_SAMPLES  = 4
} D3DMULTISAMPLE_TYPE;

typedef enum _D3DSWAPEFFECT {
    D3DSWAPEFFECT_DISCARD = 1,
    D3DSWAPEFFECT_FLIP    = 2,
    D3DSWAPEFFECT_COPY    = 3
} D3DSWAPEFFECT;

typedef enum _D3DPRIMITIVETYPE {
    D3DPT_POINTLIST     = 1,
    D3DPT_LINELIST      = 2,
    D3DPT_LINESTRIP     = 3,
    D3DPT_TRIANGLELIST  = 4,
    D3DPT_TRIANGLESTRIP = 5,
    D3DPT_TRIANGLEFAN   = 6
} D3DPRIMITIVETYPE;

typedef enum _D3DTRANSFORMSTATETYPE {
    D3DTS_VIEW       = 2,
    D3DTS_PROJECTION = 3,
    D3DTS_TEXTURE0   = 16,
    D3DTS_TEXTURE1   = 17,
    D3DTS_TEXTURE2   = 18,
    D3DTS_TEXTURE3   = 19,
    D3DTS_TEXTURE4   = 20,
    D3DTS_TEXTURE5   = 21,
    D3DTS_TEXTURE6   = 22,
    D3DTS_TEXTURE7   = 23,
    D3DTS_WORLD      = 256
} D3DTRANSFORMSTATETYPE;

typedef enum _D3DRENDERSTATETYPE {
    D3DRS_ZENABLE                  = 7,
    D3DRS_FILLMODE                 = 8,
    D3DRS_SHADEMODE                = 9,
    D3DRS_ZWRITEENABLE             = 14,
    D3DRS_ALPHATESTENABLE          = 15,
    D3DRS_LASTPIXEL                = 16,
    D3DRS_SRCBLEND                 = 19,
    D3DRS_DESTBLEND                = 20,
    D3DRS_CULLMODE                 = 22,
    D3DRS_ZFUNC                    = 23,
    D3DRS_ALPHAREF                 = 24,
    D3DRS_ALPHAFUNC                = 25,
    D3DRS_DITHERENABLE             = 26,
    D3DRS_ALPHABLENDENABLE         = 27,
    D3DRS_FOGENABLE                = 28,
    D3DRS_SPECULARENABLE           = 29,
    D3DRS_FOGCOLOR                 = 34,
    D3DRS_FOGTABLEMODE             = 35,
    D3DRS_FOGSTART                 = 36,
    D3DRS_FOGEND                   = 37,
    D3DRS_FOGDENSITY               = 38,
    D3DRS_RANGEFOGENABLE           = 48,
    D3DRS_STENCILENABLE            = 52,
    D3DRS_STENCILFAIL              = 53,
    D3DRS_STENCILZFAIL             = 54,
    D3DRS_STENCILPASS              = 55,
    D3DRS_STENCILFUNC              = 56,
    D3DRS_STENCILREF               = 57,
    D3DRS_STENCILMASK              = 58,
    D3DRS_STENCILWRITEMASK         = 59,
    D3DRS_TEXTUREFACTOR            = 60,
    D3DRS_WRAP0                    = 128,
    D3DRS_WRAP1                    = 129,
    D3DRS_WRAP2                    = 130,
    D3DRS_WRAP3                    = 131,
    D3DRS_WRAP4                    = 132,
    D3DRS_WRAP5                    = 133,
    D3DRS_WRAP6                    = 134,
    D3DRS_WRAP7                    = 135,
    D3DRS_CLIPPING                 = 136,
    D3DRS_LIGHTING                 = 137,
    D3DRS_AMBIENT                  = 139,
    D3DRS_FOGVERTEXMODE            = 140,
    D3DRS_COLORVERTEX              = 141,
    D3DRS_LOCALVIEWER              = 142,
    D3DRS_NORMALIZENORMALS         = 143,
    D3DRS_DIFFUSEMATERIALSOURCE    = 145,
    D3DRS_SPECULARMATERIALSOURCE   = 146,
    D3DRS_AMBIENTMATERIALSOURCE    = 147,
    D3DRS_EMISSIVEMATERIALSOURCE   = 148,
    D3DRS_VERTEXBLEND              = 151,
    D3DRS_CLIPPLANEENABLE          = 152,
    D3DRS_POINTSIZE                = 154,
    D3DRS_POINTSIZE_MIN            = 155,
    D3DRS_POINTSPRITEENABLE        = 156,
    D3DRS_POINTSCALEENABLE         = 157,
    D3DRS_POINTSCALE_A             = 158,
    D3DRS_POINTSCALE_B             = 159,
    D3DRS_POINTSCALE_C             = 160,
    D3DRS_MULTISAMPLEANTIALIAS     = 161,
    D3DRS_MULTISAMPLEMASK          = 162,
    D3DRS_PATCHEDGESTYLE           = 163,
    D3DRS_DEBUGMONITORTOKEN        = 165,
    D3DRS_POINTSIZE_MAX            = 166,
    D3DRS_INDEXEDVERTEXBLENDENABLE = 167,
    D3DRS_COLORWRITEENABLE         = 168,
    D3DRS_TWEENFACTOR              = 170,
    D3DRS_BLENDOP                  = 171
} D3DRENDERSTATETYPE;

typedef enum _D3DTEXTURESTAGESTATETYPE {
    D3DTSS_COLOROP               = 1,
    D3DTSS_COLORARG1             = 2,
    D3DTSS_COLORARG2             = 3,
    D3DTSS_ALPHAOP               = 4,
    D3DTSS_ALPHAARG1             = 5,
    D3DTSS_ALPHAARG2             = 6,
    D3DTSS_BUMPENVMAT00          = 7,
    D3DTSS_BUMPENVMAT01          = 8,
    D3DTSS_BUMPENVMAT10          = 9,
    D3DTSS_BUMPENVMAT11          = 10,
    D3DTSS_TEXCOORDINDEX         = 11,
    D3DTSS_BUMPENVLSCALE         = 22,
    D3DTSS_BUMPENVLOFFSET        = 23,
    D3DTSS_TEXTURETRANSFORMFLAGS = 24,
    D3DTSS_COLORARG0             = 26,
    D3DTSS_ALPHAARG0             = 27,
    D3DTSS_RESULTARG             = 28
} D3DTEXTURESTAGESTATETYPE;

#define D3DTSS_TCI_PASSTHRU                    0x00000000
#define D3DTSS_TCI_CAMERASPACENORMAL           0x00010000
#define D3DTSS_TCI_CAMERASPACEPOSITION         0x00020000
#define D3DTSS_TCI_CAMERASPACEREFLECTIONVECTOR 0x00030000
#define D3DTSS_TCI_SPHEREMAP                   0x00040000

typedef enum _D3DSAMPLERSTATETYPE {
    D3DSAMP_ADDRESSU      = 1,
    D3DSAMP_ADDRESSV      = 2,
    D3DSAMP_ADDRESSW      = 3,
    D3DSAMP_BORDERCOLOR   = 4,
    D3DSAMP_MAGFILTER     = 5,
    D3DSAMP_MINFILTER     = 6,
    D3DSAMP_MIPFILTER     = 7,
    D3DSAMP_MIPMAPLODBIAS = 8,
    D3DSAMP_MAXMIPLEVEL   = 9,
    D3DSAMP_MAXANISOTROPY = 10
} D3DSAMPLERSTATETYPE;

typedef enum _D3DTEXTUREOP {
    D3DTOP_DISABLE                   = 1,
    D3DTOP_SELECTARG1                = 2,
    D3DTOP_SELECTARG2                = 3,
    D3DTOP_MODULATE                  = 4,
    D3DTOP_MODULATE2X                = 5,
    D3DTOP_MODULATE4X                = 6,
    D3DTOP_ADD                       = 7,
    D3DTOP_ADDSIGNED                 = 8,
    D3DTOP_ADDSIGNED2X               = 9,
    D3DTOP_SUBTRACT                  = 10,
    D3DTOP_ADDSMOOTH                 = 11,
    D3DTOP_BLENDDIFFUSEALPHA         = 12,
    D3DTOP_BLENDTEXTUREALPHA         = 13,
    D3DTOP_BLENDFACTORALPHA          = 14,
    D3DTOP_BLENDTEXTUREALPHAPM       = 15,
    D3DTOP_BLENDCURRENTALPHA         = 16,
    D3DTOP_PREMODULATE               = 17,
    D3DTOP_MODULATEALPHA_ADDCOLOR    = 18,
    D3DTOP_MODULATECOLOR_ADDALPHA    = 19,
    D3DTOP_MODULATEINVALPHA_ADDCOLOR = 20,
    D3DTOP_MODULATEINVCOLOR_ADDALPHA = 21,
    D3DTOP_BUMPENVMAP                = 22,
    D3DTOP_BUMPENVMAPLUMINANCE       = 23,
    D3DTOP_DOTPRODUCT3               = 24,
    D3DTOP_MULTIPLYADD               = 25,
    D3DTOP_LERP                      = 26
} D3DTEXTUREOP;

typedef enum _D3DTEXTUREFILTERTYPE {
    D3DTEXF_NONE        = 0,
    D3DTEXF_POINT       = 1,
    D3DTEXF_LINEAR      = 2,
    D3DTEXF_ANISOTROPIC = 3
} D3DTEXTUREFILTERTYPE;

typedef enum _D3DTEXTUREADDRESS {
    D3DTADDRESS_WRAP       = 1,
    D3DTADDRESS_MIRROR     = 2,
    D3DTADDRESS_CLAMP      = 3,
    D3DTADDRESS_BORDER     = 4,
    D3DTADDRESS_MIRRORONCE = 5
} D3DTEXTUREADDRESS;

typedef enum _D3DBLEND {
    D3DBLEND_ZERO            = 1,
    D3DBLEND_ONE             = 2,
    D3DBLEND_SRCCOLOR        = 3,
    D3DBLEND_INVSRCCOLOR     = 4,
    D3DBLEND_SRCALPHA        = 5,
    D3DBLEND_INVSRCALPHA     = 6,
    D3DBLEND_DESTALPHA       = 7,
    D3DBLEND_INVDESTALPHA    = 8,
    D3DBLEND_DESTCOLOR       = 9,
    D3DBLEND_INVDESTCOLOR    = 10,
    D3DBLEND_SRCALPHASAT     = 11,
    D3DBLEND_BOTHSRCALPHA    = 12,
    D3DBLEND_BOTHINVSRCALPHA = 13
} D3DBLEND;

typedef enum _D3DBLENDOP {
    D3DBLENDOP_ADD         = 1,
    D3DBLENDOP_SUBTRACT    = 2,
    D3DBLENDOP_REVSUBTRACT = 3,
    D3DBLENDOP_MIN         = 4,
    D3DBLENDOP_MAX         = 5
} D3DBLENDOP;

typedef enum _D3DCMPFUNC {
    D3DCMP_NEVER        = 1,
    D3DCMP_LESS         = 2,
    D3DCMP_EQUAL        = 3,
    D3DCMP_LESSEQUAL    = 4,
    D3DCMP_GREATER      = 5,
    D3DCMP_NOTEQUAL     = 6,
    D3DCMP_GREATEREQUAL = 7,
    D3DCMP_ALWAYS       = 8
} D3DCMPFUNC;

typedef enum _D3DCULL {
    D3DCULL_NONE = 1,
    D3DCULL_CW   = 2,
    D3DCULL_CCW  = 3
} D3DCULL;

typedef enum _D3DFILLMODE {
    D3DFILL_POINT     = 1,
    D3DFILL_WIREFRAME = 2,
    D3DFILL_SOLID     = 3
} D3DFILLMODE;

typedef enum _D3DSHADEMODE {
    D3DSHADE_FLAT    = 1,
    D3DSHADE_GOURAUD = 2,
    D3DSHADE_PHONG   = 3
} D3DSHADEMODE;

typedef enum _D3DFOGMODE {
    D3DFOG_NONE   = 0,
    D3DFOG_EXP    = 1,
    D3DFOG_EXP2   = 2,
    D3DFOG_LINEAR = 3
} D3DFOGMODE;

typedef enum _D3DMATERIALCOLORSOURCE {
    D3DMCS_MATERIAL = 0,
    D3DMCS_COLOR1   = 1,
    D3DMCS_COLOR2   = 2
} D3DMATERIALCOLORSOURCE;

typedef enum _D3DVERTEXBLENDFLAGS {
    D3DVBF_DISABLE  = 0,
    D3DVBF_1WEIGHTS = 1,
    D3DVBF_2WEIGHTS = 2,
    D3DVBF_3WEIGHTS = 3,
    D3DVBF_TWEENING = 255,
    D3DVBF_0WEIGHTS = 256
} D3DVERTEXBLENDFLAGS;

typedef enum _D3DTEXTURETRANSFORMFLAGS {
    D3DTTFF_DISABLE   = 0,
    D3DTTFF_COUNT1    = 1,
    D3DTTFF_COUNT2    = 2,
    D3DTTFF_COUNT3    = 3,
    D3DTTFF_COUNT4    = 4,
    D3DTTFF_PROJECTED = 256
} D3DTEXTURETRANSFORMFLAGS;

typedef enum _D3DLIGHTTYPE {
    D3DLIGHT_POINT       = 1,
    D3DLIGHT_SPOT        = 2,
    D3DLIGHT_DIRECTIONAL = 3
} D3DLIGHTTYPE;

// =============================================================
// 2. D3D9 & D3DX Math Structures
// =============================================================
struct D3DVECTOR {
    float x;
    float y;
    float z;
};

struct D3DCOLORVALUE {
    float r;
    float g;
    float b;
    float a;
};

struct D3DMATRIX {
    union {
        struct {
            float _11, _12, _13, _14;
            float _21, _22, _23, _24;
            float _31, _32, _33, _34;
            float _41, _42, _43, _44;
        };
        float m[4][4];
    };
};

struct D3DXVECTOR2 {
    float x, y;

    D3DXVECTOR2() : x(0.0f), y(0.0f) {}
    D3DXVECTOR2(const float* pf) : x(pf ? pf[0] : 0.0f), y(pf ? pf[1] : 0.0f) {}
    D3DXVECTOR2(float _x, float _y) : x(_x), y(_y) {}

    operator float* () { return &x; }
    operator const float* () const { return &x; }

    D3DXVECTOR2* operator & () { return this; }
    const D3DXVECTOR2* operator & () const { return this; }

    D3DXVECTOR2& operator += (const D3DXVECTOR2& v) { x += v.x; y += v.y; return *this; }
    D3DXVECTOR2& operator -= (const D3DXVECTOR2& v) { x -= v.x; y -= v.y; return *this; }
    D3DXVECTOR2& operator *= (float f) { x *= f; y *= f; return *this; }
    D3DXVECTOR2& operator /= (float f) { float inv = 1.0f / f; x *= inv; y *= inv; return *this; }

    D3DXVECTOR2 operator + () const { return *this; }
    D3DXVECTOR2 operator - () const { return D3DXVECTOR2(-x, -y); }

    D3DXVECTOR2 operator + (const D3DXVECTOR2& v) const { return D3DXVECTOR2(x + v.x, y + v.y); }
    D3DXVECTOR2 operator - (const D3DXVECTOR2& v) const { return D3DXVECTOR2(x - v.x, y - v.y); }
    D3DXVECTOR2 operator * (float f) const { return D3DXVECTOR2(x * f, y * f); }
    D3DXVECTOR2 operator / (float f) const { float inv = 1.0f / f; return D3DXVECTOR2(x * inv, y * inv); }

    friend D3DXVECTOR2 operator * (float f, const D3DXVECTOR2& v) { return D3DXVECTOR2(f * v.x, f * v.y); }

    bool operator == (const D3DXVECTOR2& v) const { return x == v.x && y == v.y; }
    bool operator != (const D3DXVECTOR2& v) const { return x != v.x || y != v.y; }
};

struct D3DXVECTOR3 : public D3DVECTOR {
    D3DXVECTOR3() { x = 0.0f; y = 0.0f; z = 0.0f; }
    D3DXVECTOR3(const float* pf) { x = pf[0]; y = pf[1]; z = pf[2]; }
    D3DXVECTOR3(const D3DVECTOR& v) { x = v.x; y = v.y; z = v.z; }
    D3DXVECTOR3(float _x, float _y, float _z) { x = _x; y = _y; z = _z; }

    operator float* () { return (float*)&x; }
    operator const float* () const { return (const float*)&x; }

    D3DXVECTOR3* operator & () { return this; }
    const D3DXVECTOR3* operator & () const { return this; }

    D3DXVECTOR3& operator += (const D3DXVECTOR3& v) { x += v.x; y += v.y; z += v.z; return *this; }
    D3DXVECTOR3& operator -= (const D3DXVECTOR3& v) { x -= v.x; y -= v.y; z -= v.z; return *this; }
    D3DXVECTOR3& operator *= (float f) { x *= f; y *= f; z *= f; return *this; }
    D3DXVECTOR3& operator /= (float f) { float inv = 1.0f / f; x *= inv; y *= inv; z *= inv; return *this; }

    D3DXVECTOR3 operator + () const { return *this; }
    D3DXVECTOR3 operator - () const { return D3DXVECTOR3(-x, -y, -z); }

    D3DXVECTOR3 operator + (const D3DXVECTOR3& v) const { return D3DXVECTOR3(x + v.x, y + v.y, z + v.z); }
    D3DXVECTOR3 operator - (const D3DXVECTOR3& v) const { return D3DXVECTOR3(x - v.x, y - v.y, z - v.z); }
    D3DXVECTOR3 operator * (float f) const { return D3DXVECTOR3(x * f, y * f, z * f); }
    D3DXVECTOR3 operator / (float f) const { float inv = 1.0f / f; return D3DXVECTOR3(x * inv, y * inv, z * inv); }

    friend D3DXVECTOR3 operator * (float f, const D3DXVECTOR3& v) { return D3DXVECTOR3(f * v.x, f * v.y, f * v.z); }

    bool operator == (const D3DXVECTOR3& v) const { return x == v.x && y == v.y && z == v.z; }
    bool operator != (const D3DXVECTOR3& v) const { return x != v.x || y != v.y || z != v.z; }
};

struct D3DXVECTOR4 {
    float x, y, z, w;

    D3DXVECTOR4() : x(0.0f), y(0.0f), z(0.0f), w(0.0f) {}
    D3DXVECTOR4(const float* pf) : x(pf[0]), y(pf[1]), z(pf[2]), w(pf[3]) {}
    D3DXVECTOR4(const D3DXVECTOR3& v, float _w) : x(v.x), y(v.y), z(v.z), w(_w) {}
    D3DXVECTOR4(float _x, float _y, float _z, float _w) : x(_x), y(_y), z(_z), w(_w) {}

    operator float* () { return &x; }
    operator const float* () const { return &x; }

    D3DXVECTOR4* operator & () { return this; }
    const D3DXVECTOR4* operator & () const { return this; }

    D3DXVECTOR4& operator += (const D3DXVECTOR4& v) { x += v.x; y += v.y; z += v.z; w += v.w; return *this; }
    D3DXVECTOR4& operator -= (const D3DXVECTOR4& v) { x -= v.x; y -= v.y; z -= v.z; w -= v.w; return *this; }
    D3DXVECTOR4& operator *= (float f) { x *= f; y *= f; z *= f; w *= f; return *this; }
    D3DXVECTOR4& operator /= (float f) { float inv = 1.0f / f; x *= inv; y *= inv; z *= inv; w *= inv; return *this; }

    D3DXVECTOR4 operator + () const { return *this; }
    D3DXVECTOR4 operator - () const { return D3DXVECTOR4(-x, -y, -z, -w); }

    D3DXVECTOR4 operator + (const D3DXVECTOR4& v) const { return D3DXVECTOR4(x + v.x, y + v.y, z + v.z, w + v.w); }
    D3DXVECTOR4 operator - (const D3DXVECTOR4& v) const { return D3DXVECTOR4(x - v.x, y - v.y, z - v.z, w - v.w); }
    D3DXVECTOR4 operator * (float f) const { return D3DXVECTOR4(x * f, y * f, z * f, w * f); }
    D3DXVECTOR4 operator / (float f) const { float inv = 1.0f / f; return D3DXVECTOR4(x * inv, y * inv, z * inv, w * inv); }

    friend D3DXVECTOR4 operator * (float f, const D3DXVECTOR4& v) { return D3DXVECTOR4(f * v.x, f * v.y, f * v.z, f * v.w); }

    bool operator == (const D3DXVECTOR4& v) const { return x == v.x && y == v.y && z == v.z && w == v.w; }
    bool operator != (const D3DXVECTOR4& v) const { return !(*this == v); }
};

struct D3DXQUATERNION {
    float x, y, z, w;

    D3DXQUATERNION() : x(0.0f), y(0.0f), z(0.0f), w(1.0f) {}
    D3DXQUATERNION(const float* pf) : x(pf[0]), y(pf[1]), z(pf[2]), w(pf[3]) {}
    D3DXQUATERNION(float _x, float _y, float _z, float _w) : x(_x), y(_y), z(_z), w(_w) {}

    operator float* () { return &x; }
    operator const float* () const { return &x; }

    D3DXQUATERNION* operator & () { return this; }
    const D3DXQUATERNION* operator & () const { return this; }

    D3DXQUATERNION& operator += (const D3DXQUATERNION& q) { x += q.x; y += q.y; z += q.z; w += q.w; return *this; }
    D3DXQUATERNION& operator -= (const D3DXQUATERNION& q) { x -= q.x; y -= q.y; z -= q.z; w -= q.w; return *this; }
    D3DXQUATERNION& operator *= (float f) { x *= f; y *= f; z *= f; w *= f; return *this; }
    D3DXQUATERNION& operator /= (float f) { float inv = 1.0f / f; x *= inv; y *= inv; z *= inv; w *= inv; return *this; }

    D3DXQUATERNION operator + () const { return *this; }
    D3DXQUATERNION operator - () const { return D3DXQUATERNION(-x, -y, -z, -w); }

    D3DXQUATERNION operator + (const D3DXQUATERNION& q) const { return D3DXQUATERNION(x + q.x, y + q.y, z + q.z, w + q.w); }
    D3DXQUATERNION operator - (const D3DXQUATERNION& q) const { return D3DXQUATERNION(x - q.x, y - q.y, z - q.z, w - q.w); }
    D3DXQUATERNION operator * (float f) const { return D3DXQUATERNION(x * f, y * f, z * f, w * f); }
    D3DXQUATERNION operator / (float f) const { float inv = 1.0f / f; return D3DXQUATERNION(x * inv, y * inv, z * inv, w * inv); }

    friend D3DXQUATERNION operator * (float f, const D3DXQUATERNION& q) { return D3DXQUATERNION(f * q.x, f * q.y, f * q.z, f * q.w); }

    bool operator == (const D3DXQUATERNION& q) const { return x == q.x && y == q.y && z == q.z && w == q.w; }
    bool operator != (const D3DXQUATERNION& q) const { return !(*this == q); }
};

struct D3DXCOLOR {
    float r, g, b, a;

    D3DXCOLOR() : r(0.0f), g(0.0f), b(0.0f), a(0.0f) {}
    D3DXCOLOR(DWORD dw) {
        const float f = 1.0f / 255.0f;
        r = f * (float)(BYTE)(dw >> 16);
        g = f * (float)(BYTE)(dw >> 8);
        b = f * (float)(BYTE)(dw >> 0);
        a = f * (float)(BYTE)(dw >> 24);
    }
    D3DXCOLOR(const float* pf) : r(pf[0]), g(pf[1]), b(pf[2]), a(pf[3]) {}
    D3DXCOLOR(const D3DCOLORVALUE& c) : r(c.r), g(c.g), b(c.b), a(c.a) {}
    D3DXCOLOR(float _r, float _g, float _b, float _a) : r(_r), g(_g), b(_b), a(_a) {}

    operator DWORD () const {
        DWORD dwR = r >= 1.0f ? 0xff : r <= 0.0f ? 0x00 : (DWORD)(r * 255.0f + 0.5f);
        DWORD dwG = g >= 1.0f ? 0xff : g <= 0.0f ? 0x00 : (DWORD)(g * 255.0f + 0.5f);
        DWORD dwB = b >= 1.0f ? 0xff : b <= 0.0f ? 0x00 : (DWORD)(b * 255.0f + 0.5f);
        DWORD dwA = a >= 1.0f ? 0xff : a <= 0.0f ? 0x00 : (DWORD)(a * 255.0f + 0.5f);
        return (dwA << 24) | (dwR << 16) | (dwG << 8) | dwB;
    }

    D3DXCOLOR* operator & () { return this; }
    const D3DXCOLOR* operator & () const { return this; }

    operator float* () { return &r; }
    operator const float* () const { return &r; }
    operator D3DCOLORVALUE* () { return (D3DCOLORVALUE*)&r; }
    operator const D3DCOLORVALUE* () const { return (const D3DCOLORVALUE*)&r; }
    operator D3DCOLORVALUE& () { return *(D3DCOLORVALUE*)&r; }
    operator const D3DCOLORVALUE& () const { return *(const D3DCOLORVALUE*)&r; }

    D3DXCOLOR& operator += (const D3DXCOLOR& c) { r += c.r; g += c.g; b += c.b; a += c.a; return *this; }
    D3DXCOLOR& operator -= (const D3DXCOLOR& c) { r -= c.r; g -= c.g; b -= c.b; a -= c.a; return *this; }
    D3DXCOLOR& operator *= (float f) { r *= f; g *= f; b *= f; a *= f; return *this; }
    D3DXCOLOR& operator /= (float f) { float inv = 1.0f / f; r *= inv; g *= inv; b *= inv; a *= inv; return *this; }

    D3DXCOLOR operator + () const { return *this; }
    D3DXCOLOR operator - () const { return D3DXCOLOR(-r, -g, -b, -a); }

    D3DXCOLOR operator + (const D3DXCOLOR& c) const { return D3DXCOLOR(r + c.r, g + c.g, b + c.b, a + c.a); }
    D3DXCOLOR operator - (const D3DXCOLOR& c) const { return D3DXCOLOR(r - c.r, g - c.g, b - c.b, a - c.a); }
    D3DXCOLOR operator * (float f) const { return D3DXCOLOR(r * f, g * f, b * f, a * f); }
    D3DXCOLOR operator / (float f) const { float inv = 1.0f / f; return D3DXCOLOR(r * inv, g * inv, b * inv, a * inv); }

    friend D3DXCOLOR operator * (float f, const D3DXCOLOR& c) { return D3DXCOLOR(f * c.r, f * c.g, f * c.b, f * c.a); }

    bool operator == (const D3DXCOLOR& c) const { return r == c.r && g == c.g && b == c.b && a == c.a; }
    bool operator != (const D3DXCOLOR& c) const { return !(*this == c); }
};

inline D3DXCOLOR* D3DXColorModulate(D3DXCOLOR* pOut, const D3DXCOLOR* pC1, const D3DXCOLOR* pC2) {
    if (!pOut || !pC1 || !pC2) return pOut;
    pOut->r = pC1->r * pC2->r;
    pOut->g = pC1->g * pC2->g;
    pOut->b = pC1->b * pC2->b;
    pOut->a = pC1->a * pC2->a;
    return pOut;
}

struct D3DXMATRIX : public D3DMATRIX {
    D3DXMATRIX() {
        _11 = 1.0f; _12 = 0.0f; _13 = 0.0f; _14 = 0.0f;
        _21 = 0.0f; _22 = 1.0f; _23 = 0.0f; _24 = 0.0f;
        _31 = 0.0f; _32 = 0.0f; _33 = 1.0f; _34 = 0.0f;
        _41 = 0.0f; _42 = 0.0f; _43 = 0.0f; _44 = 1.0f;
    }
    D3DXMATRIX(const float* pf) { std::memcpy(&m[0][0], pf, sizeof(D3DMATRIX)); }
    D3DXMATRIX(const D3DMATRIX& mat) { *static_cast<D3DMATRIX*>(this) = mat; }
    D3DXMATRIX(float f11, float f12, float f13, float f14,
               float f21, float f22, float f23, float f24,
               float f31, float f32, float f33, float f34,
               float f41, float f42, float f43, float f44) {
        _11 = f11; _12 = f12; _13 = f13; _14 = f14;
        _21 = f21; _22 = f22; _23 = f23; _24 = f24;
        _31 = f31; _32 = f32; _33 = f33; _34 = f34;
        _41 = f41; _42 = f42; _43 = f43; _44 = f44;
    }

    float& operator () (unsigned int Row, unsigned int Col) { return m[Row][Col]; }
    float  operator () (unsigned int Row, unsigned int Col) const { return m[Row][Col]; }

    operator float* () { return &_11; }
    operator const float* () const { return &_11; }

    D3DXMATRIX operator * (const D3DXMATRIX& rhs) const {
        D3DXMATRIX out;
        for (int i = 0; i < 4; ++i) {
            for (int j = 0; j < 4; ++j) {
                out.m[i][j] = m[i][0] * rhs.m[0][j] +
                              m[i][1] * rhs.m[1][j] +
                              m[i][2] * rhs.m[2][j] +
                              m[i][3] * rhs.m[3][j];
            }
        }
        return out;
    }

    D3DXMATRIX& operator *= (const D3DXMATRIX& rhs) {
        *this = (*this) * rhs;
        return *this;
    }

    bool operator == (const D3DXMATRIX& rhs) const {
        return std::memcmp(&m[0][0], &rhs.m[0][0], sizeof(D3DMATRIX)) == 0;
    }
    bool operator != (const D3DXMATRIX& rhs) const {
        return !(*this == rhs);
    }
};

struct D3DXPLANE {
    float a, b, c, d;

    D3DXPLANE() : a(0.0f), b(0.0f), c(0.0f), d(0.0f) {}
    D3DXPLANE(const float* pf) : a(pf[0]), b(pf[1]), c(pf[2]), d(pf[3]) {}
    D3DXPLANE(float _a, float _b, float _c, float _d) : a(_a), b(_b), c(_c), d(_d) {}

    operator float* () { return &a; }
    operator const float* () const { return &a; }
};

// =============================================================
// 3. D3D9 Device Structs
// =============================================================
typedef struct _D3DMATERIAL9 {
    D3DCOLORVALUE Diffuse;
    D3DCOLORVALUE Ambient;
    D3DCOLORVALUE Specular;
    D3DCOLORVALUE Emissive;
    float         Power;
} D3DMATERIAL9;

typedef struct _D3DLIGHT9 {
    D3DLIGHTTYPE  Type;
    D3DCOLORVALUE Diffuse;
    D3DCOLORVALUE Specular;
    D3DCOLORVALUE Ambient;
    D3DVECTOR     Position;
    D3DVECTOR     Direction;
    float         Range;
    float         Falloff;
    float         Attenuation0;
    float         Attenuation1;
    float         Attenuation2;
    float         Theta;
    float         Phi;
} D3DLIGHT9;

typedef struct _D3DVIEWPORT9 {
    DWORD X;
    DWORD Y;
    DWORD Width;
    DWORD Height;
    float MinZ;
    float MaxZ;
} D3DVIEWPORT9;

typedef struct _D3DDISPLAYMODE {
    UINT      Width;
    UINT      Height;
    UINT      RefreshRate;
    D3DFORMAT Format;
} D3DDISPLAYMODE;

#define MAX_DEVICE_IDENTIFIER_STRING 512
typedef struct _D3DADAPTER_IDENTIFIER9 {
    char          Driver[MAX_DEVICE_IDENTIFIER_STRING];
    char          Description[MAX_DEVICE_IDENTIFIER_STRING];
    char          DeviceName[32];
    LARGE_INTEGER DriverVersion;
    DWORD         VendorId;
    DWORD         DeviceId;
    DWORD         SubSysId;
    DWORD         Revision;
    GUID          DeviceIdentifier;
    DWORD         WHQLLevel;
} D3DADAPTER_IDENTIFIER9;

typedef struct _D3DPRESENT_PARAMETERS_ {
    UINT                BackBufferWidth;
    UINT                BackBufferHeight;
    D3DFORMAT           BackBufferFormat;
    UINT                BackBufferCount;
    D3DMULTISAMPLE_TYPE MultiSampleType;
    DWORD               MultiSampleQuality;
    D3DSWAPEFFECT       SwapEffect;
    HWND                hDeviceWindow;
    BOOL                Windowed;
    BOOL                EnableAutoDepthStencil;
    D3DFORMAT           AutoDepthStencilFormat;
    DWORD               Flags;
    UINT                FullScreen_RefreshRateInHz;
    UINT                PresentationInterval;
} D3DPRESENT_PARAMETERS;

typedef struct _D3DCAPS9 {
    D3DDEVTYPE DeviceType;
    UINT       AdapterOrdinal;
    DWORD      Caps;
    DWORD      Caps2;
    DWORD      Caps3;
    DWORD      PresentationIntervals;
    DWORD      CursorCaps;
    DWORD      DevCaps;
    DWORD      PrimitiveMiscCaps;
    DWORD      RasterCaps;
    DWORD      ZCmpCaps;
    DWORD      SrcBlendCaps;
    DWORD      DestBlendCaps;
    DWORD      AlphaCmpCaps;
    DWORD      ShadeCaps;
    DWORD      TextureCaps;
    DWORD      TextureFilterCaps;
    DWORD      CubeTextureFilterCaps;
    DWORD      VolumeTextureFilterCaps;
    DWORD      TextureAddressCaps;
    DWORD      VolumeTextureAddressCaps;
    DWORD      LineCaps;
    DWORD      MaxTextureWidth;
    DWORD      MaxTextureHeight;
    DWORD      MaxVolumeExtent;
    DWORD      MaxTextureRepeat;
    DWORD      MaxTextureAspectRatio;
    DWORD      MaxAnisotropy;
    float      MaxVertexW;
    float      GuardBandLeft;
    float      GuardBandTop;
    float      GuardBandRight;
    float      GuardBandBottom;
    float      ExtentsAdjust;
    DWORD      StencilCaps;
    DWORD      FVFCaps;
    DWORD      TextureOpCaps;
    DWORD      MaxTextureBlendStages;
    DWORD      MaxSimultaneousTextures;
    DWORD      VertexProcessingCaps;
    DWORD      MaxActiveLights;
    DWORD      MaxUserClipPlanes;
    DWORD      MaxVertexBlendMatrices;
    DWORD      MaxVertexBlendMatrixIndex;
    float      MaxPointSize;
    DWORD      MaxPrimitiveCount;
    DWORD      MaxVertexIndex;
    DWORD      MaxStreams;
    DWORD      MaxStreamStride;
    DWORD      VertexShaderVersion;
    DWORD      MaxVertexShaderConst;
    DWORD      PixelShaderVersion;
    float      PixelShader1xMaxValue;
    DWORD      DevCaps2;
} D3DCAPS9;

#define D3DCAPS2_FULLSCREENGAMMA 0x00020000L
#define D3DSGR_NO_CALIBRATION    0x00000000L
#define D3DBACKBUFFER_TYPE_MONO  0

typedef struct _D3DGAMMARAMP {
    WORD red[256];
    WORD green[256];
    WORD blue[256];
} D3DGAMMARAMP;

typedef struct _D3DSURFACE_DESC {
    D3DFORMAT           Format;
    D3DRESOURCETYPE     Type;
    DWORD               Usage;
    D3DPOOL             Pool;
    D3DMULTISAMPLE_TYPE MultiSampleType;
    DWORD               MultiSampleQuality;
    UINT                Width;
    UINT                Height;
} D3DSURFACE_DESC;

typedef struct _D3DLOCKED_RECT {
    INT   Pitch;
    void* pBits;
} D3DLOCKED_RECT;

typedef struct _D3DXIMAGE_INFO {
    UINT            Width;
    UINT            Height;
    UINT            Depth;
    UINT            MipLevels;
    D3DFORMAT       Format;
    D3DRESOURCETYPE ResourceType;
    DWORD           ImageFileFormat;
} D3DXIMAGE_INFO;

// =============================================================
// 4. D3DX Math Functions
// =============================================================
inline float D3DXVec2LengthSq(const D3DXVECTOR2* pV) {
    if (!pV) return 0.0f;
    return pV->x * pV->x + pV->y * pV->y;
}

inline float D3DXVec2Length(const D3DXVECTOR2* pV) {
    return std::sqrt(D3DXVec2LengthSq(pV));
}

inline float D3DXVec2Dot(const D3DXVECTOR2* pV1, const D3DXVECTOR2* pV2) {
    if (!pV1 || !pV2) return 0.0f;
    return pV1->x * pV2->x + pV1->y * pV2->y;
}

inline float D3DXVec2CCW(const D3DXVECTOR2* pV1, const D3DXVECTOR2* pV2) {
    if (!pV1 || !pV2) return 0.0f;
    return pV1->x * pV2->y - pV1->y * pV2->x;
}

inline D3DXVECTOR2* D3DXVec2Add(D3DXVECTOR2* pOut, const D3DXVECTOR2* pV1, const D3DXVECTOR2* pV2) {
    if (!pOut || !pV1 || !pV2) return nullptr;
    pOut->x = pV1->x + pV2->x;
    pOut->y = pV1->y + pV2->y;
    return pOut;
}

inline D3DXVECTOR2* D3DXVec2Subtract(D3DXVECTOR2* pOut, const D3DXVECTOR2* pV1, const D3DXVECTOR2* pV2) {
    if (!pOut || !pV1 || !pV2) return nullptr;
    pOut->x = pV1->x - pV2->x;
    pOut->y = pV1->y - pV2->y;
    return pOut;
}

inline D3DXVECTOR2* D3DXVec2Scale(D3DXVECTOR2* pOut, const D3DXVECTOR2* pV, float s) {
    if (!pOut || !pV) return nullptr;
    pOut->x = pV->x * s;
    pOut->y = pV->y * s;
    return pOut;
}

inline D3DXVECTOR2* D3DXVec2Normalize(D3DXVECTOR2* pOut, const D3DXVECTOR2* pV) {
    if (!pOut || !pV) return nullptr;
    float len = D3DXVec2Length(pV);
    if (len > 1e-8f) {
        float inv = 1.0f / len;
        pOut->x = pV->x * inv;
        pOut->y = pV->y * inv;
    } else {
        pOut->x = 0.0f;
        pOut->y = 0.0f;
    }
    return pOut;
}

inline float D3DXVec3LengthSq(const D3DXVECTOR3* pV) {
    if (!pV) return 0.0f;
    return pV->x * pV->x + pV->y * pV->y + pV->z * pV->z;
}

inline float D3DXVec3Length(const D3DXVECTOR3* pV) {
    return std::sqrt(D3DXVec3LengthSq(pV));
}

inline float D3DXVec3Dot(const D3DXVECTOR3* pV1, const D3DXVECTOR3* pV2) {
    if (!pV1 || !pV2) return 0.0f;
    return pV1->x * pV2->x + pV1->y * pV2->y + pV1->z * pV2->z;
}

inline D3DXVECTOR3* D3DXVec3Cross(D3DXVECTOR3* pOut, const D3DXVECTOR3* pV1, const D3DXVECTOR3* pV2) {
    if (!pOut || !pV1 || !pV2) return nullptr;
    D3DXVECTOR3 v(
        pV1->y * pV2->z - pV1->z * pV2->y,
        pV1->z * pV2->x - pV1->x * pV2->z,
        pV1->x * pV2->y - pV1->y * pV2->x
    );
    *pOut = v;
    return pOut;
}

inline D3DXVECTOR3* D3DXVec3Add(D3DXVECTOR3* pOut, const D3DXVECTOR3* pV1, const D3DXVECTOR3* pV2) {
    if (!pOut || !pV1 || !pV2) return nullptr;
    pOut->x = pV1->x + pV2->x;
    pOut->y = pV1->y + pV2->y;
    pOut->z = pV1->z + pV2->z;
    return pOut;
}

inline D3DXVECTOR3* D3DXVec3Subtract(D3DXVECTOR3* pOut, const D3DXVECTOR3* pV1, const D3DXVECTOR3* pV2) {
    if (!pOut || !pV1 || !pV2) return nullptr;
    pOut->x = pV1->x - pV2->x;
    pOut->y = pV1->y - pV2->y;
    pOut->z = pV1->z - pV2->z;
    return pOut;
}

inline D3DXVECTOR3* D3DXVec3Scale(D3DXVECTOR3* pOut, const D3DXVECTOR3* pV, float s) {
    if (!pOut || !pV) return nullptr;
    pOut->x = pV->x * s;
    pOut->y = pV->y * s;
    pOut->z = pV->z * s;
    return pOut;
}

inline D3DXVECTOR3* D3DXVec3Lerp(D3DXVECTOR3* pOut, const D3DXVECTOR3* pV1, const D3DXVECTOR3* pV2, float s) {
    if (!pOut || !pV1 || !pV2) return nullptr;
    pOut->x = pV1->x + s * (pV2->x - pV1->x);
    pOut->y = pV1->y + s * (pV2->y - pV1->y);
    pOut->z = pV1->z + s * (pV2->z - pV1->z);
    return pOut;
}

inline D3DXVECTOR3* D3DXVec3Normalize(D3DXVECTOR3* pOut, const D3DXVECTOR3* pV) {
    if (!pOut || !pV) return nullptr;
    float len = D3DXVec3Length(pV);
    if (len > 1e-8f) {
        float inv = 1.0f / len;
        pOut->x = pV->x * inv;
        pOut->y = pV->y * inv;
        pOut->z = pV->z * inv;
    } else {
        pOut->x = 0.0f;
        pOut->y = 0.0f;
        pOut->z = 0.0f;
    }
    return pOut;
}

inline D3DXVECTOR3* D3DXVec3TransformCoord(D3DXVECTOR3* pOut, const D3DXVECTOR3* pV, const D3DXMATRIX* pM) {
    if (!pOut || !pV || !pM) return nullptr;
    float x = pV->x * pM->_11 + pV->y * pM->_21 + pV->z * pM->_31 + pM->_41;
    float y = pV->x * pM->_12 + pV->y * pM->_22 + pV->z * pM->_32 + pM->_42;
    float z = pV->x * pM->_13 + pV->y * pM->_23 + pV->z * pM->_33 + pM->_43;
    float w = pV->x * pM->_14 + pV->y * pM->_24 + pV->z * pM->_34 + pM->_44;
    if (std::fabs(w) > 1e-8f && w != 1.0f) {
        float invW = 1.0f / w;
        x *= invW; y *= invW; z *= invW;
    }
    pOut->x = x; pOut->y = y; pOut->z = z;
    return pOut;
}

inline D3DXVECTOR4* D3DXVec3Transform(D3DXVECTOR4* pOut, const D3DXVECTOR3* pV, const D3DXMATRIX* pM) {
    if (!pOut || !pV || !pM) return nullptr;
    float x = pV->x * pM->_11 + pV->y * pM->_21 + pV->z * pM->_31 + pM->_41;
    float y = pV->x * pM->_12 + pV->y * pM->_22 + pV->z * pM->_32 + pM->_42;
    float z = pV->x * pM->_13 + pV->y * pM->_23 + pV->z * pM->_33 + pM->_43;
    float w = pV->x * pM->_14 + pV->y * pM->_24 + pV->z * pM->_34 + pM->_44;
    pOut->x = x; pOut->y = y; pOut->z = z; pOut->w = w;
    return pOut;
}

inline D3DXVECTOR4* D3DXVec4Transform(D3DXVECTOR4* pOut, const D3DXVECTOR4* pV, const D3DXMATRIX* pM) {
    if (!pOut || !pV || !pM) return nullptr;
    float x = pV->x * pM->_11 + pV->y * pM->_21 + pV->z * pM->_31 + pV->w * pM->_41;
    float y = pV->x * pM->_12 + pV->y * pM->_22 + pV->z * pM->_32 + pV->w * pM->_42;
    float z = pV->x * pM->_13 + pV->y * pM->_23 + pV->z * pM->_33 + pV->w * pM->_43;
    float w = pV->x * pM->_14 + pV->y * pM->_24 + pV->z * pM->_34 + pV->w * pM->_44;
    pOut->x = x; pOut->y = y; pOut->z = z; pOut->w = w;
    return pOut;
}

inline D3DXVECTOR3* D3DXVec3TransformNormal(D3DXVECTOR3* pOut, const D3DXVECTOR3* pV, const D3DXMATRIX* pM) {
    if (!pOut || !pV || !pM) return nullptr;
    float x = pV->x * pM->_11 + pV->y * pM->_21 + pV->z * pM->_31;
    float y = pV->x * pM->_12 + pV->y * pM->_22 + pV->z * pM->_32;
    float z = pV->x * pM->_13 + pV->y * pM->_23 + pV->z * pM->_33;
    pOut->x = x; pOut->y = y; pOut->z = z;
    return pOut;
}

inline D3DXMATRIX* D3DXMatrixIdentity(D3DXMATRIX* pOut) {
    if (!pOut) return nullptr;
    *pOut = D3DXMATRIX();
    return pOut;
}

inline D3DXMATRIX* D3DXMatrixMultiply(D3DXMATRIX* pOut, const D3DXMATRIX* pM1, const D3DXMATRIX* pM2) {
    if (!pOut || !pM1 || !pM2) return nullptr;
    *pOut = (*pM1) * (*pM2);
    return pOut;
}

inline D3DXMATRIX* D3DXMatrixTranspose(D3DXMATRIX* pOut, const D3DXMATRIX* pM) {
    if (!pOut || !pM) return nullptr;
    D3DXMATRIX tmp;
    for (int i = 0; i < 4; ++i)
        for (int j = 0; j < 4; ++j)
            tmp.m[i][j] = pM->m[j][i];
    *pOut = tmp;
    return pOut;
}

inline float D3DXMatrixDeterminant(const D3DXMATRIX* pM) {
    if (!pM) return 0.0f;
    const float* m = &pM->_11;
    float s0 = m[0] * m[5] - m[4] * m[1];
    float s1 = m[0] * m[6] - m[4] * m[2];
    float s2 = m[0] * m[7] - m[4] * m[3];
    float s3 = m[1] * m[6] - m[5] * m[2];
    float s4 = m[1] * m[7] - m[5] * m[3];
    float s5 = m[2] * m[7] - m[6] * m[3];

    float c5 = m[10] * m[15] - m[14] * m[11];
    float c4 = m[9]  * m[15] - m[13] * m[11];
    float c3 = m[9]  * m[14] - m[13] * m[10];
    float c2 = m[8]  * m[15] - m[12] * m[11];
    float c1 = m[8]  * m[14] - m[12] * m[10];
    float c0 = m[8]  * m[13] - m[12] * m[9];

    return (s0 * c5 - s1 * c4 + s2 * c3 + s3 * c2 - s4 * c1 + s5 * c0);
}

inline D3DXMATRIX* D3DXMatrixInverse(D3DXMATRIX* pOut, float* pDeterminant, const D3DXMATRIX* pM) {
    if (!pOut || !pM) return nullptr;
    const float* m = &pM->_11;
    float s0 = m[0] * m[5] - m[4] * m[1];
    float s1 = m[0] * m[6] - m[4] * m[2];
    float s2 = m[0] * m[7] - m[4] * m[3];
    float s3 = m[1] * m[6] - m[5] * m[2];
    float s4 = m[1] * m[7] - m[5] * m[3];
    float s5 = m[2] * m[7] - m[6] * m[3];

    float c5 = m[10] * m[15] - m[14] * m[11];
    float c4 = m[9]  * m[15] - m[13] * m[11];
    float c3 = m[9]  * m[14] - m[13] * m[10];
    float c2 = m[8]  * m[15] - m[12] * m[11];
    float c1 = m[8]  * m[14] - m[12] * m[10];
    float c0 = m[8]  * m[13] - m[12] * m[9];

    float det = (s0 * c5 - s1 * c4 + s2 * c3 + s3 * c2 - s4 * c1 + s5 * c0);
    if (pDeterminant) *pDeterminant = det;
    if (std::fabs(det) < 1e-12f) return nullptr;

    float invDet = 1.0f / det;
    D3DXMATRIX res;
    res.m[0][0] = ( m[5] * c5 - m[6] * c4 + m[7] * c3) * invDet;
    res.m[0][1] = (-m[1] * c5 + m[2] * c4 - m[3] * c3) * invDet;
    res.m[0][2] = ( m[13] * s5 - m[14] * s4 + m[15] * s3) * invDet;
    res.m[0][3] = (-m[9] * s5 + m[10] * s4 - m[11] * s3) * invDet;

    res.m[1][0] = (-m[4] * c5 + m[6] * c2 - m[7] * c1) * invDet;
    res.m[1][1] = ( m[0] * c5 - m[2] * c2 + m[3] * c1) * invDet;
    res.m[1][2] = (-m[12] * s5 + m[14] * s2 - m[15] * s1) * invDet;
    res.m[1][3] = ( m[8] * s5 - m[10] * s2 + m[11] * s1) * invDet;

    res.m[2][0] = ( m[4] * c4 - m[5] * c2 + m[7] * c0) * invDet;
    res.m[2][1] = (-m[0] * c4 + m[1] * c2 - m[3] * c0) * invDet;
    res.m[2][2] = ( m[12] * s4 - m[13] * s2 + m[15] * s0) * invDet;
    res.m[2][3] = (-m[8] * s4 + m[9] * s2 - m[11] * s0) * invDet;

    res.m[3][0] = (-m[4] * c3 + m[5] * c1 - m[6] * c0) * invDet;
    res.m[3][1] = ( m[0] * c3 - m[1] * c1 + m[2] * c0) * invDet;
    res.m[3][2] = (-m[12] * s3 + m[13] * s1 - m[14] * s0) * invDet;
    res.m[3][3] = ( m[8] * s3 - m[9] * s1 + m[10] * s0) * invDet;

    *pOut = res;
    return pOut;
}

inline D3DXMATRIX* D3DXMatrixScaling(D3DXMATRIX* pOut, float sx, float sy, float sz) {
    if (!pOut) return nullptr;
    D3DXMatrixIdentity(pOut);
    pOut->_11 = sx; pOut->_22 = sy; pOut->_33 = sz;
    return pOut;
}

inline D3DXMATRIX* D3DXMatrixTranslation(D3DXMATRIX* pOut, float x, float y, float z) {
    if (!pOut) return nullptr;
    D3DXMatrixIdentity(pOut);
    pOut->_41 = x; pOut->_42 = y; pOut->_43 = z;
    return pOut;
}

inline D3DXMATRIX* D3DXMatrixRotationX(D3DXMATRIX* pOut, float Angle) {
    if (!pOut) return nullptr;
    float s = std::sin(Angle), c = std::cos(Angle);
    D3DXMatrixIdentity(pOut);
    pOut->_22 = c;  pOut->_23 = s;
    pOut->_32 = -s; pOut->_33 = c;
    return pOut;
}

inline D3DXMATRIX* D3DXMatrixRotationY(D3DXMATRIX* pOut, float Angle) {
    if (!pOut) return nullptr;
    float s = std::sin(Angle), c = std::cos(Angle);
    D3DXMatrixIdentity(pOut);
    pOut->_11 = c; pOut->_13 = -s;
    pOut->_31 = s; pOut->_33 = c;
    return pOut;
}

inline D3DXMATRIX* D3DXMatrixRotationZ(D3DXMATRIX* pOut, float Angle) {
    if (!pOut) return nullptr;
    float s = std::sin(Angle), c = std::cos(Angle);
    D3DXMatrixIdentity(pOut);
    pOut->_11 = c;  pOut->_12 = s;
    pOut->_21 = -s; pOut->_22 = c;
    return pOut;
}

inline D3DXMATRIX* D3DXMatrixRotationAxis(D3DXMATRIX* pOut, const D3DXVECTOR3* pV, float Angle) {
    if (!pOut || !pV) return nullptr;
    D3DXVECTOR3 v;
    D3DXVec3Normalize(&v, pV);
    float s = std::sin(Angle), c = std::cos(Angle), t = 1.0f - c;
    D3DXMatrixIdentity(pOut);
    pOut->_11 = t * v.x * v.x + c;
    pOut->_12 = t * v.x * v.y + s * v.z;
    pOut->_13 = t * v.x * v.z - s * v.y;
    pOut->_21 = t * v.x * v.y - s * v.z;
    pOut->_22 = t * v.y * v.y + c;
    pOut->_23 = t * v.y * v.z + s * v.x;
    pOut->_31 = t * v.x * v.z + s * v.y;
    pOut->_32 = t * v.y * v.z - s * v.x;
    pOut->_33 = t * v.z * v.z + c;
    return pOut;
}

inline D3DXMATRIX* D3DXMatrixRotationYawPitchRoll(D3DXMATRIX* pOut, float Yaw, float Pitch, float Roll) {
    if (!pOut) return nullptr;
    D3DXMATRIX mRoll, mPitch, mYaw;
    D3DXMatrixRotationZ(&mRoll, Roll);
    D3DXMatrixRotationX(&mPitch, Pitch);
    D3DXMatrixRotationY(&mYaw, Yaw);
    *pOut = mRoll * mPitch * mYaw;
    return pOut;
}

inline D3DXMATRIX* D3DXMatrixRotationQuaternion(D3DXMATRIX* pOut, const D3DXQUATERNION* pQ) {
    if (!pOut || !pQ) return nullptr;
    float xx = pQ->x * pQ->x, yy = pQ->y * pQ->y, zz = pQ->z * pQ->z;
    float xy = pQ->x * pQ->y, xz = pQ->x * pQ->z, yz = pQ->y * pQ->z;
    float wx = pQ->w * pQ->x, wy = pQ->w * pQ->y, wz = pQ->w * pQ->z;
    D3DXMatrixIdentity(pOut);
    pOut->_11 = 1.0f - 2.0f * (yy + zz);
    pOut->_12 = 2.0f * (xy + wz);
    pOut->_13 = 2.0f * (xz - wy);
    pOut->_21 = 2.0f * (xy - wz);
    pOut->_22 = 1.0f - 2.0f * (xx + zz);
    pOut->_23 = 2.0f * (yz + wx);
    pOut->_31 = 2.0f * (xz + wy);
    pOut->_32 = 2.0f * (yz - wx);
    pOut->_33 = 1.0f - 2.0f * (xx + yy);
    return pOut;
}

inline D3DXMATRIX* D3DXMatrixLookAtRH(D3DXMATRIX* pOut, const D3DXVECTOR3* pEye, const D3DXVECTOR3* pAt, const D3DXVECTOR3* pUp) {
    if (!pOut || !pEye || !pAt || !pUp) return nullptr;
    D3DXVECTOR3 zaxis = (*pEye) - (*pAt);
    D3DXVec3Normalize(&zaxis, &zaxis);
    D3DXVECTOR3 xaxis;
    D3DXVec3Cross(&xaxis, pUp, &zaxis);
    D3DXVec3Normalize(&xaxis, &xaxis);
    D3DXVECTOR3 yaxis;
    D3DXVec3Cross(&yaxis, &zaxis, &xaxis);

    D3DXMatrixIdentity(pOut);
    pOut->_11 = xaxis.x; pOut->_12 = yaxis.x; pOut->_13 = zaxis.x;
    pOut->_21 = xaxis.y; pOut->_22 = yaxis.y; pOut->_23 = zaxis.y;
    pOut->_31 = xaxis.z; pOut->_32 = yaxis.z; pOut->_33 = zaxis.z;
    pOut->_41 = -D3DXVec3Dot(&xaxis, pEye);
    pOut->_42 = -D3DXVec3Dot(&yaxis, pEye);
    pOut->_43 = -D3DXVec3Dot(&zaxis, pEye);
    return pOut;
}

inline D3DXMATRIX* D3DXMatrixPerspectiveFovRH(D3DXMATRIX* pOut, float fovy, float Aspect, float zn, float zf) {
    if (!pOut) return nullptr;
    std::memset(pOut, 0, sizeof(D3DXMATRIX));
    float yScale = 1.0f / std::tan(fovy * 0.5f);
    float xScale = yScale / Aspect;
    pOut->_11 = xScale;
    pOut->_22 = yScale;
    pOut->_33 = zf / (zn - zf);
    pOut->_34 = -1.0f;
    pOut->_43 = zn * zf / (zn - zf);
    return pOut;
}

inline D3DXMATRIX* D3DXMatrixOrthoRH(D3DXMATRIX* pOut, float w, float h, float zn, float zf) {
    if (!pOut) return nullptr;
    D3DXMatrixIdentity(pOut);
    pOut->_11 = 2.0f / w;
    pOut->_22 = 2.0f / h;
    pOut->_33 = 1.0f / (zn - zf);
    pOut->_43 = zn / (zn - zf);
    return pOut;
}

inline D3DXMATRIX* D3DXMatrixOrthoOffCenterRH(D3DXMATRIX* pOut, float l, float r, float b, float t, float zn, float zf) {
    if (!pOut) return nullptr;
    D3DXMatrixIdentity(pOut);
    pOut->_11 = 2.0f / (r - l);
    pOut->_22 = 2.0f / (t - b);
    pOut->_33 = 1.0f / (zn - zf);
    pOut->_41 = (l + r) / (l - r);
    pOut->_42 = (t + b) / (b - t);
    pOut->_43 = zn / (zn - zf);
    return pOut;
}

inline D3DXVECTOR3* D3DXVec3Project(
    D3DXVECTOR3* pOut,
    const D3DXVECTOR3* pV,
    const D3DVIEWPORT9* pViewport,
    const D3DXMATRIX* pProjection,
    const D3DXMATRIX* pView,
    const D3DXMATRIX* pWorld)
{
    if (!pOut || !pV) return nullptr;
    D3DXMATRIX mat = D3DXMATRIX();
    if (pWorld) mat = mat * (*pWorld);
    if (pView) mat = mat * (*pView);
    if (pProjection) mat = mat * (*pProjection);
    D3DXVECTOR3 v;
    D3DXVec3TransformCoord(&v, pV, &mat);
    if (pViewport) {
        pOut->x = pViewport->X + (1.0f + v.x) * pViewport->Width * 0.5f;
        pOut->y = pViewport->Y + (1.0f - v.y) * pViewport->Height * 0.5f;
        pOut->z = pViewport->MinZ + v.z * (pViewport->MaxZ - pViewport->MinZ);
    } else {
        *pOut = v;
    }
    return pOut;
}

inline D3DXVECTOR3* D3DXVec3Unproject(
    D3DXVECTOR3* pOut,
    const D3DXVECTOR3* pV,
    const D3DVIEWPORT9* pViewport,
    const D3DXMATRIX* pProjection,
    const D3DXMATRIX* pView,
    const D3DXMATRIX* pWorld)
{
    if (!pOut || !pV) return nullptr;
    D3DXMATRIX mat = D3DXMATRIX();
    if (pWorld) mat = mat * (*pWorld);
    if (pView) mat = mat * (*pView);
    if (pProjection) mat = mat * (*pProjection);
    D3DXMATRIX invMat;
    if (!D3DXMatrixInverse(&invMat, nullptr, &mat)) {
        *pOut = *pV;
        return pOut;
    }
    D3DXVECTOR3 v = *pV;
    if (pViewport && pViewport->Width > 0 && pViewport->Height > 0) {
        v.x = ((v.x - pViewport->X) / (float)pViewport->Width) * 2.0f - 1.0f;
        v.y = 1.0f - ((v.y - pViewport->Y) / (float)pViewport->Height) * 2.0f;
        if (pViewport->MaxZ != pViewport->MinZ)
            v.z = (v.z - pViewport->MinZ) / (pViewport->MaxZ - pViewport->MinZ);
    }
    return D3DXVec3TransformCoord(pOut, &v, &invMat);
}

inline D3DXQUATERNION* D3DXQuaternionIdentity(D3DXQUATERNION* pOut) {
    if (!pOut) return nullptr;
    pOut->x = 0.0f; pOut->y = 0.0f; pOut->z = 0.0f; pOut->w = 1.0f;
    return pOut;
}

inline D3DXQUATERNION* D3DXQuaternionRotationAxis(D3DXQUATERNION* pOut, const D3DXVECTOR3* pV, float Angle) {
    if (!pOut || !pV) return nullptr;
    D3DXVECTOR3 v;
    D3DXVec3Normalize(&v, pV);
    float halfAngle = Angle * 0.5f;
    float sinHalf = sinf(halfAngle);
    pOut->x = v.x * sinHalf;
    pOut->y = v.y * sinHalf;
    pOut->z = v.z * sinHalf;
    pOut->w = cosf(halfAngle);
    return pOut;
}

inline D3DXQUATERNION* D3DXQuaternionRotationYawPitchRoll(D3DXQUATERNION* pOut, float Yaw, float Pitch, float Roll) {
    if (!pOut) return nullptr;
    float halfYaw = Yaw * 0.5f;
    float halfPitch = Pitch * 0.5f;
    float halfRoll = Roll * 0.5f;
    float cosYaw = std::cos(halfYaw);
    float sinYaw = std::sin(halfYaw);
    float cosPitch = std::cos(halfPitch);
    float sinPitch = std::sin(halfPitch);
    float cosRoll = std::cos(halfRoll);
    float sinRoll = std::sin(halfRoll);

    pOut->x = cosYaw * sinPitch * cosRoll + sinYaw * cosPitch * sinRoll;
    pOut->y = sinYaw * cosPitch * cosRoll - cosYaw * sinPitch * sinRoll;
    pOut->z = cosYaw * cosPitch * sinRoll - sinYaw * sinPitch * cosRoll;
    pOut->w = cosYaw * cosPitch * cosRoll + sinYaw * sinPitch * sinRoll;
    return pOut;
}

inline D3DXQUATERNION* D3DXQuaternionConjugate(D3DXQUATERNION* pOut, const D3DXQUATERNION* pQ) {
    if (!pOut || !pQ) return nullptr;
    pOut->x = -pQ->x; pOut->y = -pQ->y; pOut->z = -pQ->z; pOut->w = pQ->w;
    return pOut;
}

inline D3DXQUATERNION* D3DXQuaternionMultiply(D3DXQUATERNION* pOut, const D3DXQUATERNION* pQ1, const D3DXQUATERNION* pQ2) {
    if (!pOut || !pQ1 || !pQ2) return nullptr;
    D3DXQUATERNION res(
        pQ2->w * pQ1->x + pQ2->x * pQ1->w + pQ2->y * pQ1->z - pQ2->z * pQ1->y,
        pQ2->w * pQ1->y - pQ2->x * pQ1->z + pQ2->y * pQ1->w + pQ2->z * pQ1->x,
        pQ2->w * pQ1->z + pQ2->x * pQ1->y - pQ2->y * pQ1->x + pQ2->z * pQ1->w,
        pQ2->w * pQ1->w - pQ2->x * pQ1->x - pQ2->y * pQ1->y - pQ2->z * pQ1->z
    );
    *pOut = res;
    return pOut;
}

inline D3DXQUATERNION* D3DXQuaternionSlerp(D3DXQUATERNION* pOut, const D3DXQUATERNION* pQ1, const D3DXQUATERNION* pQ2, float t) {
    if (!pOut || !pQ1 || !pQ2) return nullptr;
    float dot = pQ1->x * pQ2->x + pQ1->y * pQ2->y + pQ1->z * pQ2->z + pQ1->w * pQ2->w;
    float sign = 1.0f;
    if (dot < 0.0f) {
        dot = -dot;
        sign = -1.0f;
    }
    float scale0, scale1;
    if (dot > 0.9995f) {
        scale0 = 1.0f - t;
        scale1 = t * sign;
    } else {
        float theta = std::acos(std::clamp(dot, -1.0f, 1.0f));
        float sinTheta = std::sin(theta);
        scale0 = std::sin((1.0f - t) * theta) / sinTheta;
        scale1 = std::sin(t * theta) / sinTheta * sign;
    }
    pOut->x = scale0 * pQ1->x + scale1 * pQ2->x;
    pOut->y = scale0 * pQ1->y + scale1 * pQ2->y;
    pOut->z = scale0 * pQ1->z + scale1 * pQ2->z;
    pOut->w = scale0 * pQ1->w + scale1 * pQ2->w;
    return pOut;
}

inline float D3DXPlaneDotCoord(const D3DXPLANE* pP, const D3DXVECTOR3* pV) {
    if (!pP || !pV) return 0.0f;
    return pP->a * pV->x + pP->b * pV->y + pP->c * pV->z + pP->d;
}

inline D3DXPLANE* D3DXPlaneNormalize(D3DXPLANE* pOut, const D3DXPLANE* pP) {
    if (!pOut || !pP) return nullptr;
    float norm = std::sqrt(pP->a * pP->a + pP->b * pP->b + pP->c * pP->c);
    if (norm > 0.0f) {
        float inv = 1.0f / norm;
        pOut->a = pP->a * inv;
        pOut->b = pP->b * inv;
        pOut->c = pP->c * inv;
        pOut->d = pP->d * inv;
    } else {
        pOut->a = 0.0f; pOut->b = 0.0f; pOut->c = 0.0f; pOut->d = 0.0f;
    }
    return pOut;
}

inline UINT D3DXGetFVFVertexSize(DWORD dwFVF) {
    UINT size = 0;
    if ((dwFVF & D3DFVF_POSITION_MASK) == D3DFVF_XYZ) size += 12;
    else if ((dwFVF & D3DFVF_POSITION_MASK) == D3DFVF_XYZRHW) size += 16;
    if (dwFVF & D3DFVF_NORMAL) size += 12;
    if (dwFVF & D3DFVF_PSIZE) size += 4;
    if (dwFVF & D3DFVF_DIFFUSE) size += 4;
    if (dwFVF & D3DFVF_SPECULAR) size += 4;
    UINT texCount = (dwFVF & D3DFVF_TEXCOUNT_MASK) >> D3DFVF_TEXCOUNT_SHIFT;
    size += texCount * 8;
    return size;
}

// =============================================================
// 5. ID3DXMatrixStack (Real CPU Implementation)
// =============================================================
class ID3DXMatrixStack {
public:
    ID3DXMatrixStack() : m_refCount(1) {
        m_stack.push_back(D3DXMATRIX());
    }
    virtual ~ID3DXMatrixStack() = default;

    ULONG AddRef() { return ++m_refCount; }
    ULONG Release() {
        ULONG r = --m_refCount;
        if (r == 0) delete this;
        return r;
    }

    HRESULT Pop() {
        if (m_stack.size() > 1) m_stack.pop_back();
        return S_OK;
    }
    HRESULT Push() {
        m_stack.push_back(m_stack.back());
        return S_OK;
    }
    HRESULT LoadIdentity() {
        D3DXMatrixIdentity(&m_stack.back());
        return S_OK;
    }
    HRESULT LoadMatrix(const D3DXMATRIX* pM) {
        if (pM) m_stack.back() = *pM;
        return S_OK;
    }
    HRESULT MultMatrix(const D3DXMATRIX* pM) {
        if (pM) m_stack.back() = m_stack.back() * (*pM);
        return S_OK;
    }
    HRESULT MultMatrixLocal(const D3DXMATRIX* pM) {
        if (pM) m_stack.back() = (*pM) * m_stack.back();
        return S_OK;
    }
    HRESULT RotateAxis(const D3DXVECTOR3* pV, float Angle) {
        D3DXMATRIX m;
        D3DXMatrixRotationAxis(&m, pV, Angle);
        return MultMatrix(&m);
    }
    HRESULT RotateAxisLocal(const D3DXVECTOR3* pV, float Angle) {
        D3DXMATRIX m;
        D3DXMatrixRotationAxis(&m, pV, Angle);
        return MultMatrixLocal(&m);
    }
    HRESULT RotateYawPitchRoll(float Yaw, float Pitch, float Roll) {
        D3DXMATRIX m;
        D3DXMatrixRotationYawPitchRoll(&m, Yaw, Pitch, Roll);
        return MultMatrix(&m);
    }
    HRESULT RotateYawPitchRollLocal(float Yaw, float Pitch, float Roll) {
        D3DXMATRIX m;
        D3DXMatrixRotationYawPitchRoll(&m, Yaw, Pitch, Roll);
        return MultMatrixLocal(&m);
    }
    HRESULT Scale(float x, float y, float z) {
        D3DXMATRIX m;
        D3DXMatrixScaling(&m, x, y, z);
        return MultMatrix(&m);
    }
    HRESULT ScaleLocal(float x, float y, float z) {
        D3DXMATRIX m;
        D3DXMatrixScaling(&m, x, y, z);
        return MultMatrixLocal(&m);
    }
    HRESULT Translate(float x, float y, float z) {
        D3DXMATRIX m;
        D3DXMatrixTranslation(&m, x, y, z);
        return MultMatrix(&m);
    }
    HRESULT TranslateLocal(float x, float y, float z) {
        D3DXMATRIX m;
        D3DXMatrixTranslation(&m, x, y, z);
        return MultMatrixLocal(&m);
    }
    D3DXMATRIX* GetTop() {
        return &m_stack.back();
    }

private:
    ULONG m_refCount;
    std::vector<D3DXMATRIX> m_stack;
};

typedef ID3DXMatrixStack* LPD3DXMATRIXSTACK;

inline HRESULT D3DXCreateMatrixStack(DWORD /*Flags*/, LPD3DXMATRIXSTACK* ppStack) {
    if (!ppStack) return E_INVALIDARG;
    *ppStack = new ID3DXMatrixStack();
    return S_OK;
}

// =============================================================
// 6. Direct3D 9 & D3DX9 Resource / Device Interfaces
// =============================================================
class IDirect3DResource9 {
public:
    IDirect3DResource9() : m_refCount(1) {}
    virtual ~IDirect3DResource9() = default;
    virtual ULONG AddRef() { return ++m_refCount; }
    virtual ULONG Release() {
        ULONG r = --m_refCount;
        if (r == 0) delete this;
        return r;
    }
protected:
    ULONG m_refCount;
};

class IDirect3DTexture9;

class IDirect3DSurface9 : public IDirect3DResource9 {
public:
    IDirect3DSurface9(UINT w = 1, UINT h = 1, IDirect3DTexture9* pTexture = nullptr,
                      bool bDepthStencil = false, bool bDefault = false);
    ~IDirect3DSurface9() override;

    HRESULT LockRect(D3DLOCKED_RECT* pLockedRect, const RECT* /*pRect*/, DWORD /*Flags*/) {
        if (!pLockedRect) return E_INVALIDARG;
        pLockedRect->Pitch = (INT)(m_width * 4);
        pLockedRect->pBits = m_data.data();
        return S_OK;
    }
    HRESULT UnlockRect() { return S_OK; }
    HRESULT GetDesc(D3DSURFACE_DESC* pDesc) {
        if (!pDesc) return E_INVALIDARG;
        std::memset(pDesc, 0, sizeof(D3DSURFACE_DESC));
        pDesc->Format = m_bDepthStencil ? D3DFMT_D16 : D3DFMT_A8R8G8B8;
        pDesc->Type = D3DRTYPE_SURFACE;
        pDesc->Width = m_width;
        pDesc->Height = m_height;
        return S_OK;
    }
    UINT GetWidth() const { return m_width; }
    UINT GetHeight() const { return m_height; }

    IDirect3DTexture9* GetTexture() const { return m_texture; }
    bool IsDepthStencil() const { return m_bDepthStencil; }
    bool IsDefault() const { return m_bDefault; }
    RenderHandle GetDepthStencil();

private:
    UINT m_width;
    UINT m_height;
    std::vector<BYTE> m_data;
    IDirect3DTexture9* m_texture;
    bool m_bDepthStencil;
    bool m_bDefault;
    RenderHandle m_depthStencil;
};
typedef IDirect3DSurface9* LPDIRECT3DSURFACE9;

class IDirect3DBaseTexture9 : public IDirect3DResource9 {
public:
    virtual DWORD GetLevelCount() { return 1; }
};
typedef IDirect3DBaseTexture9* LPDIRECT3DBASETEXTURE9;

class IDirect3DTexture9 : public IDirect3DBaseTexture9 {
public:
    IDirect3DTexture9(UINT w = 1, UINT h = 1, D3DFORMAT fmt = D3DFMT_A8R8G8B8, UINT levels = 0);
    ~IDirect3DTexture9() override;

    UINT GetWidth() const { return m_width; }
    UINT GetHeight() const { return m_height; }
    D3DFORMAT GetFormat() const { return m_format; }
    void* GetData() { return m_data.data(); }
    const void* GetData() const { return m_data.data(); }
    size_t GetDataSize() const { return m_data.size(); }

    bool HasMipMaps() const { return !m_mipLevels.empty(); }
    DWORD GetLevelCount() override { return 1 + (DWORD)m_mipLevels.size(); }

    RenderHandle GetTextureHandle();
    HRESULT LockRect(UINT Level, D3DLOCKED_RECT* pLockedRect, const RECT* pRect, DWORD Flags);
    HRESULT UnlockRect(UINT Level);
    void UploadToBackend();
    void Fill(const DWORD color);
    INT GetPitch(UINT Level = 0) const;
    size_t GetLevelDataSize(UINT Level) const;
    static size_t GetFormatBytesPerPixel(D3DFORMAT fmt);
    HRESULT GetSurfaceLevel(UINT Level, IDirect3DSurface9** ppSurfaceLevel);

private:
    void UploadLevelToBackend(UINT level, const BYTE* pBytes, size_t dataSize);

    UINT m_width;
    UINT m_height;
    D3DFORMAT m_format;
    RenderHandle m_handle;
    bool m_dirty;
    std::vector<BYTE> m_data;
    std::map<UINT, std::vector<BYTE>> m_mipLevels;
};
typedef IDirect3DTexture9* LPDIRECT3DTEXTURE9;

class IDirect3DVertexBuffer9 : public IDirect3DResource9 {
public:
    explicit IDirect3DVertexBuffer9(UINT length = 64) : m_data(length ? length : 64, 0) {}
    HRESULT Lock(UINT OffsetToLock, UINT /*SizeToLock*/, void** ppbData, DWORD /*Flags*/) {
        if (!ppbData) return E_INVALIDARG;
        if (OffsetToLock < m_data.size())
            *ppbData = m_data.data() + OffsetToLock;
        else
            *ppbData = m_data.data();
        return S_OK;
    }
    HRESULT Unlock() { return S_OK; }
    const void* GetData() const { return m_data.data(); }
    void* GetData() { return m_data.data(); }
    size_t GetSize() const { return m_data.size(); }
private:
    std::vector<BYTE> m_data;
};
typedef IDirect3DVertexBuffer9* LPDIRECT3DVERTEXBUFFER9;

class IDirect3DIndexBuffer9 : public IDirect3DResource9 {
public:
    explicit IDirect3DIndexBuffer9(UINT length = 64) : m_data(length ? length : 64, 0) {}
    HRESULT Lock(UINT OffsetToLock, UINT /*SizeToLock*/, void** ppbData, DWORD /*Flags*/) {
        if (!ppbData) return E_INVALIDARG;
        if (OffsetToLock < m_data.size())
            *ppbData = m_data.data() + OffsetToLock;
        else
            *ppbData = m_data.data();
        return S_OK;
    }
    HRESULT Unlock() { return S_OK; }
    const void* GetData() const { return m_data.data(); }
    void* GetData() { return m_data.data(); }
    size_t GetSize() const { return m_data.size(); }
private:
    std::vector<BYTE> m_data;
};
typedef IDirect3DIndexBuffer9* LPDIRECT3DINDEXBUFFER9;

class IDirect3DVertexShader9 : public IDirect3DResource9 {};
typedef IDirect3DVertexShader9* LPDIRECT3DVERTEXSHADER9;

class IDirect3DPixelShader9 : public IDirect3DResource9 {};
typedef IDirect3DPixelShader9* LPDIRECT3DPIXELSHADER9;

class IDirect3DVertexDeclaration9 : public IDirect3DResource9 {};
typedef IDirect3DVertexDeclaration9* LPDIRECT3DVERTEXDECLARATION9;

class ID3DXBuffer : public IDirect3DResource9 {
public:
    explicit ID3DXBuffer(DWORD size = 64) : m_data(size ? size : 64, 0) {}
    void* GetBufferPointer() { return m_data.data(); }
    DWORD GetBufferSize() const { return (DWORD)m_data.size(); }
private:
    std::vector<BYTE> m_data;
};
typedef ID3DXBuffer* LPD3DXBUFFER;

class ID3DXMesh : public IDirect3DResource9 {
public:
    ID3DXMesh() : m_fvf(D3DFVF_XYZ | D3DFVF_NORMAL), m_numVertices(8), m_numFaces(12) {
        m_vb = new IDirect3DVertexBuffer9(m_numVertices * 24);
        m_ib = new IDirect3DIndexBuffer9(m_numFaces * 6);
    }
    ~ID3DXMesh() override {
        if (m_vb) m_vb->Release();
        if (m_ib) m_ib->Release();
    }
    DWORD GetFVF() const { return m_fvf; }
    DWORD GetNumVertices() const { return m_numVertices; }
    DWORD GetNumFaces() const { return m_numFaces; }
    HRESULT GetVertexBuffer(LPDIRECT3DVERTEXBUFFER9* ppVB) {
        if (!ppVB) return E_INVALIDARG;
        *ppVB = m_vb;
        return S_OK;
    }
    HRESULT GetIndexBuffer(LPDIRECT3DINDEXBUFFER9* ppIB) {
        if (!ppIB) return E_INVALIDARG;
        *ppIB = m_ib;
        return S_OK;
    }
private:
    DWORD m_fvf;
    DWORD m_numVertices;
    DWORD m_numFaces;
    LPDIRECT3DVERTEXBUFFER9 m_vb;
    LPDIRECT3DINDEXBUFFER9  m_ib;
};
typedef ID3DXMesh* LPD3DXMESH;

class IDirect3DDevice9 : public IDirect3DResource9 {
public:
    IDirect3DDevice9();
    ~IDirect3DDevice9() override;

    struct StreamSource {
        IDirect3DVertexBuffer9* vb;
        UINT offset;
        UINT stride;
        StreamSource() : vb(nullptr), offset(0), stride(0) {}
    };

    HRESULT TestCooperativeLevel() { return S_OK; }
    UINT    GetAvailableTextureMem() { return 512 * 1024 * 1024; }
    HRESULT Reset(D3DPRESENT_PARAMETERS*) { return S_OK; }
    HRESULT Present(const RECT*, const RECT*, HWND, const void*) { return S_OK; }
    HRESULT GetDeviceCaps(D3DCAPS9* pCaps) {
        if (!pCaps) return E_INVALIDARG;
        std::memset(pCaps, 0, sizeof(D3DCAPS9));
        pCaps->MaxTextureWidth = 4096;
        pCaps->MaxTextureHeight = 4096;
        pCaps->MaxAnisotropy = 16;
        pCaps->DevCaps = D3DDEVCAPS_HWTRANSFORMANDLIGHT;
        pCaps->VertexProcessingCaps = D3DVTXPCAPS_DIRECTIONALLIGHTS | D3DVTXPCAPS_POSITIONALLIGHTS | D3DVTXPCAPS_TEXGEN;
        pCaps->Caps2 = D3DCAPS2_FULLSCREENGAMMA;
        pCaps->PrimitiveMiscCaps = D3DPMISCCAPS_CLIPTLVERTS;
        pCaps->TextureAddressCaps = D3DPTADDRESSCAPS_BORDER;
        pCaps->VertexShaderVersion = D3DVS_VERSION(2, 0);
        return S_OK;
    }
    HRESULT GetBackBuffer(UINT /*iSwapChain*/, UINT /*BackBuffer*/, DWORD /*Type*/, IDirect3DSurface9** ppBackBuffer) {
        if (!ppBackBuffer) return E_INVALIDARG;
        *ppBackBuffer = new IDirect3DSurface9(m_viewport.Width, m_viewport.Height, nullptr, false, true);
        return S_OK;
    }
    void SetGammaRamp(UINT /*iSwapChain*/, DWORD /*Flags*/, const D3DGAMMARAMP* /*pRamp*/) {}
    HRESULT GetRenderTarget(DWORD, IDirect3DSurface9** ppRenderTarget) {
        if (!ppRenderTarget) return E_INVALIDARG;
        *ppRenderTarget = new IDirect3DSurface9(m_viewport.Width, m_viewport.Height, nullptr, false, true);
        return S_OK;
    }
    HRESULT SetRenderTarget(DWORD RenderTargetIndex, IDirect3DSurface9* pRenderTarget);
    HRESULT GetDepthStencilSurface(IDirect3DSurface9** ppZStencilSurface) {
        if (!ppZStencilSurface) return E_INVALIDARG;
        *ppZStencilSurface = new IDirect3DSurface9(m_viewport.Width, m_viewport.Height, nullptr, true, true);
        return S_OK;
    }
    HRESULT SetDepthStencilSurface(IDirect3DSurface9* pZStencilSurface);
    HRESULT BeginScene();
    HRESULT EndScene();
    HRESULT Clear(DWORD Count, const RECT* pRects, DWORD Flags, D3DCOLOR Color, float Z, DWORD Stencil);
    HRESULT SetTransform(D3DTRANSFORMSTATETYPE State, const D3DMATRIX* pMatrix) {
        if (pMatrix && (unsigned)State < 512) m_transforms[State] = *pMatrix;
        return S_OK;
    }
    HRESULT GetTransform(D3DTRANSFORMSTATETYPE State, D3DMATRIX* pMatrix) {
        if (pMatrix && (unsigned)State < 512) *pMatrix = m_transforms[State];
        return S_OK;
    }
    HRESULT SetViewport(const D3DVIEWPORT9* pViewport);
    HRESULT GetViewport(D3DVIEWPORT9* pViewport) {
        if (pViewport) *pViewport = m_viewport;
        return S_OK;
    }
    HRESULT SetMaterial(const D3DMATERIAL9* pMaterial) {
        if (pMaterial) m_material = *pMaterial;
        return S_OK;
    }
    HRESULT GetMaterial(D3DMATERIAL9* pMaterial) {
        if (pMaterial) *pMaterial = m_material;
        return S_OK;
    }
    HRESULT SetLight(DWORD, const D3DLIGHT9*) { return S_OK; }
    HRESULT GetLight(DWORD, D3DLIGHT9*) { return S_OK; }
    HRESULT LightEnable(DWORD, BOOL) { return S_OK; }
    HRESULT SetRenderState(D3DRENDERSTATETYPE State, DWORD Value);
    HRESULT GetRenderState(D3DRENDERSTATETYPE State, DWORD* pValue);
    HRESULT SetTexture(DWORD Stage, IDirect3DBaseTexture9* pTexture);
    HRESULT GetTexture(DWORD Stage, IDirect3DBaseTexture9** ppTexture);
    HRESULT SetTextureStageState(DWORD Stage, D3DTEXTURESTAGESTATETYPE Type, DWORD Value);
    HRESULT GetTextureStageState(DWORD Stage, D3DTEXTURESTAGESTATETYPE Type, DWORD* pValue);
    HRESULT SetSamplerState(DWORD Stage, D3DSAMPLERSTATETYPE Type, DWORD Value);
    HRESULT GetSamplerState(DWORD Stage, D3DSAMPLERSTATETYPE Type, DWORD* pValue);
    HRESULT SetSoftwareVertexProcessing(BOOL) { return S_OK; }
    HRESULT SetVertexDeclaration(IDirect3DVertexDeclaration9*) { return S_OK; }
    HRESULT GetVertexDeclaration(IDirect3DVertexDeclaration9**) { return S_OK; }
    HRESULT SetFVF(DWORD FVF) { m_fvf = FVF; return S_OK; }
    HRESULT GetFVF(DWORD* pFVF) { if (pFVF) *pFVF = m_fvf; return S_OK; }
    HRESULT CreateVertexShader(const DWORD*, IDirect3DVertexShader9** ppShader) {
        if (!ppShader) return E_INVALIDARG;
        *ppShader = new IDirect3DVertexShader9();
        return S_OK;
    }
    HRESULT SetVertexShader(IDirect3DVertexShader9*) { return S_OK; }
    HRESULT GetVertexShader(IDirect3DVertexShader9**) { return S_OK; }
    HRESULT SetVertexShaderConstantF(UINT, const float*, UINT) { return S_OK; }
    HRESULT CreatePixelShader(const DWORD*, IDirect3DPixelShader9** ppShader) {
        if (!ppShader) return E_INVALIDARG;
        *ppShader = new IDirect3DPixelShader9();
        return S_OK;
    }
    HRESULT SetPixelShader(IDirect3DPixelShader9*) { return S_OK; }
    HRESULT GetPixelShader(IDirect3DPixelShader9**) { return S_OK; }
    HRESULT SetPixelShaderConstantF(UINT, const float*, UINT) { return S_OK; }
    HRESULT CreateTexture(UINT Width, UINT Height, UINT Levels, DWORD Usage, D3DFORMAT Format, D3DPOOL /*Pool*/, IDirect3DTexture9** ppTexture, HANDLE* /*pSharedHandle*/) {
        if (!ppTexture) return E_INVALIDARG;
        *ppTexture = new IDirect3DTexture9(Width, Height, Format, Levels);
        // Render-target textures start white (the value the D3D code clears the
        // shadow map to) so that sampling them before the first render pass is
        // a no-op instead of blackening the scene.
        if (Usage & D3DUSAGE_RENDERTARGET)
            (*ppTexture)->Fill(0xFFFFFFFF);
        return S_OK;
    }
    HRESULT CreateVertexBuffer(UINT Length, DWORD /*Usage*/, DWORD /*FVF*/, D3DPOOL /*Pool*/, IDirect3DVertexBuffer9** ppVertexBuffer, HANDLE* /*pSharedHandle*/) {
        if (!ppVertexBuffer) return E_INVALIDARG;
        *ppVertexBuffer = new IDirect3DVertexBuffer9(Length);
        return S_OK;
    }
    HRESULT CreateIndexBuffer(UINT Length, DWORD /*Usage*/, D3DFORMAT /*Format*/, D3DPOOL /*Pool*/, IDirect3DIndexBuffer9** ppIndexBuffer, HANDLE* /*pSharedHandle*/) {
        if (!ppIndexBuffer) return E_INVALIDARG;
        *ppIndexBuffer = new IDirect3DIndexBuffer9(Length);
        return S_OK;
    }
    HRESULT CreateDepthStencilSurface(UINT Width, UINT Height, D3DFORMAT, D3DMULTISAMPLE_TYPE, DWORD, BOOL, IDirect3DSurface9** ppSurface, HANDLE*) {
        if (!ppSurface) return E_INVALIDARG;
        *ppSurface = new IDirect3DSurface9(Width, Height, nullptr, true, false);
        return S_OK;
    }
    HRESULT SetStreamSource(UINT StreamNumber, IDirect3DVertexBuffer9* pStreamData, UINT OffsetInBytes, UINT Stride) {
        if (StreamNumber < 16) {
            m_streams[StreamNumber].vb = pStreamData;
            m_streams[StreamNumber].offset = OffsetInBytes;
            m_streams[StreamNumber].stride = Stride;
        }
        return S_OK;
    }
    HRESULT SetIndices(IDirect3DIndexBuffer9* pIndexData) {
        m_pIndexBuffer = pIndexData;
        return S_OK;
    }
    HRESULT DrawPrimitive(D3DPRIMITIVETYPE PrimitiveType, UINT StartVertex, UINT PrimitiveCount);
    HRESULT DrawIndexedPrimitive(D3DPRIMITIVETYPE PrimitiveType, INT BaseVertexIndex, UINT MinIndex, UINT NumVertices, UINT StartIndex, UINT PrimitiveCount);
    HRESULT DrawPrimitiveUP(D3DPRIMITIVETYPE PrimitiveType, UINT PrimitiveCount, const void* pVertexStreamZeroData, UINT VertexStreamZeroStride);
    HRESULT DrawIndexedPrimitiveUP(D3DPRIMITIVETYPE PrimitiveType, UINT MinVertexIndex, UINT NumVertexIndices, UINT PrimitiveCount, const void* pIndexData, D3DFORMAT IndexDataFormat, const void* pVertexStreamZeroData, UINT VertexStreamZeroStride);

    DWORD GetSrcBlend() const { return m_srcBlend; }
    DWORD GetDestBlend() const { return m_destBlend; }
    bool  GetZEnable() const { return m_zEnable; }
    bool  GetZWriteEnable() const { return m_zWriteEnable; }
    DWORD GetZFunc() const { return m_zFunc; }
    bool  GetAlphaBlendEnable() const { return m_alphaBlendEnable; }
    bool  GetAlphaTestEnable() const { return m_alphaTestEnable; }
    DWORD GetAlphaRef() const { return m_alphaRef; }
    DWORD GetAlphaFunc() const { return m_alphaFunc; }
    DWORD GetCullMode() const { return m_cullMode; }
    DWORD GetFillMode() const { return m_fillMode; }

private:
    void FillRenderState(RenderState& state) const;

    struct SamplerStageState {
        DWORD addressU;
        DWORD addressV;
        DWORD minFilter;
        DWORD magFilter;
        DWORD mipFilter;
        DWORD borderColor;
    };

    struct TextureStageState {
        DWORD colorOp;
        DWORD colorArg0;
        DWORD colorArg1;
        DWORD colorArg2;
        DWORD alphaOp;
        DWORD alphaArg0;
        DWORD alphaArg1;
        DWORD alphaArg2;
        DWORD resultArg;
    };

    StreamSource           m_streams[16];
    IDirect3DIndexBuffer9* m_pIndexBuffer;
    IDirect3DTexture9*     m_boundTextures[8];
    SamplerStageState      m_samplerStates[8];
    TextureStageState      m_texStageStates[8];
    DWORD                  m_texTransformFlags[8];
    DWORD                  m_texCoordIndex[8];
    DWORD                  m_textureFactor;
    DWORD                  m_lighting;
    DWORD                  m_colorVertex;
    DWORD                  m_ambient;
    bool                   m_fogEnable;
    DWORD                  m_fogColor;
    DWORD                  m_fogVertexMode;
    DWORD                  m_fogTableMode;
    float                  m_fogStart;
    float                  m_fogEnd;
    float                  m_fogDensity;
    bool                   m_rangeFogEnable;
    bool                   m_inRenderTarget;
    D3DMATERIAL9           m_material;
    DWORD                  m_fvf;
    D3DVIEWPORT9           m_viewport;
    DWORD                  m_srcBlend;
    DWORD                  m_destBlend;
    bool                   m_zEnable;
    bool                   m_zWriteEnable;
    DWORD                  m_zFunc;
    bool                   m_alphaBlendEnable;
    bool                   m_alphaTestEnable;
    DWORD                  m_alphaRef;
    DWORD                  m_alphaFunc;
    DWORD                  m_cullMode;
    DWORD                  m_fillMode;
    D3DXMATRIX             m_transforms[512];

    bool ComputeUntexturedStage0Color(const float* current, float* out) const;
};
typedef IDirect3DDevice9* LPDIRECT3DDEVICE9;

class IDirect3D9 : public IDirect3DResource9 {
public:
    UINT GetAdapterCount() { return 1; }
    HRESULT GetAdapterIdentifier(UINT, DWORD, D3DADAPTER_IDENTIFIER9* pIdentifier) {
        if (!pIdentifier) return E_INVALIDARG;
        std::memset(pIdentifier, 0, sizeof(D3DADAPTER_IDENTIFIER9));
        std::strcpy(pIdentifier->Driver, "LinuxD3D9Compat");
        std::strcpy(pIdentifier->Description, "OpenMT2 Linux Graphics Adapter");
        return S_OK;
    }
    UINT GetAdapterModeCount(UINT, D3DFORMAT) { return 1; }
    HRESULT EnumAdapterModes(UINT, D3DFORMAT fmt, UINT, D3DDISPLAYMODE* pMode) {
        if (!pMode) return E_INVALIDARG;
        pMode->Width = 800;
        pMode->Height = 600;
        pMode->RefreshRate = 60;
        pMode->Format = fmt;
        return S_OK;
    }
    HRESULT GetAdapterDisplayMode(UINT, D3DDISPLAYMODE* pMode) {
        if (!pMode) return E_INVALIDARG;
        pMode->Width = 800;
        pMode->Height = 600;
        pMode->RefreshRate = 60;
        pMode->Format = D3DFMT_X8R8G8B8;
        return S_OK;
    }
    HRESULT CheckDeviceType(UINT, D3DDEVTYPE, D3DFORMAT, D3DFORMAT, BOOL) { return S_OK; }
    HRESULT CheckDeviceFormat(UINT, D3DDEVTYPE, D3DFORMAT, DWORD, D3DRESOURCETYPE, D3DFORMAT) { return S_OK; }
    HRESULT CheckDepthStencilMatch(UINT, D3DDEVTYPE, D3DFORMAT, D3DFORMAT, D3DFORMAT) { return S_OK; }
    HRESULT GetDeviceCaps(UINT, D3DDEVTYPE, D3DCAPS9* pCaps) {
        IDirect3DDevice9 dummy;
        return dummy.GetDeviceCaps(pCaps);
    }
    HRESULT CreateDevice(UINT, D3DDEVTYPE, HWND, DWORD, D3DPRESENT_PARAMETERS*, IDirect3DDevice9** ppReturnedDeviceInterface) {
        if (!ppReturnedDeviceInterface) return E_INVALIDARG;
        *ppReturnedDeviceInterface = new IDirect3DDevice9();
        return S_OK;
    }
};
typedef IDirect3D9* LPDIRECT3D9;

inline IDirect3D9* Direct3DCreate9(UINT /*SDKVersion*/) {
    return new IDirect3D9();
}

// D3DX9 Resource Creation Helpers
inline HRESULT D3DXCreateSphere(LPDIRECT3DDEVICE9, float, UINT, UINT, LPD3DXMESH* ppMesh, LPD3DXBUFFER*) {
    if (!ppMesh) return E_INVALIDARG;
    *ppMesh = new ID3DXMesh();
    return S_OK;
}

inline HRESULT D3DXCreateCylinder(LPDIRECT3DDEVICE9, float, float, float, UINT, UINT, LPD3DXMESH* ppMesh, LPD3DXBUFFER*) {
    if (!ppMesh) return E_INVALIDARG;
    *ppMesh = new ID3DXMesh();
    return S_OK;
}

inline HRESULT D3DXCreateTexture(LPDIRECT3DDEVICE9, UINT Width, UINT Height, UINT MipLevels, DWORD /*Usage*/, D3DFORMAT Format, D3DPOOL /*Pool*/, LPDIRECT3DTEXTURE9* ppTexture) {
    if (!ppTexture) return E_INVALIDARG;
    *ppTexture = new IDirect3DTexture9(Width, Height, Format, MipLevels);
    return S_OK;
}

inline HRESULT D3DXGetImageInfoFromFileInMemory(LPCVOID pSrcData, UINT SrcDataSize, D3DXIMAGE_INFO* pSrcInfo) {
    if (!pSrcData || SrcDataSize < 4 || !pSrcInfo) return E_FAIL;
    std::memset(pSrcInfo, 0, sizeof(D3DXIMAGE_INFO));
    pSrcInfo->Width = 64;
    pSrcInfo->Height = 64;
    pSrcInfo->Depth = 1;
    pSrcInfo->MipLevels = 1;
    pSrcInfo->Format = D3DFMT_A8R8G8B8;
    pSrcInfo->ResourceType = D3DRTYPE_TEXTURE;

#if defined(OPENMT2_PLATFORM_LINUX)
    if (Platform_GetImageInfo) {
        uint32_t w = 0, h = 0, fmt = 21;
        if (Platform_GetImageInfo(pSrcData, SrcDataSize, &w, &h, &fmt)) {
            pSrcInfo->Width = w;
            pSrcInfo->Height = h;
            pSrcInfo->Format = (D3DFORMAT)fmt;
        }
    }
#endif
    return S_OK;
}

inline HRESULT D3DXCreateTextureFromFileInMemoryEx(
    LPDIRECT3DDEVICE9, LPCVOID pSrcData, UINT SrcDataSize,
    UINT Width, UINT Height, UINT /*MipLevels*/, DWORD /*Usage*/,
    D3DFORMAT Format, D3DPOOL /*Pool*/, DWORD /*Filter*/, DWORD /*MipFilter*/,
    D3DCOLOR /*ColorKey*/, D3DXIMAGE_INFO* /*pSrcInfo*/, void* /*pPalette*/,
    LPDIRECT3DTEXTURE9* ppTexture)
{
    if (!pSrcData || SrcDataSize == 0 || !ppTexture) return E_FAIL;

#if defined(OPENMT2_PLATFORM_LINUX)
    if (Platform_DecodeImage) {
        uint32_t w = 0, h = 0;
        std::vector<uint8_t> pixels;
        if (Platform_DecodeImage(pSrcData, SrcDataSize, &w, &h, pixels)) {
            IDirect3DTexture9* pTex = new IDirect3DTexture9(w, h, D3DFMT_A8R8G8B8);
            D3DLOCKED_RECT lr;
            if (SUCCEEDED(pTex->LockRect(0, &lr, nullptr, 0))) {
                if (lr.pBits && !pixels.empty()) {
                    std::memcpy(lr.pBits, pixels.data(), std::min(pixels.size(), (size_t)(w * h * 4)));
                }
                pTex->UnlockRect(0);
            }
            *ppTexture = pTex;
            return S_OK;
        }
    }
#endif

    *ppTexture = new IDirect3DTexture9(Width == D3DX_DEFAULT ? 64 : Width, Height == D3DX_DEFAULT ? 64 : Height, Format);
    return S_OK;
}

inline HRESULT D3DXLoadSurfaceFromSurface(
    LPDIRECT3DSURFACE9 pDestSurface, const void*, const RECT*,
    LPDIRECT3DSURFACE9 pSrcSurface, const void*, const RECT*,
    DWORD, D3DCOLOR)
{
    return (pDestSurface && pSrcSurface) ? S_OK : E_FAIL;
}

inline HRESULT D3DXAssembleShaderFromFileA(LPCSTR, const void*, void*, DWORD, LPD3DXBUFFER* ppShader, LPD3DXBUFFER* ppErrorMsgs) {
    if (ppErrorMsgs) *ppErrorMsgs = nullptr;
    if (!ppShader) return E_FAIL;
    *ppShader = new ID3DXBuffer(64);
    return S_OK;
}
#define D3DXAssembleShaderFromFile D3DXAssembleShaderFromFileA

// =============================================================
// 7. DirectInput 8 Stubs
// =============================================================
#define DIRECTINPUT_VERSION 0x0800
#define DISCL_EXCLUSIVE     0x00000001
#define DISCL_NONEXCLUSIVE  0x00000002
#define DISCL_FOREGROUND    0x00000004
#define DISCL_BACKGROUND    0x00000008

struct DIDATAFORMAT { int dummy; };
static const GUID IID_IDirectInput8 = { 0xbf798030, 0x483a, 0x4da2, { 0xaa, 0x99, 0x5d, 0x64, 0xed, 0x36, 0x97, 0x00 } };
static const GUID GUID_SysKeyboard  = { 0x6f1d2b61, 0xd5a0, 0x11cf, { 0xbf, 0xc7, 0x44, 0x45, 0x53, 0x54, 0x00, 0x00 } };
static const DIDATAFORMAT c_dfDIKeyboard = { 0 };

class IDirectInputDevice8A : public IDirect3DResource9 {
public:
    HRESULT SetDataFormat(const DIDATAFORMAT*) { return S_OK; }
    HRESULT SetCooperativeLevel(HWND, DWORD) { return S_OK; }
    HRESULT Acquire() { return S_OK; }
    HRESULT Unacquire() { return S_OK; }
    HRESULT GetDeviceState(DWORD cbData, LPVOID lpvData) {
        if (lpvData && cbData > 0) std::memset(lpvData, 0, cbData);
        return S_OK;
    }
};
typedef IDirectInputDevice8A* LPDIRECTINPUTDEVICE8;

class IDirectInput8A : public IDirect3DResource9 {
public:
    HRESULT CreateDevice(REFGUID, LPDIRECTINPUTDEVICE8* lplpDirectInputDevice, void*) {
        if (!lplpDirectInputDevice) return E_INVALIDARG;
        *lplpDirectInputDevice = new IDirectInputDevice8A();
        return S_OK;
    }
};
typedef IDirectInput8A* LPDIRECTINPUT8;

inline HRESULT DirectInput8Create(HINSTANCE, DWORD, REFIID, LPVOID* ppvOut, void*) {
    if (!ppvOut) return E_INVALIDARG;
    *ppvOut = new IDirectInput8A();
    return S_OK;
}

#endif
