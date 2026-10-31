#include <support/TestAssert.h>

#include <cstring>

#include <Platform/DirectXMathCompat.h>
#include <Platform/RenderBackend.h>

// T1.7 — renderer seam contract.
//
// The façade must describe state in value snapshots that carry no GL types, and
// a draw without vertex data must stay a no-op (it must not reach the backend,
// so this test needs no GPU or GL context).

int main()
{
    TEST_SECTION("RenderHandle is opaque");

    RenderHandle none{0, 0};
    TEST_ASSERT(!none.IsValid());

    RenderHandle texture{3, 1};
    TEST_ASSERT(texture.IsValid());
    TEST_ASSERT(texture.id == 3);
    TEST_ASSERT(texture.generation == 1);
    TEST_PASS("invalid handle is id 0, valid handles carry a generation");

    TEST_SECTION("RenderState is a value snapshot");

    RenderState state{};
    TEST_ASSERT(state.fvf == 0);
    TEST_ASSERT(state.viewportWidth == 0);

    state.fvf = D3DFVF_XYZ | D3DFVF_DIFFUSE;
    state.zEnable = true;
    state.stages[0].colorOp = D3DTOP_MODULATE;
    state.textures[1] = RenderHandle{7, 2};

    RenderState copy = state;
    TEST_ASSERT(copy.fvf == state.fvf);
    TEST_ASSERT(copy.zEnable);
    TEST_ASSERT(copy.stages[0].colorOp == D3DTOP_MODULATE);
    TEST_ASSERT(copy.textures[1].id == 7);
    TEST_ASSERT(copy.textures[1].generation == 2);
    TEST_PASS("snapshot copies every stage, sampler and handle");

    TEST_SECTION("DrawCall carries its state");

    DrawCall call{};
    call.state.zFunc = D3DCMP_LESS;
    call.primitiveType = D3DPT_TRIANGLELIST;
    call.vertexCount = 3;
    TEST_ASSERT(call.state.zFunc == D3DCMP_LESS);
    TEST_ASSERT(call.vertexData == nullptr);
    TEST_PASS("draw and its snapshot travel together");

    TEST_SECTION("D3D9 facade tracks state without GL");

    IDirect3DDevice9 device;
    device.SetFVF(D3DFVF_XYZ | D3DFVF_TEX1);
    device.SetRenderState(D3DRS_CULLMODE, D3DCULL_NONE);
    device.SetRenderState(D3DRS_ALPHABLENDENABLE, TRUE);
    device.SetRenderState(D3DRS_DESTBLEND, D3DBLEND_ONE);
    device.SetSamplerState(0, D3DSAMP_ADDRESSU, D3DTADDRESS_CLAMP);
    device.SetTextureStageState(0, D3DTSS_COLOROP, D3DTOP_SELECTARG1);

    DWORD value = 0;
    TEST_ASSERT(device.GetRenderState(D3DRS_CULLMODE, &value) == S_OK);
    TEST_ASSERT(value == D3DCULL_NONE);
    TEST_ASSERT(device.GetRenderState(D3DRS_ALPHABLENDENABLE, &value) == S_OK);
    TEST_ASSERT(value == 1);
    TEST_ASSERT(device.GetSamplerState(0, D3DSAMP_ADDRESSU, &value) == S_OK);
    TEST_ASSERT(value == D3DTADDRESS_CLAMP);
    TEST_ASSERT(device.GetTextureStageState(0, D3DTSS_COLOROP, &value) == S_OK);
    TEST_ASSERT(value == D3DTOP_SELECTARG1);
    TEST_PASS("state round-trips through the facade");

    TEST_SECTION("empty draws stay no-ops");

    TEST_ASSERT(device.DrawPrimitive(D3DPT_TRIANGLELIST, 0, 1) == S_OK);
    TEST_ASSERT(device.DrawIndexedPrimitive(D3DPT_TRIANGLELIST, 0, 0, 0, 0, 1) == S_OK);
    TEST_ASSERT(device.DrawPrimitiveUP(D3DPT_TRIANGLELIST, 1, nullptr, 0) == S_OK);
    TEST_ASSERT(device.DrawIndexedPrimitiveUP(D3DPT_TRIANGLELIST, 0, 0, 1, nullptr,
                                              D3DFMT_INDEX16, nullptr, 0) == S_OK);
    TEST_PASS("draws without vertex data return S_OK");

    TEST_SECTION("textures keep their D3D9 description");

    {
        IDirect3DTexture9 texture(4, 8, D3DFMT_A8R8G8B8);
        TEST_ASSERT(texture.GetWidth() == 4);
        TEST_ASSERT(texture.GetHeight() == 8);
        TEST_ASSERT(texture.GetFormat() == D3DFMT_A8R8G8B8);
        TEST_ASSERT(texture.HasMipMaps() == false);
        TEST_PASS("texture metadata stays on the facade side");
    }

    return 0;
}
