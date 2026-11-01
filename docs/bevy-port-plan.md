# Porting m2rusty to Bevy (Rust)

Status: active migration. Phases 0–2 are complete; the client logs in to a real
server (phase 4). **Since 2026-11-01 the UI is reimplemented in Rust and the port
runs no Python (decision D17 in `rust_port.md`)**; the sections below are updated
for that. The milestone roadmap is `rust_port.md` §8. See
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
| `client/src/ui/*Module.cpp` + `neoeter-python/src/bindings/*Module.cpp` | 38 Python modules, ~1,013 methods | 17.3k | **dropped** (D17): the UI calls Rust game state directly |
| `neoeter-python/src/host` | CPython 2.7 interpreter | 2.3k | **dropped** (D17) |
| `neoeter-python/src/bindings` (non-module) | window tree (`wndMgr`, slots, text) | ~6k | Rust: `m2-ui-core` (logic) + `m2-ui` (Bevy) |
| `bin/pack/root/*.py`, `uiscript/*.py`, `locale/*/ui/*.py` | UI behaviour and layouts in Python | — | behaviour reimplemented in `m2-ui`; the 350 layout files are read as data |

Nothing stays C++. The first plan kept CPython 2.7 and a generated marshalling
layer so the unmodified scripts could run; D17 replaced that with a Rust UI. The
packs do not change: the layout scripts are pure data (dict literals and simple
expressions), so `m2-ui` reads them instead of running them.

## 2. Target architecture

```
  Rust (Bevy)   m2-client (bin): the App; composes the plugins below
                 ├ m2-ui       screens and windows (login, select, in-game);
                 │             layouts read from the original uiscript files
                 ├ m2-ui-core  window tree, text layout, edit and slot logic (no Bevy)
                 ├ m2-game     net plugin, actors, player, world scene, drawing
                 ├ m2-net      packets, framing, phases, account login (no Bevy)
                 ├ m2-world    terrain, map areas, map loading (no Bevy)
                 ├ m2-effect   particles / effect meshes (no Bevy)
                 ├ m2-render   D3D9-style materials, shaders
                 ├ m2-assets   Bevy AssetReader/Loaders
                 └ m2-formats  pack, gr2, spt, msa/msm, protos, uiscript (no Bevy)
```

Rules:

- `m2-formats`, `m2-net`, `m2-world`, `m2-effect` and `m2-ui-core` are pure Rust
  with no Bevy dependency, so they can be tested, fuzzed and differential-tested
  headless.
- Bevy code lives in `m2-assets`, `m2-render`, `m2-game`, `m2-ui` and the
  binaries (`m2-client`, `m2-viewer`). `m2-viewer` is a preview tool built from
  the same `m2-game` pieces as the client.
- No C or C++ in the client build, and no system libraries beyond what Bevy
  needs, so wasm, Android and iOS stay reachable (D16, D17).

### The main loop (superseded)

The first plan ran Bevy *inside* Python's `app.Loop`, because `prototype.py`
owns the loop in the C++ client. With D17 there is no Python: Bevy owns the loop
and the login/select/game flow is Rust state driven by the server's phases.

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
- The UI is drawn by `m2-ui` on top of the scene. Windows are built from the
  original layout files (D17) as a retained tree of `bevy_ui` nodes with the
  client's absolute positions and sprites, not a reflowing layout; text goes
  through `bevy_text` with Metin2 text tags (`|cFF…|h`, `|H…|h`) parsed in Rust
  (`m2-ui-core`).

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

### Phase 2: Asset viewer in Bevy (4–6 weeks) — done
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

### Phase 4: Game logic and networking (6–10 weeks)
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
  character, walks and attacks, all driven from Rust with stub UI. *(Progress:
  `m2-client` and `cargo run -p m2-net --example login` log in to a real server
  through the account server; see `rust_port.md` §1.)*

### Phase 5: UI in Rust (8–12 weeks)
Replaces the first plan's "Python bridge and UI" (D17). Milestones 4 and 5 of
the roadmap in `rust_port.md` §8.
- `m2-formats::uiscript`: evaluate the layout files (`uiscript/*.py`,
  `locale/*/ui/*.py`): dict/tuple/list literals, numbers, strings, arithmetic,
  `float()`/`int()`, `SCREEN_WIDTH`/`SCREEN_HEIGHT` and the `uiScriptLocale`/
  `localeInfo` constants from the locale text files. All 350 files are data (no
  `def`, `class` or loops). Differential-check against CPython on every file.
