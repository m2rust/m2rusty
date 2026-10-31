# Architecture — Neoeter (engine)

## Layering

```
effect ── terrain ── model ── audio ── render ── image ── io ── platform
                                                                ▲
                                                    nothing below this line
```

A directory may depend only on directories to its right. `platform` depends on
nothing. The game and the script bridges depend on the engine, never the other
way round.

## Tree

| Directory | Contents | Target |
|---|---|---|
| `src/platform/base` | OS abstraction, memory, LZO/TEA/CRC, file/mmap I/O, timers, D3D9/GL compat device | `Neoeter::Platform::Base` |
| `src/platform/locale` | locale tables and codecs | `Neoeter::Platform::Locale` |
| `src/platform/math` | `GrpMath`, ray/OBB | `Neoeter::Platform::Math` |
| `src/io` | `EIX`/`EPK`/zip providers, path normalisation | `Neoeter::IO` |
| `src/image` | DXT codecs, stb image integration | `Neoeter::Image` |
| `src/render` | device, fixed-function state, scene graph, `IRenderBackend` | `Neoeter::Render` |
| `src/audio` | Miles façade over miniaudio, null backend | `Neoeter::Audio` |
| `src/model/grn` | `.gr2` reader, skinning/animation | `Neoeter::Model::Grn` |
| `src/model/speedtree` | `.spt` and the `treecache` reader | `Neoeter::Model::SpeedTree` |
| `src/terrain` | heightfield, patches, collision | `Neoeter::Terrain` |
| `src/effect` | particle/effect runtime | `Neoeter::Effect` |

There is a single header root, `include/`: public headers live in
`include/Neoeter/<Role>/…` and are included as `<Neoeter/<Role>/…>`. The
D3D9-compatibility vocabulary shared by all packages is the one top-level
`include/Platform/` shim.

`platform` and `model` have subpackages because they contain more than one
library; the other roles contain their code directly.

## Build paths

The root `CMakeLists.txt` pins one binary directory per role/subpackage
(`build/src/render`, `build/src/platform/base`, …). Consumers therefore never
see a historic package name, and the devkit's test/golden paths are role-based.

## Boundaries

- **Python** lives in `neoeter-python` (`ScriptLib` host, `EterPythonLib`
  bindings). The engine must never include `Python.h`.
- **The game** lives in `client` (`GameLib`, `UserInterface`, `CWebBrowser`).
- **Assets** (`bin/`) stay a devkit-owned tree.

## Script hosts and the ABI boundary

The engine exposes C++ today; the C ABI in `include/Neoeter/` (`docs/abi.md`)
is the language-neutral boundary that a future `neoeter-csharp` host will use.
`docs/scripting-hosts.md` describes the host contract and the C# plan.
