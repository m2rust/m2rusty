// ============================================================================
// test_render_golden.cpp - headless renderer goldens (TA.4)
// ============================================================================
// Pins a set of small, scripted scenes rendered through the real client
// renderer: CGraphicDevice (device startup + state manager) on top of the
// Direct3D 9 compatibility façade, which draws with the fixed-function GL
// pipeline. The target is an EGL surfaceless FBO, so no X server, window or
// GPU is involved.
//
// Each scene is captured with glReadPixels, saved as PNG, and compared against
// the committed golden with a perceptual tolerance. Missing goldens, size
// changes and structural differences fail the test.
//
// Usage:
//   test_render_golden [--update] [--output DIR] [--golden DIR]
//                      [--tolerance N] [--min-similarity PCT] [--list]
//
// --update rewrites tests/golden/render/*.png; review the images and refresh
// golden/render/manifest.sha256 afterwards.

#include <Neoeter/Render/StdAfx.h>

#include <Neoeter/Render/Camera.h>
#include <Neoeter/Render/GrpDevice.h>
#include <Neoeter/Render/StateManager.h>

#include <support/OffscreenGL.h>
#include <support/Png.h>
#include <support/TestAssert.h>

#include <algorithm>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <filesystem>
#include <string>
#include <vector>

#ifndef RENDER_GOLDEN_DIR
#define RENDER_GOLDEN_DIR "tests/golden/render"
#endif
#ifndef RENDER_DEFAULT_OUTPUT_DIR
#define RENDER_DEFAULT_OUTPUT_DIR "render-output"
#endif

namespace
{

constexpr int kWidth = 160;
constexpr int kHeight = 120;
constexpr DWORD kBackground = 0xFF101820;
constexpr float kFogStart = 3.0f;
constexpr float kFogEnd = 12.0f;

struct Vertex
{
    float x;
    float y;
    float z;
    DWORD color;
    float u;
    float v;
};

constexpr DWORD kFvf = D3DFVF_XYZ | D3DFVF_DIFFUSE | D3DFVF_TEX1;

struct Renderer : public CGraphicBase
{
    LPDIRECT3DDEVICE9 Device() const { return ms_lpd3dDevice; }

    void FullViewport()
    {
        D3DVIEWPORT9 viewport = { 0, 0, kWidth, kHeight, 0.0f, 1.0f };
        Device()->SetViewport(&viewport);
    }

    void IdentityWorldView()
    {
        D3DXMATRIX identity;
        D3DXMatrixIdentity(&identity);
        STATEMANAGER.SetTransform(D3DTS_WORLD, &identity);
        STATEMANAGER.SetTransform(D3DTS_VIEW, &identity);
    }

    void IdentityTransforms()
    {
        D3DXMATRIX identity;
        D3DXMatrixIdentity(&identity);
        STATEMANAGER.SetTransform(D3DTS_WORLD, &identity);
        STATEMANAGER.SetTransform(D3DTS_VIEW, &identity);
        STATEMANAGER.SetTransform(D3DTS_PROJECTION, &identity);
    }