- `m2-ui`: the window tree on `bevy_ui` built from those layouts (image,
  expanded and animated images, boards, title bars, buttons, text, edit lines,
  scroll bars, slots and grid slots, list boxes), `.sub` sprite sheets, fonts,
  focus, capture, drag and drop, tooltips, cursors, and IME (Bevy `Ime` events).
  `m2-ui-core` provides the ported text layout, edit and slot logic.
- Toolkit: `bevy_ui` (D18). The egui login/select screens of milestones 1-2
  are stopgaps; login and select are rebuilt on `bevy_ui` first, to prove the
  approach on a small surface before the in-game windows.
- Screen behaviour (what `root/*.py` does) is written in Rust against
  `m2-game` state, screen by screen: login and select first, then the in-game
  windows in dependency order (task bar, chat, character/inventory, skills and
  quick slots, minimap, shops/exchange/safebox, quests, party, guild,
  messenger, …).
- **Gate:** login, character select and in-game look like the original (same
  layout files) and work against the server.

### Phase 6: Parity and cut-over (4+ weeks)
- Test every UI window and gameplay system (inventory, shops, exchange,
  safebox, guild marks upload/download, quests, minimap, chat, messenger).
- Performance budget: frame time ≤ the C++ client on the same map with the
  same entity count.
- Remove the C++ engine, game and Python host from the build, and update the
  Nix package to build the Rust binary.

Rough total: 7–11 months for one experienced developer; less with parallel
tracks after phase 1. Phases 2/3 (rendering) and phase 4 (net/game) are
independent once `m2-formats` exists.

## 5. Mapping cheatsheet

| C++ | Rust / Bevy |
|---|---|
| `CSingleton<T>` managers | `Resource` |
| `CGraphicThingInstance`, `CActorInstance` | entity + components (`Transform`, `Actor`, `MotionState`, `Attachments`, …) |
| `CStateManager` / D3D9 render states | material specialisation keys / pipeline descriptors |
| `CResourceManager`, `CResource` refcount | `AssetServer`, `Handle<T>` |
| `CFileLoaderThread`, `AreaLoaderThread` | Bevy async asset loading / `AsyncComputeTaskPool` |
| `ELTimer_GetMSec` | `Res<Time>` (the net plugin keeps its own ms clock for the protocol) |
| `CNetworkStream` phases | `State<NetPhase>` + systems per phase |
| `CPythonEventManager` | Bevy events/messages (`NetMessage`, UI messages) |
| Miles / miniaudio | `bevy_kira_audio` |
| `IME_Sdl`, `Input.cpp` | `ButtonInput<KeyCode>`, `Ime` events, `CursorMoved` |
| FreeType `GrpFontTexture` | `bevy_text` (cosmic-text) |

## 6. Risks

- **UI behaviour is rewritten, not ported.** Without Python (D17), every
  screen's logic in `root/*.py` (~40k lines in `bin/pack/root`) is reimplemented in Rust. The
  layouts come from the original files, but behaviour can drift: compare each
  screen with the C++ client side by side, and treat the server protocol as the
  source of truth.
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

### Decided

- **UI toolkit (D18, 2026-11-01): `bevy_ui` for the game UI, egui for developer
  tools** (the asset viewer, debug overlays) and for stopgap screens until the
  layout loader exists. The client's layouts are absolute positions and sprites,
  which map onto a retained `bevy_ui` tree; slots, drag and drop and tooltips are
  stateful and fit ECS; `m2-ui-core` already holds that state. Editable text and
  IME are the cost: `m2-ui-core`'s edit logic over Bevy keyboard and `Ime` events.

### Open

1. When to delete the superseded Python pieces (`m2-script`, `m2-modgen`,
   `modules/*.toml`, `neoeter_python_abi`): nothing uses them since D17.
2. Windows: confirm the Rust target (`x86_64-pc-windows-gnu` or `-msvc`); with
   no Python there is nothing else to link.
