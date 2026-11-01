# Porting m2rusty to Bevy (Rust)

Status: active migration. Phases 0 and 1 are complete. Foundations for phases
2–5 have been merged; their integration and acceptance gates remain open. See
[`rust_port.md`](../rust_port.md) for current implementation status and findings.

## 1. What we are porting

| Area | Today (C++) | Size | Moves to |
|---|---|---|---|
| `neoeter/src/platform` | OS layer, LZO/TEA/CRC, timers, D3D9→GL compat device, locale codecs, math | 10.6k | Rust (most of it disappears into Bevy/std) |
| `neoeter/src/io` | ZIP and folder pack providers | 1.2k | Rust: `m2-formats::pack` + Bevy `AssetReader` |
| `neoeter/src/image` | DXT, stb, DDS/TGA | 1.9k | Rust: `image` crate + Bevy's DDS support |
| `neoeter/src/render` | Grp* device/state wrappers, camera, culling, fonts, text tags, IME, **network** (`NetStream`), threads, sky/lens flare | 26.7k | Rust: Bevy render + custom materials; net goes to `m2-net` |
| `neoeter/src/audio` | Miles façade over miniaudio | 2.2k | Rust: `bevy_kira_audio` (or `bevy_audio`) |
| `neoeter/src/model` | `.gr2` reader + skinning, SpeedTree `.spt` | 11.6k | Rust: `m2-gr2`, `m2-speedtree` + Bevy asset loaders |
| `neoeter/src/terrain`, `client/src/game/MapOutdoor*`, `Terrain*` | heightfield, splatting, quadtree, water | ~6k | Rust: `m2-terrain` |
| `neoeter/src/effect` | particles, effect meshes, lights | 5.1k | Rust: `m2-effect` (ECS) |
| `client/src/game` | actors, motion, races, items, maps, flying objects, weapon trace | 25.7k | Rust: `m2-game` |
| `client/src/ui` (non-module) | `CPythonApplication`, net phases, player, chr manager, managers | ~41k | Rust: `m2-client`, `m2-net`, `m2-game` |
| `client/src/ui/*Module.cpp` + `neoeter-python/src/bindings/*Module.cpp` | 38 Python modules, ~1,013 methods | 17.3k | **stays C++**, shrunk to argument marshalling |
| `neoeter-python/src/host` | CPython 2.7 interpreter | 2.3k | **stays C++** |
| `neoeter-python/src/bindings` (non-module) | window tree (`wndMgr`, slots, text) | ~6k | Rust (window tree + UI rendering) |

What stays C++: the CPython 2.7 host and a thin marshalling layer for the 38
modules. PyO3 does not support Python 2.7, and `rust-cpython` is unmaintained,
so the Python C API stays on the C++ side, as requested.

The game content (`bin/root/*.py`, packs) does not change. The new client has
to run the same scripts.

## 2. Target architecture

```
                 ┌───────────────────────────────────────────────┐
  Rust (Bevy)    │ m2-client (bin): App, plugins, runner         │
                 │  ├ m2-ui      window tree, UI render, input   │
                 │  ├ m2-game    actors, player, items, skills…  │
                 │  ├ m2-net     TCP stream, phases, packets     │
                 │  ├ m2-world   terrain, map areas, water, sky  │
                 │  ├ m2-effect  particles / effect meshes       │
                 │  ├ m2-render  D3D9-style materials, shaders   │
                 │  ├ m2-assets  Bevy AssetReader/Loaders        │
                 │  └ m2-formats pack, gr2, spt, msa/msm, protos │  (no Bevy dependency)
                 │ m2-script    extern "C" API + callback table  │
                 └──────────────────────▲────────────────────────┘
                                        │  C ABI (cbindgen header)
                 ┌──────────────────────┴────────────────────────┐
  C++            │ neoeter-python/host   Py_Initialize, RunFile  │
                 │ py-modules            38 PyMethodDef tables → │
                 │                       call m2_* functions     │
                 └───────────────────────────────────────────────┘
```

Rules:

- `m2-formats` is pure Rust with no Bevy dependency, so it can be tested and
  fuzzed headless (as `io/tests/fuzz_pack.cpp` does today).
- Only `m2-script` knows the C ABI. The game crates expose ordinary Rust APIs,
  and `m2-script` wraps them.