    void Draw(D3DPRIMITIVETYPE primitive, UINT primitiveCount,
              const std::vector<Vertex>& vertices)
    {
        Device()->SetFVF(kFvf);
        STATEMANAGER.DrawPrimitiveUP(primitive, primitiveCount, vertices.data(),
                                     static_cast<UINT>(sizeof(Vertex)));
    }
};

// ---------------------------------------------------------------------------
// Deterministic state and geometry helpers
// ---------------------------------------------------------------------------

void ResetState(Renderer& renderer)
{
    STATEMANAGER.SetDefaultState();
    STATEMANAGER.SetRenderState(D3DRS_ZENABLE, FALSE);
    STATEMANAGER.SetRenderState(D3DRS_ZWRITEENABLE, TRUE);
    STATEMANAGER.SetRenderState(D3DRS_ALPHABLENDENABLE, FALSE);
    STATEMANAGER.SetRenderState(D3DRS_ALPHATESTENABLE, FALSE);
    STATEMANAGER.SetRenderState(D3DRS_CULLMODE, D3DCULL_NONE);
    STATEMANAGER.SetRenderState(D3DRS_FOGENABLE, FALSE);
    STATEMANAGER.SetRenderState(D3DRS_LIGHTING, FALSE);
    STATEMANAGER.SetRenderState(D3DRS_TEXTUREFACTOR, 0xFFFFFFFF);
    STATEMANAGER.SetTextureStageState(0, D3DTSS_COLOROP, D3DTOP_MODULATE);
    STATEMANAGER.SetTextureStageState(0, D3DTSS_COLORARG1, D3DTA_TEXTURE);
    STATEMANAGER.SetTextureStageState(0, D3DTSS_COLORARG2, D3DTA_DIFFUSE);
    STATEMANAGER.SetTextureStageState(0, D3DTSS_ALPHAOP, D3DTOP_SELECTARG1);
    STATEMANAGER.SetTextureStageState(0, D3DTSS_ALPHAARG1, D3DTA_TEXTURE);
    STATEMANAGER.SetSamplerState(0, D3DSAMP_MAGFILTER, D3DTEXF_POINT);
    STATEMANAGER.SetSamplerState(0, D3DSAMP_MINFILTER, D3DTEXF_POINT);
    STATEMANAGER.SetTexture(0, nullptr);
    renderer.FullViewport();
    STATEMANAGER.BeginScene();
    renderer.IdentityTransforms();
    renderer.Device()->Clear(0, nullptr, D3DCLEAR_TARGET | D3DCLEAR_ZBUFFER, kBackground,
                             1.0f, 0);
}

void Ortho(Renderer& renderer)
{
    renderer.SetOrtho2D(static_cast<float>(kWidth), static_cast<float>(kHeight), 1.0f);
    renderer.IdentityWorldView();
}

DWORD FloatBits(float value)
{
    DWORD bits = 0;
    static_assert(sizeof(bits) == sizeof(value), "float is not 32 bit");
    std::memcpy(&bits, &value, sizeof(bits));
    return bits;
}

void Perspective(Renderer& renderer, float eyeX, float eyeY, float eyeZ)
{
    renderer.SetEyeCamera(eyeX, eyeY, eyeZ, 0.0f, 0.0f, 0.0f, 0.0f, 1.0f, 0.0f);
    renderer.SetPerspective(60.0f, static_cast<float>(kWidth) / kHeight, 0.5f, 100.0f);
}

std::vector<Vertex> Quad(float left, float top, float right, float bottom, float z,
                         DWORD color, float u0 = 0.0f, float v0 = 0.0f,
                         float u1 = 1.0f, float v1 = 1.0f)
{
    return {
        { left, top, z, color, u0, v0 },
        { right, top, z, color, u1, v0 },
        { right, bottom, z, color, u1, v1 },
        { left, top, z, color, u0, v0 },
        { right, bottom, z, color, u1, v1 },
        { left, bottom, z, color, u0, v1 },
    };
}

std::vector<Vertex> TexturedQuad(float left, float top, float right, float bottom, float z,
                                 DWORD color, float uMax, float vMax)
{
    return Quad(left, top, right, bottom, z, color, 0.0f, 0.0f, uMax, vMax);
}

LPDIRECT3DTEXTURE9 CreateTexture(LPDIRECT3DDEVICE9 device, int width, int height,
                                 const std::vector<DWORD>& pixels)
{
    LPDIRECT3DTEXTURE9 texture = nullptr;
    if (FAILED(D3DXCreateTexture(device, width, height, 1, 0, D3DFMT_A8R8G8B8,
                                 D3DPOOL_MANAGED, &texture)))
        return nullptr;

    D3DLOCKED_RECT locked = {};
    if (FAILED(texture->LockRect(0, &locked, nullptr, 0)))
    {
        texture->Release();
        return nullptr;
    }

    for (int y = 0; y < height; ++y)
    {
        DWORD* row = reinterpret_cast<DWORD*>(static_cast<BYTE*>(locked.pBits) +
                                              static_cast<std::size_t>(y) * locked.Pitch);
        for (int x = 0; x < width; ++x)
            row[x] = pixels[static_cast<std::size_t>(y) * width + x];
    }

    texture->UnlockRect(0);
    return texture;
}

LPDIRECT3DTEXTURE9 MakeCheckerTexture(LPDIRECT3DDEVICE9 device, int size, int cell, DWORD even, DWORD odd)
{
    std::vector<DWORD> pixels(static_cast<std::size_t>(size) * size);
    for (int y = 0; y < size; ++y)
        for (int x = 0; x < size; ++x)
            pixels[static_cast<std::size_t>(y) * size + x] =
                (((x / cell) + (y / cell)) & 1) ? odd : even;
    return CreateTexture(device, size, size, pixels);
}

LPDIRECT3DTEXTURE9 MakeAlphaCheckerTexture(LPDIRECT3DDEVICE9 device, int size, int cell)
{
    std::vector<DWORD> pixels(static_cast<std::size_t>(size) * size);
    for (int y = 0; y < size; ++y)
    {
        for (int x = 0; x < size; ++x)
        {
            const bool opaque = (((x / cell) + (y / cell)) & 1) == 0;
            pixels[static_cast<std::size_t>(y) * size + x] =
                opaque ? D3DCOLOR_ARGB(255, 250, 190, 40) : D3DCOLOR_ARGB(0, 30, 60, 200);
        }
    }
    return CreateTexture(device, size, size, pixels);
}

void BindTexture(LPDIRECT3DTEXTURE9 texture)
{
    STATEMANAGER.SetTexture(0, texture);
    STATEMANAGER.SetTextureStageState(0, D3DTSS_COLOROP, D3DTOP_MODULATE);
    STATEMANAGER.SetTextureStageState(0, D3DTSS_COLORARG1, D3DTA_TEXTURE);
    STATEMANAGER.SetTextureStageState(0, D3DTSS_COLORARG2, D3DTA_DIFFUSE);
}

void UnbindTexture(LPDIRECT3DTEXTURE9 texture)
{
    STATEMANAGER.SetTexture(0, nullptr);
    if (texture)
        texture->Release();
}

// ---------------------------------------------------------------------------
// Scenes
// ---------------------------------------------------------------------------

void SceneClear(Renderer&)
{
    // Deliberately empty: pins the clear colour and the empty target.
}

void SceneOrthoTriangle(Renderer& renderer)
{
    Ortho(renderer);
    std::vector<Vertex> vertices = {
        { 80.0f, 10.0f, -0.5f, D3DCOLOR_ARGB(255, 220, 60, 60), 0.0f, 0.0f },
        { 150.0f, 110.0f, -0.5f, D3DCOLOR_ARGB(255, 60, 220, 90), 0.0f, 0.0f },
        { 10.0f, 110.0f, -0.5f, D3DCOLOR_ARGB(255, 70, 90, 230), 0.0f, 0.0f },
    };
    renderer.Draw(D3DPT_TRIANGLELIST, 1, vertices);
}

void SceneOrthoBlend(Renderer& renderer)
{
    Ortho(renderer);
    STATEMANAGER.SetRenderState(D3DRS_ALPHABLENDENABLE, TRUE);
    STATEMANAGER.SetRenderState(D3DRS_SRCBLEND, D3DBLEND_SRCALPHA);
    STATEMANAGER.SetRenderState(D3DRS_DESTBLEND, D3DBLEND_INVSRCALPHA);

    renderer.Draw(D3DPT_TRIANGLELIST, 2,
                  Quad(25.0f, 25.0f, 115.0f, 95.0f, -0.5f,
                       D3DCOLOR_ARGB(255, 40, 80, 220)));
    renderer.Draw(D3DPT_TRIANGLELIST, 2,
                  Quad(70.0f, 45.0f, 140.0f, 110.0f, -0.5f,
                       D3DCOLOR_ARGB(140, 230, 60, 50)));
}

void SceneCheckerNearest(Renderer& renderer)
{
    Ortho(renderer);
    LPDIRECT3DTEXTURE9 texture = MakeCheckerTexture(
        renderer.Device(), 8, 2, D3DCOLOR_ARGB(255, 235, 235, 235), D3DCOLOR_ARGB(255, 40, 40, 40));
    TEST_ASSERT(texture != nullptr);
    BindTexture(texture);
    STATEMANAGER.SetSamplerState(0, D3DSAMP_MAGFILTER, D3DTEXF_POINT);
    STATEMANAGER.SetSamplerState(0, D3DSAMP_MINFILTER, D3DTEXF_POINT);
    renderer.Draw(D3DPT_TRIANGLELIST, 2,
                  TexturedQuad(20.0f, 20.0f, 140.0f, 100.0f, -0.5f,
                               D3DCOLOR_ARGB(255, 255, 255, 255), 1.0f, 1.0f));
    UnbindTexture(texture);
}

void SceneTextureModulate(Renderer& renderer)
{
    Ortho(renderer);
    LPDIRECT3DTEXTURE9 texture = MakeCheckerTexture(
        renderer.Device(), 8, 1, D3DCOLOR_ARGB(255, 240, 120, 40), D3DCOLOR_ARGB(255, 40, 120, 240));
    TEST_ASSERT(texture != nullptr);
    BindTexture(texture);
    STATEMANAGER.SetTextureStageState(0, D3DTSS_COLORARG2, D3DTA_DIFFUSE);
    renderer.Draw(D3DPT_TRIANGLELIST, 2,
                  TexturedQuad(20.0f, 20.0f, 140.0f, 100.0f, -0.5f,
                               D3DCOLOR_ARGB(255, 128, 255, 128), 1.0f, 1.0f));
    UnbindTexture(texture);
}

void SceneTextureWrap(Renderer& renderer)
{
    Ortho(renderer);
    LPDIRECT3DTEXTURE9 texture = MakeCheckerTexture(
        renderer.Device(), 8, 2, D3DCOLOR_ARGB(255, 250, 250, 250), D3DCOLOR_ARGB(255, 30, 90, 200));
    TEST_ASSERT(texture != nullptr);
    BindTexture(texture);
    STATEMANAGER.SetSamplerState(0, D3DSAMP_ADDRESSU, D3DTADDRESS_WRAP);
    STATEMANAGER.SetSamplerState(0, D3DSAMP_ADDRESSV, D3DTADDRESS_WRAP);
    renderer.Draw(D3DPT_TRIANGLELIST, 2,
                  TexturedQuad(10.0f, 15.0f, 150.0f, 105.0f, -0.5f,
                               D3DCOLOR_ARGB(255, 255, 255, 255), 4.0f, 3.0f));
    UnbindTexture(texture);
}

void SceneDepthTest(Renderer& renderer)
{
    Perspective(renderer, 0.0f, 0.0f, 3.0f);
    STATEMANAGER.SetRenderState(D3DRS_ZENABLE, TRUE);
    STATEMANAGER.SetRenderState(D3DRS_ZWRITEENABLE, TRUE);
    STATEMANAGER.SetRenderState(D3DRS_ZFUNC, D3DCMP_LESS);

    // Near quad first, far quad second. Depth testing must keep the near
    // quad; a broken depth buffer lets the far red quad overwrite it.
    renderer.Draw(D3DPT_TRIANGLELIST, 2,
                  Quad(-1.5f, -1.5f, 1.5f, 1.5f, -2.0f,
                       D3DCOLOR_ARGB(255, 60, 210, 100)));
    renderer.Draw(D3DPT_TRIANGLELIST, 2,
                  Quad(-4.0f, -4.0f, 4.0f, 4.0f, -5.0f,
                       D3DCOLOR_ARGB(255, 220, 60, 50)));
}

void ScenePerspectiveCube(Renderer& renderer)
{
    Perspective(renderer, 0.0f, 0.0f, 3.2f);
    STATEMANAGER.SetRenderState(D3DRS_ZENABLE, TRUE);
    STATEMANAGER.SetRenderState(D3DRS_ZWRITEENABLE, TRUE);
    STATEMANAGER.SetRenderState(D3DRS_ZFUNC, D3DCMP_LESSEQUAL);

    D3DXMATRIX world;
    D3DXMatrixRotationYawPitchRoll(&world, D3DXToRadian(32.0f), D3DXToRadian(21.0f), 0.0f);
    STATEMANAGER.SetTransform(D3DTS_WORLD, &world);

    const float s = 0.7f;
    const float corners[8][3] = {
        { -s, -s, -s }, { s, -s, -s }, { s, s, -s }, { -s, s, -s },
        { -s, -s, s }, { s, -s, s }, { s, s, s }, { -s, s, s },
    };
    const int faces[6][4] = {
        { 0, 3, 2, 1 }, // back
        { 4, 5, 6, 7 }, // front
        { 0, 1, 5, 4 }, // bottom
        { 3, 7, 6, 2 }, // top
        { 0, 4, 7, 3 }, // left
        { 1, 2, 6, 5 }, // right
    };
    const DWORD faceColors[6] = {
        D3DCOLOR_ARGB(255, 210, 70, 70),
        D3DCOLOR_ARGB(255, 70, 200, 90),
        D3DCOLOR_ARGB(255, 70, 90, 210),
        D3DCOLOR_ARGB(255, 230, 200, 60),
        D3DCOLOR_ARGB(255, 190, 70, 210),
        D3DCOLOR_ARGB(255, 70, 200, 210),
    };

    std::vector<Vertex> vertices;
    vertices.reserve(36);
    for (int face = 0; face < 6; ++face)
    {
        const int* indices = faces[face];
        const DWORD color = faceColors[face];
        const int order[6] = { 0, 1, 2, 0, 2, 3 };
        for (int i = 0; i < 6; ++i)
        {
            const float* p = corners[indices[order[i]]];
            vertices.push_back({ p[0], p[1], p[2], color, 0.0f, 0.0f });
        }
    }
    renderer.Draw(D3DPT_TRIANGLELIST, 12, vertices);
}

void SceneCullBackface(Renderer& renderer)
{
    Ortho(renderer);
    STATEMANAGER.SetRenderState(D3DRS_CULLMODE, D3DCULL_CW);

    // Clockwise in screen space: culled by D3DCULL_CW.
    std::vector<Vertex> culled = {
        { 0.0f, 0.0f, -0.5f, D3DCOLOR_ARGB(255, 220, 50, 220), 0.0f, 0.0f },
        { static_cast<float>(kWidth), 0.0f, -0.5f, D3DCOLOR_ARGB(255, 220, 50, 220), 0.0f, 0.0f },
        { 0.0f, static_cast<float>(kHeight), -0.5f, D3DCOLOR_ARGB(255, 220, 50, 220), 0.0f, 0.0f },
    };
    renderer.Draw(D3DPT_TRIANGLELIST, 1, culled);

    // Counter-clockwise: survives.
    std::vector<Vertex> visible = {
        { 40.0f, 90.0f, -0.5f, D3DCOLOR_ARGB(255, 60, 220, 230), 0.0f, 0.0f },
        { 120.0f, 90.0f, -0.5f, D3DCOLOR_ARGB(255, 60, 220, 230), 0.0f, 0.0f },
        { 80.0f, 30.0f, -0.5f, D3DCOLOR_ARGB(255, 60, 220, 230), 0.0f, 0.0f },
    };
    renderer.Draw(D3DPT_TRIANGLELIST, 1, visible);
}

void SceneFogLinear(Renderer& renderer)
{
    Perspective(renderer, 0.0f, 0.0f, 3.0f);
    STATEMANAGER.SetRenderState(D3DRS_ZENABLE, TRUE);
    STATEMANAGER.SetRenderState(D3DRS_FOGENABLE, TRUE);
    STATEMANAGER.SetRenderState(D3DRS_FOGCOLOR, D3DCOLOR_ARGB(255, 235, 240, 255));
    STATEMANAGER.SetRenderState(D3DRS_FOGVERTEXMODE, D3DFOG_LINEAR);
    STATEMANAGER.SetRenderState(D3DRS_FOGSTART, FloatBits(kFogStart));
    STATEMANAGER.SetRenderState(D3DRS_FOGEND, FloatBits(kFogEnd));

    std::vector<Vertex> vertices = {
        { -4.0f, -3.0f, -3.0f, D3DCOLOR_ARGB(255, 30, 50, 160), 0.0f, 0.0f },
        { 4.0f, -3.0f, -3.0f, D3DCOLOR_ARGB(255, 30, 50, 160), 0.0f, 0.0f },
        { 4.0f, 3.0f, -3.0f, D3DCOLOR_ARGB(255, 30, 50, 160), 0.0f, 0.0f },
        { -4.0f, -3.0f, -3.0f, D3DCOLOR_ARGB(255, 30, 50, 160), 0.0f, 0.0f },
        { 4.0f, 3.0f, -3.0f, D3DCOLOR_ARGB(255, 30, 50, 160), 0.0f, 0.0f },
        { -4.0f, 3.0f, -3.0f, D3DCOLOR_ARGB(255, 30, 50, 160), 0.0f, 0.0f },
        { -9.0f, -9.0f, -14.0f, D3DCOLOR_ARGB(255, 30, 50, 160), 0.0f, 0.0f },
        { 9.0f, -9.0f, -14.0f, D3DCOLOR_ARGB(255, 30, 50, 160), 0.0f, 0.0f },
        { 9.0f, 9.0f, -14.0f, D3DCOLOR_ARGB(255, 30, 50, 160), 0.0f, 0.0f },
        { -9.0f, -9.0f, -14.0f, D3DCOLOR_ARGB(255, 30, 50, 160), 0.0f, 0.0f },
        { 9.0f, 9.0f, -14.0f, D3DCOLOR_ARGB(255, 30, 50, 160), 0.0f, 0.0f },
        { -9.0f, 9.0f, -14.0f, D3DCOLOR_ARGB(255, 30, 50, 160), 0.0f, 0.0f },
    };
    renderer.Draw(D3DPT_TRIANGLELIST, 4, vertices);
}

void SceneViewport(Renderer& renderer)
{
    Ortho(renderer);
    D3DVIEWPORT9 viewport = { 40, 30, 80, 60, 0.0f, 1.0f };
    renderer.Device()->SetViewport(&viewport);
    renderer.Draw(D3DPT_TRIANGLELIST, 2,
                  Quad(0.0f, 0.0f, static_cast<float>(kWidth),
                       static_cast<float>(kHeight), -0.5f,
                       D3DCOLOR_ARGB(255, 220, 140, 60)));
}

void SceneAlphaTest(Renderer& renderer)
{
    Ortho(renderer);
    LPDIRECT3DTEXTURE9 texture = MakeAlphaCheckerTexture(renderer.Device(), 8, 4);
    TEST_ASSERT(texture != nullptr);
    BindTexture(texture);
    STATEMANAGER.SetSamplerState(0, D3DSAMP_MAGFILTER, D3DTEXF_POINT);
    STATEMANAGER.SetSamplerState(0, D3DSAMP_MINFILTER, D3DTEXF_POINT);
    STATEMANAGER.SetRenderState(D3DRS_ALPHATESTENABLE, TRUE);
    STATEMANAGER.SetRenderState(D3DRS_ALPHAFUNC, D3DCMP_GREATER);
    STATEMANAGER.SetRenderState(D3DRS_ALPHAREF, 128);
    renderer.Draw(D3DPT_TRIANGLELIST, 2,
                  TexturedQuad(20.0f, 20.0f, 140.0f, 100.0f, -0.5f,
                               D3DCOLOR_ARGB(255, 255, 255, 255), 1.0f, 1.0f));
    UnbindTexture(texture);
}

struct Scene
{
    const char* name;
    void (*draw)(Renderer&);
};

const Scene kScenes[] = {
    { "clear", SceneClear },
    { "ortho_triangle", SceneOrthoTriangle },
    { "ortho_blend", SceneOrthoBlend },
    { "checker_nearest", SceneCheckerNearest },
    { "texture_modulate", SceneTextureModulate },
    { "texture_wrap", SceneTextureWrap },
    { "depth_test", SceneDepthTest },
    { "perspective_cube", ScenePerspectiveCube },
    { "cull_backface", SceneCullBackface },
    { "fog_linear", SceneFogLinear },
    { "viewport", SceneViewport },
    { "alpha_test", SceneAlphaTest },
};

// ---------------------------------------------------------------------------
// Runner
// ---------------------------------------------------------------------------

struct Options
{
    bool update = false;
    bool list = false;
    std::string outputDir = RENDER_DEFAULT_OUTPUT_DIR;
    std::string goldenDir = RENDER_GOLDEN_DIR;
    int tolerance = 2;
    double minSimilarity = 99.5;
};

void PrintUsage(const char* program)
{
    std::printf(
        "usage: %s [--update] [--output DIR] [--golden DIR] [--tolerance N]\n"
        "          [--min-similarity PCT] [--list]\n",
        program);
}

bool ParseInt(const char* text, int* value)
{
    char* end = nullptr;
    const long parsed = std::strtol(text, &end, 10);
    if (!end || *end != '\0')
        return false;
    *value = static_cast<int>(parsed);
    return true;
}

bool ParseDouble(const char* text, double* value)
{
    char* end = nullptr;
    const double parsed = std::strtod(text, &end);
    if (!end || *end != '\0')
        return false;
    *value = parsed;
    return true;
}

bool ParseOptions(int argc, char** argv, Options* options)
{
    for (int i = 1; i < argc; ++i)
    {
        const std::string argument = argv[i];
        if (argument == "--update")
            options->update = true;
        else if (argument == "--list")
            options->list = true;
        else if (argument == "--output" && i + 1 < argc)
            options->outputDir = argv[++i];
        else if (argument == "--golden" && i + 1 < argc)
            options->goldenDir = argv[++i];
        else if (argument == "--tolerance" && i + 1 < argc)
        {
            if (!ParseInt(argv[++i], &options->tolerance))
                return false;
        }
        else if (argument == "--min-similarity" && i + 1 < argc)
        {
            if (!ParseDouble(argv[++i], &options->minSimilarity))
                return false;
        }
        else
        {
            std::fprintf(stderr, "unknown argument: %s\n", argument.c_str());
            return false;
        }
    }
    return true;
}

} // namespace

