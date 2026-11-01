# m2-render

Fixed-function (Direct3D 9 style) rendering for the Bevy port:
`FixedFunctionMaterial` + `M2RenderPlugin` (WESL shader `src/fixed_function.wesl`),
`FixedFunctionProjection`, and a golden conformance harness against the C++
renderer's PNGs. No C/C++ and no `m2-script`; wasm/mobile portable.

## What the C++ renderer actually does

Sources: `neoeter/include/Platform/RenderBackend.h` (the `RenderState` snapshot),
`neoeter/src/platform/base/backends/gl/GLRenderBackend.cpp` (`ApplyState`,
`SetStageColorCombine`, `SetStageAlphaCombine`, `Draw`),
`neoeter/src/platform/base/Direct3DDevice9Compat.cpp` (`FillRenderState`,
`DrawPrimitive`, `ComputeUntexturedStage0Color`), `neoeter/src/render/StateManager.cpp`
(`SetDefaultState`). The GL backend is the reference for the goldens.

| State | GL backend (C++) | Here |
|---|---|---|
| Texture stages | only stages 0 and 1 applied (`kAppliedStages`); stages 2-7 carried but ignored | 2 stages, `TextureStage` |
| Colour ops | DISABLE, SELECTARG1/2, ADD, ADDSIGNED, SUBTRACT, MODULATE2X/4X, BLENDTEXTUREALPHA, BLENDCURRENTALPHA, BLENDFACTORALPHA; the four `MODULATE*_ADD*` ops approximated as ADD; everything else (incl. BLENDDIFFUSEALPHA, ADDSIGNED2X, ADDSMOOTH) falls back to MODULATE | all of those with D3D semantics, plus ADDSIGNED2X, ADDSMOOTH, BLENDDIFFUSEALPHA, BLENDTEXTUREALPHAPM. Not: the four `MODULATE*_ADD*` specular ops, BUMPENVMAP, DOTPRODUCT3, MULTIPLYADD, LERP |
| Alpha ops | DISABLE, SELECTARG1/2, ADD and ADDSIGNED (both plain ADD, a C++ quirk), SUBTRACT, default MODULATE; 2x/4x ignored | same op set as colour ops, D3D semantics |
| Args | TEXTURE, DIFFUSE, CURRENT, TFACTOR; SPECULAR mapped to primary colour; TEMP to previous; CONSTANT to TFACTOR; COMPLEMENT, ALPHAREPLICATE (colour only) | same (`TextureArg`) |
| Texture factor | `D3DRS_TEXTUREFACTOR` as `GL_TEXTURE_ENV_COLOR` | `texture_factor` |
| Stage without texture | stage disabled, output = primary colour (alpha too) | same: a stage with no texture is skipped. Flat TFACTOR draws need a 1x1 white texture |
| TCI / texture matrix | CAMERASPACEPOSITION via `GL_EYE_LINEAR`, per-stage texture matrices | **not supported** (terrain/shadow/water, later phases). Both stages use UV0 |
| Sampler min/mag | POINT or LINEAR; with mips POINT min gives NEAREST_MIPMAP_NEAREST else LINEAR_MIPMAP_LINEAR (mip filter ignored) | `Filter` min/mag + explicit `MipFilter` (None/Point/Linear) |
| Address | WRAP, CLAMP, BORDER (`GL_CLAMP_TO_BORDER`); MIRROR/MIRRORONCE fall back to REPEAT | Wrap, Clamp, **Mirror** (extension). BORDER not supported (needs a wgpu feature absent on WebGPU/GLES) |
| Anisotropy | not applied | not supported |
| Alpha test | func + ref/255 (`glAlphaFunc`) | func + 8-bit ref, compared as integers (D3D semantics) |
| Blend | src/dst factors ZERO, ONE, SRC/INVSRC COLOR/ALPHA, DEST/INVDEST COLOR/ALPHA; others become ONE; same factors for alpha; BLENDOP ignored (always ADD) | the same 10 factors + SRCALPHASAT, plus BLENDOP Add/Sub/RevSub/Min/Max (extensions). Same factors for colour and alpha |
| Cull | `D3DCULL_CW` culls clockwise-on-screen triangles | `CullMode::{None, Cw, Ccw}`, judged on the final image; default `Cw` (as `SetDefaultState`) |
| Depth | test, write, func (all 8); write is off when the test is off | same (`DepthState`); funcs flipped for Bevy's reverse-Z |
| Fog | enabled only with a perspective projection; linear (start/end, off when end <= start), EXP, EXP2; GL fog on eye-space depth per fragment; colour alpha ignored; RANGEFOG ignored | `Fog::{linear, exp, exp2}`, per fragment on view-axis depth, any projection (the caller enables it) |
| Vertex colour | `D3DFVF_DIFFUSE` BGRA used as primary colour, interpolated in gamma space; none -> white, or `Material.Diffuse` when `D3DRS_LIGHTING` is on | `ATTRIBUTE_COLOR` Float32x4 gamma-space; absent -> `FixedFunctionMaterial::diffuse` (white default) |
| Specular | `D3DFVF_SPECULAR` skipped, `SPECULARENABLE` ignored | not supported |
| Lighting | **not implemented**: `SetLight`/`LightEnable` are no-ops in the compat layer, `GL_LIGHTING` is never enabled, so `GrpLightManager` lights, material ambient/emissive and `D3DRS_AMBIENT` have no effect on Linux | default off (= reference). `.lit(Lighting)` adds the D3D9 equation for one directional light + ambient, per vertex, material colours (extension, needs normals) |
| Fill mode | solid/wire/point | not supported |
| Colour write mask, stencil, clip planes | `COLORWRITEENABLE`, stencil, clip planes set but ignored | not supported |
| Render targets | clip-space Y flip while rendering to a texture | not needed: Bevy cameras render to images with the normal convention |
| Viewport | `glViewport(X, Y, ...)` with D3D's top-left `Y` used as GL's bottom-left (C++ bug, invisible in the golden) | `Camera::viewport`, top-left origin (correct) |
| 2D projection | `SetOrtho2D` is replaced by `glOrtho(-0.5, w-0.5, h-0.5, -0.5, -1000, 1000)`: pixel centres on integer coordinates | `FixedFunctionProjection::orthographic_2d(w, h)` |
| 3D projection | D3D `[0,1]` depth matrix loaded unchanged into GL `[-1,1]` clip: the effective near plane is about half the requested one | `FixedFunctionProjection::perspective`: exact near plane (deviation, invisible in the goldens) |

