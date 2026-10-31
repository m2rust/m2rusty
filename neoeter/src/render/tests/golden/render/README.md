# Headless renderer goldens (TA.4)

Small, scripted scenes rendered through the client renderer without a window
system: `CGraphicDevice` on the Direct3D 9 compatibility façade, drawing on
EGL's surfaceless platform with Mesa llvmpipe. No X server, GPU or game asset is
involved, so the set runs headless in CI.

```
ctest -R render --output-on-failure
```

The test renders every scene into `<build>/src/EterLib/tests/render-output/`,
captures it with `glReadPixels`, writes a PNG and compares it with the committed
golden below. A pixel matches when every channel is within `--tolerance`
(default 2); a scene passes at `--min-similarity` (default 99.5%). Missing or
resized goldens fail.

## Updating

Only update when the renderer change is intentional. Review the images; a
`clear` or uniform scene means the scene stopped drawing.

```bash
build/src/EterLib/tests/test_render_golden --update
(cd tests/golden/render && sha256sum *.png > manifest.sha256)
```

`manifest.sha256` records the exact PNG bytes for provenance; the test itself
compares decoded pixels so driver rounding does not create noise.

Use `scripts/render-diff.sh <reference-dir> <rendered-dir>` from the devkit to
compare two render directories (also used by T3.2 and T5.4).

## Pinned environment

| Item | Value |
|---|---|
| Mesa | 22.3.6 (`debian:bookworm`) |
| GL | 4.5 compatibility profile |
| Driver | llvmpipe (LLVM 15), `LIBGL_ALWAYS_SOFTWARE=1`, `GALLIUM_DRIVER=llvmpipe` |
| Target | 160×120 RGBA8 + depth24, no multisampling, dither off |
| EGL | surfaceless platform (`EGL_PLATFORM_SURFACELESS_MESA`) |

Bumping Mesa or the target size invalidates every golden: update them in the
same commit and call that out in the task evidence.

## Scenes

| Scene | Pins |
|---|---|
| `clear` | clear colour + empty target |
| `ortho_triangle` | 2D ortho pixel pipeline, vertex-colour interpolation |
| `ortho_blend` | source-alpha / inverse-source-alpha blending |
| `checker_nearest` | texture upload, sampling, nearest filter |
| `texture_modulate` | texel × vertex colour modulation |
| `texture_wrap` | UV repeat beyond `[0, 1]` |
| `depth_test` | depth test rejecting the later, farther quad |
| `perspective_cube` | perspective projection, world transform, z-buffer |
| `cull_backface` | `D3DCULL_CW` face-culling orientation |
| `fog_linear` | D3D linear vertex fog |
| `viewport` | `glViewport` sub-rect mapping |
| `alpha_test` | alpha test with threshold 128 |