int main(int argc, char** argv)
{
    Options options;
    if (!ParseOptions(argc, argv, &options))
    {
        PrintUsage(argv[0]);
        return 2;
    }

    if (options.list)
    {
        for (const Scene& scene : kScenes)
            std::printf("%s\n", scene.name);
        return 0;
    }

    std::error_code errorCode;
    std::filesystem::create_directories(options.outputDir, errorCode);
    if (errorCode)
    {
        std::fprintf(stderr, "[FAIL] cannot create output directory '%s': %s\n",
                     options.outputDir.c_str(), errorCode.message().c_str());
        return 1;
    }

    render::OffscreenGL target;
    std::string error;
    if (!target.Create(kWidth, kHeight, &error))
    {
        std::fprintf(stderr, "[FAIL] cannot create the offscreen GL target: %s\n",
                     error.c_str());
        return 1;
    }

    std::printf("[info] offscreen target: %dx%d\n", target.Width(), target.Height());
    std::printf("[info] renderer: %s\n", target.Renderer().c_str());
    std::printf("[info] output: %s\n", options.outputDir.c_str());
    std::printf("[info] goldens: %s\n", options.goldenDir.c_str());

    CGraphicDevice device;
    device.InitBackBufferCount(1);
    const int createResult = device.Create(reinterpret_cast<HWND>(1), kWidth, kHeight, true, 32, 0);
    if (createResult != CGraphicDevice::CREATE_OK)
    {
        std::fprintf(stderr, "[FAIL] CGraphicDevice::Create returned 0x%x\n", createResult);
        device.Destroy();
        target.Destroy();
        return 1;
    }

    Renderer renderer;
    ResetState(renderer);

    std::size_t passed = 0;
    std::size_t failed = 0;

    for (const Scene& scene : kScenes)
    {
        ResetState(renderer);
        scene.draw(renderer);
        STATEMANAGER.EndScene();

        render::Image image;
        if (!target.Capture(&image, &error))
        {
            std::fprintf(stderr, "[FAIL] %s: %s\n", scene.name, error.c_str());
            ++failed;
            continue;
        }

        const std::string outputPath =
            (std::filesystem::path(options.outputDir) / (std::string(scene.name) + ".png")).string();
        if (!render::WritePng(outputPath, image, &error))
        {
            std::fprintf(stderr, "[FAIL] %s: %s\n", scene.name, error.c_str());
            ++failed;
            continue;
        }

        const std::string hash = render::FormatHash(render::PixelHash(image));

        if (options.update)
        {
            std::filesystem::create_directories(options.goldenDir, errorCode);
            errorCode.clear();
            const std::string goldenPath =
                (std::filesystem::path(options.goldenDir) / (std::string(scene.name) + ".png")).string();
            std::filesystem::copy_file(outputPath, goldenPath,
                                       std::filesystem::copy_options::overwrite_existing,
                                       errorCode);
            if (errorCode)
            {
                std::fprintf(stderr, "[FAIL] %s: cannot write golden '%s': %s\n", scene.name,
                             goldenPath.c_str(), errorCode.message().c_str());
                ++failed;
                continue;
            }
            std::printf("[update] %-20s %s  raw=%s\n", scene.name, outputPath.c_str(), hash.c_str());
            ++passed;
            continue;
        }

        const std::string goldenPath =
            (std::filesystem::path(options.goldenDir) / (std::string(scene.name) + ".png")).string();
        render::Image golden;
        if (!render::LoadPng(goldenPath, &golden, &error))
        {
            std::fprintf(stderr,
                         "[FAIL] %s: missing golden '%s' (%s); run with --update\n",
                         scene.name, goldenPath.c_str(), error.c_str());
            ++failed;
            continue;
        }

        const render::DiffStats stats = render::CompareImages(golden, image, options.tolerance);
        const double similarityPercent = stats.similarity * 100.0;
        const bool ok = stats.totalPixels > 0 && similarityPercent >= options.minSimilarity;
        std::printf("[%s] %-20s similarity=%6.2f%% max_delta=%3d mae=%6.3f pixels=%zu raw=%s\n",
                    ok ? "PASS" : "FAIL", scene.name, similarityPercent, stats.maxDelta,
                    stats.meanAbsError, stats.totalPixels, hash.c_str());
        if (ok)
            ++passed;
        else
            ++failed;
    }

    device.Destroy();
    target.Destroy();

    std::printf("[summary] %zu/%zu scenes passed (tolerance=%d, min similarity=%.2f%%)\n",
                passed, passed + failed, options.tolerance, options.minSimilarity);
    return failed == 0 ? 0 : 1;
}