## Colour space

The original has no sRGB conversion anywhere: values are blended and stored as
written. Therefore:

- render to an `Rgba8Unorm` (non-sRGB) image: Bevy then uses it as the main
  texture and writes/blends shader output unchanged;
- cameras use `gamma_space_camera()` (Tonemapping off, DebandDither off, Msaa off)
  and `clear_color()` (a raw `LinearRgba` carrying gamma values);
- textures must be non-sRGB formats (`Rgba8Unorm`, BC formats without the `Srgb`
  suffix) and vertex colours are `byte / 255`, never converted;
- a window surface is always sRGB in Bevy (`ExtractedWindow` adds an sRGB view).
  To show the result in a window render into a `Rgba8Unorm` image and present it
  with a `Camera2d` + `CompositingSpace::Srgb` sprite (Bevy undoes the encode);
  `examples/golden_view.rs` does exactly this.

## Mesh producer contract (m2-assets)

Bevy `Mesh` attributes, triangle lists:

| Attribute | Format | Notes |
|---|---|---|
| `ATTRIBUTE_POSITION` | Float32x3 | required |
| `ATTRIBUTE_NORMAL` | Float32x3 | only for `.lit()` |
| `ATTRIBUTE_UV_0` | Float32x2 | both texture stages (D3D v-down convention, no flip) |
| `ATTRIBUTE_COLOR` | Float32x4 | gamma-space `D3DFVF_DIFFUSE / 255`; omit to use `material.diffuse` |
| `ATTRIBUTE_JOINT_INDEX` / `ATTRIBUTE_JOINT_WEIGHT` | Uint16x4 / Float32x4 | with Bevy's `SkinnedMesh` component; GPU skinning through Bevy's skin buffers |

Winding: triangles that are counter-clockwise on screen are front faces
(`CullMode::Cw`, the default, culls clockwise ones), exactly as in D3D.

Material bind group (group 3 in the shader, built by `FixedFunctionMaterial`):
0 uniform `FixedFunctionUniform`, 1 texture / 2 sampler (stage 0), 3 texture /
4 sampler (stage 1). Missing textures bind Bevy's white fallback and the stage is
skipped. Samplers come from `SamplerState` on the material, not from the `Image`.

Draw order: opaque materials go to Bevy's opaque phase (unordered); `blend`
materials go to the sorted transparent phase (back to front by distance,
`depth_bias` nudges ties). Draws that must keep an exact order should be one mesh.

## Golden harness

`tests/golden.rs` re-creates the 12 scenes of `neoeter/src/render/tests/render_golden.cpp`
(`tests/golden/scenes.rs`: same geometry, 160x120 target, camera, textures, state),
renders them headless to an offscreen image, reads it back (`Screenshot::image`)
and compares with `neoeter/src/render/tests/golden/render/*.png`. It skips with a
message when no GPU adapter exists. Both the PNGs and the wgpu readback are
top-row-first (the C++ capture flips GL's bottom-up `glReadPixels` before writing
the PNG), so no flip is needed; the GL origin only matters for the viewport,
which the scene makes symmetric.

C++ rule (`render_golden.cpp`, `golden/render/README.md`): a pixel matches when
every channel is within 2; a scene passes at 99.5% matching pixels. The goldens
come from Mesa llvmpipe, whose rasteriser differs from a GPU at triangle edges
and texel boundaries. The Rust test is stricter than the C++ rule: per scene it
pins the measured maximum fraction of pixels differing by more than 2 and the
maximum delta, and additionally requires every differing pixel to lie within
one pixel of a strong edge of the golden (surface interiors match exactly).

Failures write `<name>.actual.png` and `<name>.diff.png` to `$M2_GOLDEN_OUT`
(default `<temp dir>/m2_render_golden`) and print the paths; `M2_GOLDEN_DUMP=1`
writes them always. `cargo run -p m2-render --example golden_view -- <scene> [--diff PATH]`
shows a scene next to its golden and writes a diff image.

RESULTS_PLACEHOLDER