- The C++ side never touches engine objects. It converts `PyObject*` to and
  from C types and calls `m2_*`. Rust calls back into Python through a
  function-pointer table the host registers at startup (`m2py_call_method`,
  `m2py_incref`/`decref`, …). That covers the ~254
  `PyCallClassMemberFunc` sites in today's game code (net phase handlers,
  window events, …).
- That boundary follows the rules already in `neoeter/docs/abi.md` (opaque
  handles, POD structs, UTF-8 strings, `neoeter_result`, main thread only), so
  the planned C# host can use it too.

### The main loop inversion

Today Python owns the loop: the boot script calls `app.Create()` and then
`app.Loop()`, which blocks inside `CPythonApplication::Loop`. In Bevy, the app
owns the loop through winit.

Phase 0 finding: the boot entry is `system.py` (`UserInterface.cpp`).
`app.Loop` is a plain C call (`PythonApplicationModule.cpp:422`) that returns
only when the game exits. `system.py` ends with `RunMainScript("prototype.py")`,
and `prototype.py`'s `RunApp()` does `app.Create(...)`, `mainStream.Create()`,
`SetLogoPhase()`, `app.Loop()` and then `mainStream.Destroy()`. That teardown
would never run if `Loop` returned early. So **Bevy runs inside `app.Loop`**:

1. `m2-client` builds the Bevy `App` (not yet run) and starts the Python host
   on the main thread.
2. It runs `system.py`. `app.Create` configures the Bevy window.
3. `app.Loop` calls back into Rust, which runs the Bevy runner to completion.
   The script's frame sits on the stack the whole time, as it does in the C++
   client today. Per-frame Python work (the current `Process()` → Python
   callbacks) runs in an exclusive main-thread system.
4. When the window closes, the runner returns, `app.Loop` returns, the
   script's teardown runs, and `Py_Finalize` follows.

This keeps the scripts unmodified. Confirmed against `bin/pack/root/`
(`system.py:295`, `prototype.py:17-61`).

## 3. Rendering approach

The game relies on D3D9 fixed-function state: texture stage ops, alpha test,
linear fog, blend modes, and multipass terrain splatting. Bevy's
`StandardMaterial` will not match it. Instead:

- **`m2-render`** provides a small set of custom `Material`s whose WGSL is
  specialised by shader defs that encode the fixed-function combos actually
  used: `TerrainSplatMaterial`, `SkinnedActorMaterial` (GPU skinning through
  Bevy's `SkinnedMesh`), `SpeedTreeMaterial` (branch/frond/leaf billboards),
  `WaterMaterial`, `SkyMaterial`, `EffectMaterial` (additive/alpha billboards),
  `ShadowReceiverMaterial`.
- Render order today is explicit (`RenderGame`: shadow-to-texture → sky →
  lens flare → clouds → terrain/objects → characters → water → snow → effects
  → items → flyers → PC blocker → lens flare). In Bevy it becomes `RenderLayers`
  and camera order, plus `Transparent3d` sort keys, and a separate camera
  (render-to-texture) for the character shadow map.
- The existing golden images (`neoeter/src/render/tests/golden/render/*.png`:
  alpha test, fog, blending, culling, depth) become the material conformance
  suite. A headless Bevy render of each scene must match within tolerance.
- The UI is drawn by `m2-ui` with a 2D camera on top. The **window tree stays
  retained and script-driven**: Python creates and moves windows by handle. It
  is not ported to `bevy_ui` layout. Each window maps to an entity with a
  sprite/mesh, and text goes through `bevy_text` with Metin2 text tags
  (`|cFF…|h`, `|H…|h`) parsed in Rust.

## 4. Phases

Each phase ends with something runnable and a test gate. The C++ client
keeps building and shipping until phase 6.

### Phase 0: Groundwork (1–2 weeks) — done
Done: Cargo workspace (`crates/m2-script`, `crates/m2-client`), Rust and Bevy
deps in `shell.nix`, Bevy pinned to `=0.20.1`, `neoeter_python_abi` (shared,
`M2_BUILD_PYTHON_ABI=ON`) built by `m2-script`'s `build.rs`, and the
headless boot test `cargo test -p m2-script`. Run everything from
`nix-shell`.

- Cargo workspace at the repo root (`crates/*`), next to the existing CMake.
- Nix: add the Rust toolchain and Bevy's system deps (vulkan-loader, wayland,
  libxkbcommon, udev, alsa) to `default.nix`/`shell.nix`.
- Pin a Bevy release (the latest stable at kickoff) and do not upgrade
  mid-phase.
- Spike: confirm what the boot scripts do after `app.Loop()` (needs the
  devkit `bin/`).
- Spike: link `neoeter-python/host` into a Rust binary through the `cmake`
  crate, boot the interpreter headless, and run `dbg`/`time`/`math`. That
  mirrors today's `TA.6` test.

### Phase 1: Formats, no Bevy (3–5 weeks)
- `m2-formats`: ZIP/folder packs (**done**: `m2_formats::pack`, byte-identical to C++ on all 52,578 files of `bin/pack`), LZO, TEA, CRC32, `Index`/`Index.dev`, path
  normalisation, text-file loader/parser (`TextFileLoader`, `parser.cpp`),
  locale codecs (`encoding_rs` for CP949/CP1252/…, plus the Arabic, Japanese
  and Vietnamese handling), item/mob proto, skill tables, race data, `.msm`,
  `.msa`, property files, map settings, area data.
- `.gr2`: port `Gr2Reader` + `Gr2Decompress.c`. To unblock later phases,
  `Gr2Decompress.c` can first be built through the `cc` crate and ported after.
- `.spt`: port the `SpeedTreeRTCompat` geometry generation.
- **Gate:** differential tests. A small C++ harness dumps parsed structs to
  JSON for every asset in a pack, and Rust must produce identical output. Port
  the existing `test_eterpack`, `test_pack_formats`, `test_image` and
  `test_dxt_invariants` cases.

### Phase 2: Asset viewer in Bevy (4–6 weeks)
- `m2-assets`: a `pack://` `AssetSource` over `m2-formats`, plus loaders for
  textures (DDS/TGA/JPG), gr2 → `Mesh` + `SkinnedMesh` + `AnimationClip`,
  spt → meshes, effect files.
- `m2-render` materials (section 3), checked against the golden images.
- `m2-viewer` (bin): browse a pack, show a model with its motions, a tree, an
  effect.
- **Gate:** golden image suite passes, and a character plays `.msa` motions
  with correct skinning.

### Phase 3: World (4–6 weeks)
- `m2-world`: outdoor map loading (`MapOutdoorLoad`, `AreaLoaderThread` →
  Bevy async asset tasks), terrain patches with splatting, quadtree
  culling (or Bevy frustum culling), water, sky box, clouds, lens flare, snow,
  dungeon blocks, terrain decals, character shadow texture, fog and day/night
  (`EnvironmentMap`).
- `m2-effect`: particle systems and effect instances as ECS components, ported
  with `test_effect_determinism` as the oracle (same seed gives the same
  particle stream).
- Audio: 2D/3D sound and music streaming via `bevy_kira_audio`.
- **Gate:** fly-camera through every map in the client data, at parity with
  the C++ renderer in side-by-side screenshots.

### Phase 4: Game logic and networking, no Python yet (6–10 weeks)
- `m2-net`: non-blocking TCP (`std::net` polled each frame, or a background
  thread with channels), packet header map, packet structs (`zerocopy` /
  `bytemuck`, `#[repr(C, packed)]` checked against `test_packet.cpp`), the
  handshake/login/select/loading/game phases, encryption.
- `m2-game`: `ActorInstance*`/`InstanceBase*` → actor entities with motion,
  blending, battle, fly, weapon trace, collision, and attach components.
  Character/item/skill/NPC managers, player state, movement, sync, flying
  objects, and the event manager become Bevy resources and systems.
- Input: keyboard/mouse picking (`PythonPlayerInput*`) using Bevy input and
  ray casts. The camera procedure ports too.
- **Gate:** a scripted headless bot logs in to a test server, selects a
  character, walks and attacks, all driven from Rust with stub UI.

### Phase 5: Python bridge and UI (6–10 weeks)
- `m2-script`: `extern "C"` API generated by cbindgen, plus the callback
  table. **Generate the C++ marshalling instead of hand-writing it.** One
  declarative spec (for example `modules/player.toml`:
  `GetStatus(int) -> int`) generates (a) the C++ `PyMethodDef` + argument
  parsing and (b) a Rust trait that `m2-script` implements. That replaces most
  of today's ~17k hand-written lines.
- The existing API-surface snapshots (`golden/api.sha256`,
  `test_python_client_api.cpp`) become the conformance check: the generated
  modules must expose the same names and signatures.
- `m2-ui`: the window tree (`PythonWindow`, `WindowManager`, slot/grid-slot,
  text, image, expanded/ratio images, mark images), focus, capture, drag and
  drop, tooltips, cursors, and IME (Bevy `Ime` events in place of `IME_Sdl`).
- Port order follows the dependency chain: `dbg`/`time`/`math` → `app`,
  `systemSetting`, `pack`, `snd` → `wndMgr`, `grp*`, `wnd*` → `net` → `player`,
  `chr`, `chrmgr` → `item`, `skill`, `nonplayer` → the rest (`quest`, `guild`,
  `shop`, `safebox`, `exchange`, `messenger`, `miniMap`, `textTail`, `effect`,
  `fly`, `event`, `eventMgr`, `background`, `chat`, `ime`, `profiler`,
  `ServerStateChecker`).
- **Gate:** the unmodified `root/*.py` reaches the login screen, then
  character select, then in-game.

### Phase 6: Parity and cut-over (4+ weeks)
- Test every UI window and gameplay system (inventory, shops, exchange,
  safebox, guild marks upload/download, quests, minimap, chat, messenger).
- Performance budget: frame time ≤ the C++ client on the same map with the
  same entity count.
- Remove the C++ engine/game from the build. Keep `neoeter-python/host` and
  the generated module shims only, and update the Nix package to build the
  Rust binary.

Rough total: 7–11 months for one experienced developer; less with parallel
tracks after phase 1. Phases 2/3 (rendering) and phase 4 (net/game) are
independent once `m2-formats` exists.

## 5. Mapping cheatsheet

| C++ | Rust / Bevy |
|---|---|
| `CSingleton<T>` managers | `Resource` (or `NonSend` if touched by Python) |
| `CGraphicThingInstance`, `CActorInstance` | entity + components (`Transform`, `Actor`, `MotionState`, `Attachments`, …) |
| `CStateManager` / D3D9 render states | material specialisation keys / pipeline descriptors |
| `CResourceManager`, `CResource` refcount | `AssetServer`, `Handle<T>` |
| `CFileLoaderThread`, `AreaLoaderThread` | Bevy async asset loading / `AsyncComputeTaskPool` |
| `ELTimer_GetMSec` | `Res<Time>` (keep a ms clock resource for the scripts) |
| `CNetworkStream` phases | `State<NetPhase>` + systems per phase |
| `CPythonEventManager` | Bevy events/messages + Python callbacks |
| Miles / miniaudio | `bevy_kira_audio` |
| `IME_Sdl`, `Input.cpp` | `ButtonInput<KeyCode>`, `Ime` events, `CursorMoved` |
| FreeType `GrpFontTexture` | `bevy_text` (cosmic-text) |

## 6. Risks

- **Python owns control flow.** The main-loop inversion (§2) and re-entrancy:
  Python calling `m2_*`, which calls back into Python, which calls `m2_*`
  again. All of it has to stay on the main thread in one exclusive system,
  with no ECS borrows held across a Python call. Design `m2-script` around a
  command queue plus `&mut World` access.
- **Fixed-function fidelity.** Subtle differences in fog, alpha test and
  blending show up immediately. The golden images help, but terrain splatting
  and SpeedTree need their own screenshot comparisons.
- **Granny `.gr2`.** The custom reader and decompressor are the riskiest
  format, so port them early and differential-test them on every model.
- **Bevy churn.** Pin one version, and isolate Bevy APIs behind `m2-render` /
  `m2-assets` so upgrades are local.
- **Packed packet structs.** Unaligned `#[repr(packed)]` access is a common
  source of UB, so read packets via `zerocopy` / byte parsers and never take
  references to their fields.
- **Assets are not in the repo.** Every gate after phase 1 needs the devkit
  `bin/`, so CI needs a way to mount it (or a redistributable test pack).

## 7. Open decisions

1. Keep `neoeter-python` building as a CMake static library linked through the
   `cmake` crate, or move it to a `cc` build in `build.rs`? (Recommend the
   `cmake` crate first, since it reuses the existing setup.)
2. Codegen spec format for the module shims (TOML vs a Rust proc-macro that
   emits C++). Recommend TOML plus a small Rust generator, which keeps the C++
   side dumb.
3. Windows/MinGW: the C++ client cross-builds today. Confirm the Rust target
   (`x86_64-pc-windows-gnu`) and Python 2.7 linking before phase 5.
