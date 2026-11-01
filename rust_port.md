# Rust / Bevy port: progress, findings and decisions

Living log of the port described in [docs/bevy-port-plan.md](docs/bevy-port-plan.md).
The plan says *what we intend*; this file says *what is done, what we learned
and what we decided*. Update it as work lands.

Last updated: 2026-11-01.

## 1. Status

| Phase | State |
|---|---|
| 0. Groundwork | done (`1b5b551`) |
| 1. Formats, no Bevy | **done**: all 12 format families merged and verified against C++ |
| 2. Asset viewer in Bevy | **done**: asset loaders, fixed-function material, viewer shell, texture + Granny model/motion + SpeedTree + effect (`.mse`) viewers, and the render harness merged; golden-image suite passes, a model plays `.msa` motions with skinning, and effects simulate and draw |
| 3. World | **partial**: world queries/meshes, effect simulation and the `.msenv` environment (fog, lights, gradient sky, clouds) are merged; `m2-viewer --map` draws whole outdoor maps as a preview (LOD 0, splat layers, animated water, buildings/trees, map effects, environment fog, sky and clouds). LOD 1/2, quadtree culling and 3×3 streaming belong to the game-world integration |
| 4. Game logic and networking | **partial**: packet layouts, transport, framing, phase machine and the account (auth) connection merged; **`m2-client` logs in to a real server** (account server, then channel) through a native login screen, selects a character, enters the game and walks/attacks with a third-person camera. Actors are tracked but not drawn; movement and warps streaming and the bot gate remain |
| 5. UI in Rust (D17; was "Python bridge and UI") | **in progress**: `m2-ui` is all `bevy_ui` (D18): login, character select, in-game task bar/chat and the inventory/equipment/costume windows, from the original `.sub`/`.tga` artwork. The Python bridge is dropped; layouts follow the original `uiscript` data files (roadmap §8) |
| 6. Parity and cut-over | not started |

### Phase 1 format families (`crates/m2-formats`)

| Module | Ports (C++) | Verified against C++ on | State |
|---|---|---|---|
| `pack` | `neoeter/src/io` (ZIP, folder, manager, `Index`) | all 52,578 files of `bin/pack`; 9 zipped packs (3,257 files) incl. non-UTF-8 names | merged |
| `script` | `CTextFileLoader`, `LoadMultipleTextData`, `SplitLine*`, `atoi`/`atof` | ~22,000 real files, 3.04 M dump lines, 18 synthetic files | merged |
| `spt` | SpeedTree `.spt` (`SpeedTreeRTCompat`, wrapper) | all 118 `.spt`, 5 variants each (378,654 lines); 30 synthetic files | merged |
| `locale` | `neoeter/src/platform/locale` (iconv wrapper, Arabic, Japanese, Vietnamese) | every byte/pair per code page (~200k lines each), 405 real locale files | merged |
| `crypt` | XTEA ("TEA"), `CLZObject`, LZO1X decoder | 48 LZ objects made by real liblzo, both keys | merged |
| `proto` | item/mob/skill tables, insult list, `lang*.cvt`, `LoadLocaleData` | all 15 `locale_*` packs, both YMIR modes (~15k lines per run) | merged |
| `gr2` | `Gr2Reader.cpp`, `Gr2Decompress.c` | all 8,831 `.gr2` (809,200 lines, 364,853 tracks); 420 damaged files | merged |
| `race` | `.msm` race data, `.msa` motions, `.mss` | 2,877 `.msm` + 6,027 `.msa` (all load; 98.6k lines) | merged |
| `property` | `Property*.cpp` (`.prb .prt .prd .pre .pra`), reserved CRCs | 1,835 files (1,342 properties, 69 reserved CRCs) | merged |
| `map` | `setting.txt`, area, terrain, texture sets, monster areas, atlas | 88 maps, 1,189 terrains, 51,344 area objects (66k lines) | merged |
| `map::environment` | `Environment_Init`/`Environment_Load` (`MapUtil.cpp`, `.msenv`) | all 57 `.msenv`, 570 dump lines | merged (Phase 3) |
| `effect` | `.mse`, particles, `.mde` meshes, flying data `.msf` | 1,148 `.mse`, 98 `.msf`, 112 `.mde` (1.57 M lines) | merged |
| `image` | DXT/DDS, TGA, stb JPEG/BMP/PNG loaders, `CImage` | 7,445 dds, 4,022 tga, 471 jpg, 8 bmp (81.9k lines; 516 files pixel-for-pixel); 44 synthetic files | merged |

Before the later Phase 2–5 merges, 151 tests, clippy, and ten differential
suites passed (`scripts/differential.sh`). In this continuation,
`nix-shell --run 'cargo check --workspace --all-targets'` passes on the merged
workspace. The golden-image test also passes all 12 scenes on the local GPU;
the largest differing-pixel fraction is 4.13% (`texture_modulate`), within the
scene's configured tolerance. The `m2-ui-core` differential suite was re-run
after the `@NNNN` fix and passes all three tests (46 074 corpus strings and 300
random scenarios). The effect viewer was checked by screenshot on
`critical.mse` and `success.mse`. The client's `--login admin admin --enter 0
--screenshot` run was checked end to end: it logs in, selects slot 0, resolves
`metin2_map_b1` from `AtlasInfo.txt` and draws the character in the b1 village
(buildings, trees, fog, sky). `--demo` walks to the nearest actor, swings at it
and the screenshot shows the attack pose with the target named in the HUD; the
server kept the reported position across runs, so the `FUNC_MOVE` stream is
accepted.

## 2. What exists

- **Cargo workspace** at the repo root (`Cargo.toml`, `crates/*`). Bevy is
  pinned to `=0.20.1`. Dev builds optimise dependencies (`opt-level = 3`).
- **`crates/m2-script`** (superseded by D17, no longer used by the client): `build.rs` builds `neoeter_python_abi` (shared C
  library over the CPython 2.7 host) with the repo's CMake and links it. A safe
  `PythonHost` wrapper (one per process, main thread only). `cargo test -p
  m2-script` boots the interpreter headless and runs `dbg`/`time`/`math`.
- **`crates/m2-client`**: the Bevy app; it composes `m2_game::net::NetPlugin` and
  `m2_ui::M2UiPlugin`. No Python and no C++ build step (D17). `m2-client --login
  admin admin --screenshot out.png` logs in unattended and captures the select
  screen; `--enter <slot>` starts the game and captures the character in the map;
  `--demo` walks to the nearest actor and swings once before the capture;
  `--host`/`--auth-port` override the shipped `serverinfo.py` defaults.
- **`crates/m2-audio`**: `neoeter/src/audio` on `bevy_audio`. `SoundManager`
  (system param) ports `CSoundManager`: 2D sounds (4 at once), 3D sounds (32,
  heard from the `SoundListener` that `m2-game` keeps on the camera centre,
  with the Miles shim's inverse-distance falloff), the three music slots with
  their fade-in/fade-out/limit fades, volume grades, save/restore and
  `metin2.cfg` volumes. `M2Sound` assets (PCM `.wav`, `.mp3`) are validated on
  load; all 1,635 shipped sounds decode. `m2-ui` plays the screen music, the
  login result sounds and button clicks. Motion (`.mss`), effect and map
  ambience sounds are not wired yet.
- **`crates/m2-ui`**: the game UI in Rust (D17), all `bevy_ui` (D18). `login.rs`
  holds the login/character state and the original failure messages; `select.rs`
  draws the account's characters in 3D with their intro motions and enters the
  game. `frontend.rs` is the login and character-select window (original
  `locale/en/ui` artwork), `core.rs` the shared boards, title bars, buttons,
  tooltips and draggable windows, `hud.rs` the in-game task bar (HP/SP/stamina
  gauges and quick slots), chat and character/system windows, and
  `inventory.rs` the Metin2 inventory: equipment paper doll (equipped weapon
  included), 45-cell pages, money, item icons from `item_list.txt`/`item_proto`,
  tooltips and use/move/split/quick-bind actions sent to the server through
  `PlayerAction`; `character.rs` the tabbed character window (status, skills,
  emotions, quests); `skills.rs`/`emotes.rs` its skill and emotion pages. Items,
  skills and emotions can be assigned to the task bar's quick slots by
  dragging or Ctrl+click, and used with the 1-4/F1-F4 keys. Non-UTF-8 and
  `.sub` sprite regions are decoded by `m2-assets`.
- **`crates/m2-game`**: the game-side Bevy library shared by the client and the
  viewer. `render::{model, tree, effect, terrain, sky, cloud}` draw with the
  client's render states; `render::character` dresses a race in its shape and
  hair and plays `.msa` motions (the select scene). `world::prepare_map` loads a
  map (`m2_world::load_map` plus the ground texture array and effect instances)
  off the main thread and `world::MapScene` spawns and runs it.
  `ingame::{InGamePlugin, GameWorld}` is the in-game scene: on `EnterWorld` it
  resolves the map from the global position in `AtlasInfo.txt`
  (`world::prepare_map_at`), loads it off the main thread and spawns the map, the
  player character at its terrain height and a third-person camera. The same
  module drives the controls: left-click ground picking (heightfield ray march),
  `WASD`/arrows relative to the camera, `FUNC_MOVE`/`FUNC_WAIT` position reports
  every 300 ms, and space attacks the nearest actor (`CG_ATTACK`). The character
  turns towards its movement direction at the client's rotation speed
  (`SetAdvancingRotation`: 1200°/s, 5/12 of that under 45°), and motion clips
  switch without blending, as the client does. The game lightens the
  environment's fog (`soften_fog`: it starts at 100 m and is stretched) so the
  near view stays clear; the viewer keeps the file's exact fog. Other actors are
  tracked from `ActorAppend`/`Move`/`Sync`/`Remove`.
  Map lighting comes from the `.msenv` environment (`render::lighting`): two
  directional lights (background and character) and the D3D9 material ambient,
  with the original terrain formula `ambient·light.ambient + |N·L|·diffuse` and
  a 2048 shadow map on the background light, instead of the old hard-coded
  sun. Everything spawns under a caller-owned root; the scene camera carries
  `m2_game::SceneCamera`. `net::NetPlugin` is the
  server connection: `m2-net` over a non-blocking `TcpTransport`, pumped in
  `PreUpdate`, driven by `LoginRequest`/`EnterGame` and reported as
  `NetMessage`s; the open game connection is `Net::game`. `m2-world` stays
  free of Bevy (D3), so loading is there and spawning here.
- **Phase 2 crates:** `m2-assets` registers an `m2://` source and loads textures,
  `.gr2` models/motions, `.spt` trees and race data; `m2-render` provides a
  D3D9-style fixed-function material; `m2-viewer` has the pack browser, camera,
  inspector shell and the texture, Granny model/motion, SpeedTree and effect
  viewers. The 12-scene golden-image suite passes on the current machine.
  The effect viewer and the map effects share one Bevy adapter
  (`m2_game::render::effect`): `m2-effect` produces quads/meshes, the adapter
  translates blend, texture-stage and depth state and keeps one entity per
  batch alive across frames.
- **Viewer CLI:** `m2-viewer [client dir] [--index file] [--motion <vpath>]
  [--map <name>] [--fog] [--screenshot <vpath> <out.png>]
  [--screenshot-only <out.png>]`.
  `--motion` starts a `.msa`/motion-only `.gr2` on the opened model, which is
  what makes the skinning walkthrough checkable from the command line. A `.mse`
  screenshot (`--screenshot "d:/ymir work/effect/…/x.mse" out.png`) is the
  effect check; the viewer loops and frames the effect from the live particle
  extent. Screenshot mode ignores scene input, so repeated captures are
  byte-identical.
- **World and effects:** `m2-world` ports terrain height/attribute queries,
  patch meshes, splats, lighting and area placement. `m2-effect` ports particle,
  mesh and light simulation with a C++ differential harness, and is drawn by
  Bevy through the shared effect adapter (map effects and the `.mse` viewer).
  `m2-world` is drawn through `m2-game` (see above), as in the map viewer:
  `m2-viewer --map metin2_map_a1 --screenshot-only /tmp/a1.png` loads a map on
  `AsyncComputeTaskPool` and spawns one mesh per terrain patch (LOD 0, UVs from
  `m2_world::lighting`, splat layers blended in `m2-game`'s `terrain.wesl`, animated water,
  buildings, trees and the placed effects). The map's `.msenv` environment is
  ported (`m2_formats::map::environment`, differential-tested on all 57 files);
  its linear or density fog is applied to terrain, water, objects, trees and
  effects, and its fog colour is the clear colour. `--fog` (or the inspector
  checkbox) turns fog on; the default overview camera is far outside the map's
  fog range, so fog is opt-in. The gradient sky is baked from
  `SkyBoxGradientColorVector` into a cubemap on the camera (`Skybox`), which
  Bevy draws as a background; the scrolling cloud layer is a large quad with
  the client's `MODULATEINVALPHA_ADDCOLOR` and `ONE`/`INVSRCCOLOR` state. It is
  depth-tested with Bevy's reverse-Z so terrain and objects occlude it, its
  texture sampler repeats (`CloudTextureScale` tiles it), and the shader
  emulates `INVSRCCOLOR` by writing the cloud luminance into alpha (WebGPU has
  no source-colour destination factor; the shipped cloud textures are
  grayscale, so it matches).
  Textured sky boxes (`SkyBoxTextureRenderMode`) are still missing. The viewer
  is deliberately a preview: LOD 1/2, quadtree culling and 3×3 streaming belong
  to the game-world integration, not to `m2-viewer`.
- **Networking:** `m2-net` contains packet structs/codecs, packet framing,
  sequence handling, transport abstractions, the game phase machine and
  `auth` (a port of `CAccountConnector`: handshake, `CG_LOGIN3`, login key).
  `transport::pump` drives either machine. `cargo run -p m2-net --example login`
  is a headless login (auth, then channel) against a real server. It drives the
  `m2-client` login; game entities are not connected yet.
- **UI core and Python modules:** `m2-ui-core` ports the C++ window, slot, text
  and edit logic (no Bevy, no Python; its `UiHost` events stood in for the Python
  handlers) and is reusable by `m2-ui`. `m2-modgen` generates the Python module
  shims; with D17 it has no consumer.
- **`crates/m2-formats`**: pure Rust, no Bevy dependency. Binary format APIs
  stay byte-oriented; `utf8` provides the `encoding_rs` display boundary and
  `locale` keeps its glibc reference behind the optional `glibc-tables` feature.
- **C ABI** (`neoeter-python/include/Neoeter/Python/Abi/neoeter_python.h`):
  `neoeter_python_version/init/run_line/shutdown`. Opt-in via
  `-DM2_BUILD_PYTHON_ABI=ON`; the normal C++ build is unchanged. Only
  `neoeter_python_*` symbols are exported (the ~2,850 engine symbols are hidden
  with `--exclude-libs`).
- **Differential harness**: `tools/differential/<module>/` holds one C++ dumper
  per format (`m2diff_<module>`, linking the *real* C++ loader), built with
  `-DM2_BUILD_DIFF_TOOLS=ON`. `scripts/differential.sh [client dir] [filter]`
  builds them all and runs the ignored `tests/differential_*.rs`. Each test
  compares a line-based dump from C++ and Rust, floats as IEEE bit hex.
- **`shell.nix`**: Rust 1.99 toolchain (cargo, clippy, rustfmt, rust-analyzer)
  and Bevy's runtime libraries (vulkan-loader, wayland, libxkbcommon, X11,
  udev, alsa). Everything runs from `nix-shell`.

### Differential suites: current results

`scripts/differential.sh` runs every `crates/*/tests/differential*.rs` now, not
just the format readers. On the local `bin/` (2026-11-01):

| Crate | Tests | Result |
|---|---|---|
| `m2-formats` (11 files) | 39 | pass (`gr2` 71 s, `image` 28 s, `proto` 20 s, rest fast; `env` 57 `.msenv`, 570 lines) |
| `m2-net` | 6 | pass (structs, offsets, headers, sequence, framing) |
| `m2-world` | 1 | pass (88 maps, 156 080 dump lines, 43 s) |
| `m2-effect` | 2 | pass (1 148 effects, 734 720 frames) |
| `m2-ui-core` | 3 | pass (46 074 corpus strings, 300 random scenarios, 252 615 dump lines) |

The UI dumper used to fail for reasons that were not Rust bugs, all fixed:
`RECT` fields went through `%ld` as 32-bit `LONG` (variadic UB, so `-163`
printed as `4294967133`), engine `Tracef` lines were compared as dump lines,
and an empty text measured the stub font's space glyph (height 16) instead of
the fake metrics. The remaining real difference — `@NNNN` is a **code-page
switch token** in `C++` (`GrpTextInstance.cpp:88-116`, five characters,
`@9999` = UTF-8) — is now decoded the way the C++ iconv loop does, so the
`m2-ui-core` suite passes in full (2026-11-01).

## 3. Findings

### Architecture
1. **Python owns the main loop, so Bevy must run inside `app.Loop`.**
   *(Superseded by D17: the port runs no Python, so Bevy owns the loop.)*
   `system.py` ends with `RunMainScript("prototype.py")`; `RunApp()` there calls
   `app.Create(...)`, `mainStream.Create()`, `SetLogoPhase()`, `app.Loop()`,
   then `mainStream.Destroy()`. `app.Loop` is a plain C call that returns only
   at exit, and the teardown after it would never run if `Loop` returned early.
   Design: Rust builds the Bevy `App`, starts Python and runs `system.py`;
   `app.Loop` calls back into Rust, which runs Bevy's runner to completion.
   (Confirmed against `bin/pack/root/`; supersedes the first plan.)
2. **There is no EIX/EPK reader in this codebase.** Packs are only ZIP archives
   or plain folders. The plan's "EIX/EPK" line was wrong and was corrected.
3. **`pack/Index` and `pack/Index.dev` differ.** `Index` lists `ZIP` packs, none
   of which exist in this `bin/`; `Index.dev` lists the same packs as `FOLDER`.
   Only `Index.dev` loads data here. A `_DISTRIBUTE` build reads `Index`.
4. **The Python module surface is large**: 38 modules, ~1,013 `METH_VARARGS`
   functions, ~17k lines of binding code, and ~254 C++-to-Python call sites in
   game code. That is why the plan generates the shims from a spec.
5. **Linux text conversion in C++ is glibc `iconv`.** `StringCodec.cpp` is a
   loop around `iconv`, so the "reference behaviour" on Linux is glibc's, not
   Windows'. Differences exist (CP1252 0x81, GBK/Big5 edge cases, CP1255/CP1258
   accent combining).
6. **"TEA" is XTEA with 32 rounds.**
7. **There is no stream cipher on the network.** In `CNetworkStream::Send` the TEA call is commented out and `Recv` never decrypts. TEA protects only pack and proto files. The only per-packet transformation is the sequence byte. The `adwClientKey` sent in `LOGIN2` is just a configured value. (From the `m2-net` port.)

### Data
7. `bin/pack` holds 109 pack folders (2.2 GB, 52,578 virtual files). 31 file
   names contain non-ASCII (CP949) bytes, which is why keys are `Vec<u8>`.
8. Three files in the root pack are in an older layout that both C++ and Rust
   reject: `20110915_ymir_item_proto` (152-byte records, expected 156),
   `936mob_proto` (235-byte records), `936skilldesc.txt` (different columns).
   They look like leftovers; the locale packs carry the live tables.
9. `bin/` has no SpeedTree caches (`treecache/`), no `.cvt` files and no ZIP
   packs, so those paths are covered only by synthetic fixtures.
10. The `.gr2` files are all Oodle1-compressed. At most one animation per
    file, which hides a C++ dangling-pointer bug (see §5).
11. Reference timings: reading all 52,578 files takes 2.3 s in Rust vs 12.4 s
    in C++ (pack). `.gr2` whole-suite is comparable (C++ 33 s, Rust 41 s,
    including pack reads and dump formatting; parse time alone not measured).

See also [c++_rust_bugs.md](c++_rust_bugs.md) for the C++ bugs found along the way.

## 4. Decisions

| # | Decision | Why |
|---|---|---|
| D1 | *(Superseded by D17)* Python 2.7 and the 38 module shims stay in C++ | PyO3 does not support 2.7; this was the brief |
| D2 | *(Superseded by D17)* `m2-script` links one shared library (`neoeter_python_abi`) | Avoids Rust having to link the engine's static archives and their system deps |
| D3 | Pin Bevy `=0.20.1` | Avoid churn mid-phase; keep Bevy dependencies in the asset/render/viewer crates |
| D4 | Rust ports must match C++ *byte for byte on real data*, proven by differential tests | The only way to port 100k+ lines of binary/text format code without regressions |
| D5 | Phase 1 names and contents stay `&[u8]`/`Vec<u8>`; new display APIs decode to UTF-8 | Preserve byte-exact format behavior while exposing UTF-8 to the rest of the client |
| D6 | Reference = Linux C++ build (glibc `atoi`/`atof`/`iconv`) | That is what builds here; MSVC would differ on `atoi` overflow, hex floats, some code pages |
| D7 | Malformed input returns an error, never panics; C++ undefined behaviour is *not* mirrored | Several C++ readers over-read or divide by zero on corrupt files; deviations are documented per module |
| D8 | Real C++ bugs are mirrored when they affect output on real data, with a comment | e.g. Arabic shaping skips one index after LAM-ALEF; CP1255/CP1258 decoder loses its held character at end of input |
| D9 | Dependencies pinned per crate, exact versions in `crates/m2-formats/Cargo.toml`; `zip` uses `deflate-flate2-zlib-rs` (pure Rust) | Keep the format crate free of C builds |
| D10 | *(Superseded by D17)* Bevy runs inside `app.Loop` (§3.1) | Keeps `prototype.py` unmodified |
| D11 | Differential dumpers live in `tools/differential/` (opt-in `M2_BUILD_DIFF_TOOLS`), not in the engine tree | Keeps test tooling out of the C++ libraries |
| D12 | Parallel module ports run as subagents in git worktrees, one module each, merged by hand | Modules are independent once `script` exists |
| D13 | Not ported: `parser.cpp` (`script::Group`), skill icon images, the Vietnamese weapon-vnum conversion, the LZO compressor, CRC32 (no loader uses it), `Arabic_ConvEnglishModeSymbol` | Unused or only in other builds |
| D14 | **UTF-8 everywhere it is possible** (user decision, 2026-11-01). New Rust APIs (Phase 2 onwards) expose text as `String`/`&str`. Legacy code pages are converted to UTF-8 once, at the file boundary; other encodings are not a goal and are not preserved on output | The user does not care about non-UTF-8 encodings. Phase 1 modules stay byte-oriented because they are proven byte-identical to C++; a UTF-8 façade is added on top, not a rewrite |
| D15 | C++ bugs that are replicated or deliberately not replicated are tracked in [c++_rust_bugs.md](c++_rust_bugs.md) | One ledger, stable IDs, referenced from Rust comments |
| D17 | **The UI is reimplemented in Rust; the port runs no Python** (user decision, 2026-11-01). Supersedes D1, D2 and D10 and the Python-bridge plan of Phase 5. Screens live in `m2-ui` on top of `m2-game` state; the original `uiscript`/`locale/*/ui` layout files are read as data | Removes CPython 2.7, the C ABI, the generated shims and the main-loop inversion; the client becomes pure Rust (one toolchain, and wasm/mobile are reachable). The 350 layout files are pure data (dict literals and simple expressions, no `def`/`class`/loops), so the original look survives without an interpreter |
| D18 | **The game UI is built on `bevy_ui`; egui is for developer tools only** (user decision, 2026-11-01). The asset viewer and debug overlays use egui; the game client's login, select, task bar and windows are a retained `bevy_ui` tree | The layouts are absolute positions and `.sub` sprites, which map almost one to one onto `bevy_ui` nodes; matching the original pixel for pixel in egui would be a constant fight. Slots, drag and drop, cooldowns, tooltips and chat links are stateful, which suits ECS entities better than immediate mode. `m2-ui-core` already holds that state as a retained tree. One renderer, input path, asset source and version pin, on every target. Cost: no ready-made widgets, and editable text/IME is `bevy_ui`'s weak spot (covered by `m2-ui-core`'s edit logic over Bevy keyboard and `Ime` events) |
| D16 | **Reuse reputable Rust crates instead of reimplementing formats and codecs, but only if they work on every target platform: desktop (Linux/Windows/macOS), wasm32, Android and iOS.** In practice: pure Rust, no C/C++ build step or system library, maintained and widely used. Where a crate would change output on valid real data compared with C++, the differential test decides and own code is used (or the difference is accepted explicitly by the user) | User policy, 2026-11-01. Less code to own, and the port keeps the option of non-desktop targets |

## 5. Known deviations from C++ (all on malformed input or C++ UB)

- **pack:** an `Index` without a final newline loses its last pack in C++;
  Rust reads it. Duplicate virtual names in a folder: C++ order is unspecified,
  Rust takes byte order.
- **script / text:** a file ending in a lone CP949 lead byte makes C++ read one
  byte past the buffer (`locale/hu/insult.txt` does this); Rust clamps. `start`
  without a name reads out of bounds in C++; Rust uses an empty name.
  `atof("nan(123)")` payloads are not preserved.
- **spt:** a tree cache with more than ~21,845 vertices makes C++ loop forever
  (16-bit counter); Rust skips those normals. Negative/huge float-to-unsigned
  casts copy what x86-64 GCC does.
- **locale:** CP949 `A2 E8` at the very end of input makes C++ read past the
  end; Rust stops.
- **crypt / proto:** unchecked LZO decode, element counts beyond the
  decompressed data and partial TEA blocks over-read in C++; Rust returns an
  error or zero-pads. `CItemManager::Destroy` does not clear `m_vec_ItemRange`
  (dangling pointers across locale reloads); Rust starts fresh per load.
- **gr2:** 32-bit bounds checks wrap in C++; relocations past the section table
  index out of bounds; each animation shares a track vector that later
  animations reallocate (dangling pointer when a file has more than one
  animation); a zero weight total divides by zero. Rust checks all of these and
  adds a 256 MiB section cap and a work budget.
- **cloud:** `D3DBLEND_INVSRCCOLOR` (destination scaled by one minus the source
  colour) has no WebGPU blend factor; the cloud shader writes the source
  luminance into alpha and uses `ONE`/`ONE_MINUS_SRC_ALPHA`. Exact for the
  shipped grayscale cloud textures, approximate for a tinted `CloudColor`.

## 6. Open items and risks

- **Phase 2 gate:** closed. The golden-image suite passes, the texture,
  model/motion, SpeedTree and effect viewers are registered, and `.mse` effects
  simulate.
- **Missing-model screenshots, fixed.** `m2-viewer --motion
  "d:/ymir work/pc/assassin/general/wait.msa" --screenshot
  "d:/ymir work/pc/assassin/assassin.gr2" out.png` used to show only the ground
  grid: the `.gr2` names `assassin_lord1/2.dds`, which are not in the packs, and
  Bevy drops a mesh whose texture fails to load. `m2_game::render::model` now
  leaves a missing texture out so the part draws untextured, like the C++ client
  (`/tmp` check: the assassin draws and animates). Effect screenshots were not
  re-checked in the same run.
- **Phase 3 gate remains open:** LOD 1/2, quadtree culling, 3×3 streaming,
  textured sky boxes, audio and gameplay entities remain. Terrain, splats,
  water, objects, map effects, fog, sky and clouds are drawn (`m2-game`).
- **Phase 4 gate remains open:** login, character select, entering the game and
  movement/attack work against a real server (the player walks where clicked and
  the server keeps the reported position); actors are tracked but not drawn,
  terrain attributes/collision are not checked, warps are not handled and the
  headless bot does not exist yet.
- **In-game fog is deliberately softer than the environment files.** The
  client's linear fog starts at 50 m and is full at 200 m, which washed out the
  near scene; the game uses `m2_game::render::soften_fog` (start at 100 m, full
  at +400 m, density halved). The viewer's `--fog` still shows the exact file.
- **CP949 object names block a few map objects.** `metin2_map_b1` places
  `ymir work/zone/<CP949 dir>/hay_01.gr2`/`hay_02.gr2`; the `m2://` source takes
  UTF-8 names, so those two fail to load (logged by Bevy). The reversible
  canonical mapping promised in §9 is now needed for full map objects.
- **Movement has no collision or pathfinding.** Click-to-move runs in a straight
  line and only the terrain height is sampled: the character can walk through
  buildings, trees and water (`m2_world`'s attribute maps are ported but not
  wired). Attacks target any tracked actor inside 320 units (NPCs included) and
  the server's damage replies are ignored.
- **Superseded by D17, still in the tree:** `m2-script`, `m2-modgen`, the C ABI
  (`neoeter_python_abi`, CMake option `M2_BUILD_PYTHON_ABI`) and the module
  specs in `modules/`. Nothing in the client uses them any more; remove them
  when the user confirms.
- **No `rustfmt.toml`:** `cargo fmt` would reformat every module. Agents format
  their own files with `max_width = 120, use_small_heuristics = "Max"`. Add that
  as a config before the next big merge.
- **Windows text behaviour:** the locale port reproduces glibc; if Windows
  behaviour matters, regenerate the 320 KB code-page tables
  (`m2diff_locale tables`) from a Windows reference.
- **Assets are not in the repo:** every differential gate needs the client in
  `bin/` (git-ignored). CI needs a way to mount it.
- **Windows build** of the Rust client is unverified. Without Python (D17) it is
  a plain Rust target (`x86_64-pc-windows-gnu` or `-msvc`).
- **First Bevy compile is slow** (~11 min); later builds are incremental.
- **Differential suite location:** it needs `build/differential` in the
  checkout it runs from; agents' worktrees each build their own copy.
- **Never share one `target/` between two checkouts of the workspace.** Cargo
  names path-dependency artifacts by their workspace-relative path, so a second
  checkout overwrites the first one's crates with older code that still looks
  fresh. Use a separate `CARGO_TARGET_DIR` per worktree.

## 7. How to run things

```bash
nix-shell                                  # everything below runs inside it
cargo test -p m2-formats                   # unit + fixture tests
cargo clippy -p m2-formats --all-targets
scripts/differential.sh                    # Rust vs C++ on bin/ (slow: ~1 min per suite)
scripts/differential.sh bin gr2            # one suite
cargo run -p m2-net --example login        # headless login: 127.0.0.1 11000 13000 admin admin
cargo run -p m2-client                     # the client: login screen
cargo run -p m2-client -- --login admin admin --screenshot /tmp/login.png
cargo run -p m2-client -- --login admin admin --enter 0 --screenshot /tmp/ingame.png
cargo run -p m2-client -- --login admin admin --enter 0 --demo --screenshot /tmp/demo.png
cargo run -p m2-viewer -- bin --map metin2_map_a1
```

The local test server is `serverinfo.py`'s default: account server
127.0.0.1:11000, CH1-CH4 on 13000/13010/13020/13030, account `admin`/`admin`.

## 8. Roadmap

Done so far:

1. ~~Differential suites for networking, world, effects and UI core~~ (see §1).
2. ~~Phase 2 viewers~~ (texture, model/motion, SpeedTree, effect, map).
3. ~~**Milestone 0, prerequisites**~~ (2026-11-01): the `m2-assets` real-data
   tests check the real bind-pose invariant; drawing and map loading moved out of
   the viewer into `m2-game` and `m2_world::load_map`.
4. ~~**Milestone 1, connect and log in**~~ (2026-11-01): `m2_net::auth`
   (`CAccountConnector`), `m2_game::net::NetPlugin`, `m2-ui`'s login screen. The
   client logs in to the local server and lists the account's characters.

Next, in order. Each milestone ends with something visible against the server.

5. ~~**Milestone 2, character select and entering the game**~~ (2026-11-01),
   for the existing-character path:
   `m2-ui`'s select screen draws the characters (race model, armor shape, hair,
   intro motions) and starts the chosen slot; `m2-game` sends
   `SendSelectCharacter`/`SendEnterGame`, and `m2_game::ingame` resolves the map
   from the global position in `AtlasInfo.txt` (`m2_world::{load_atlas, map_at}`),
   loads it off the main thread, spawns the player at its terrain height and a
   third-person camera. Visible result: `--enter 0` stands in `metin2_map_b1`'s
   village, drawn with its buildings, trees, fog and sky. Character creation,
   deletion and the empire choice for a new account move with the real
   `bevy_ui` select screen (Milestone 4).
6. **Milestone 3, the living world** (in progress, 2026-11-01):
   - ~~Click-to-move and keyboard movement with `send_move`; terrain height;
     third-person camera (orbit/zoom).~~ Done: ground picking, `WASD`/arrows,
     `FUNC_MOVE` every 300 ms and `FUNC_WAIT` on stop, the server accepts and
     persists the reported position.
   - ~~Attack~~ Done for the player: space plays `attack.msa` and sends
     `CG_ATTACK` to the nearest tracked actor inside `ATTACK_RANGE`.
   - Actors from `ActorAppend`/`Update`/`Move`/`Remove`/`Sync` are tracked (VID,
     race, name, position) but not drawn yet: NPCs, monsters and other players
     with their motions (`.msa` wait/run/attack, blending) are the next step.
   - Game-world rendering the viewer left out: 3×3 terrain streaming around the
     player, LOD 1/2, quadtree culling, warps between maps.
   - Visible result so far: walking around b1 and swinging at the nearest NPC,
     shown by `--demo` (the character moves and the HUD names the target).
7. **Milestone 4, the UI system (the original look, from the original data).**
   - `m2-formats::uiscript`: evaluate the layout dialect of `uiscript/*.py` and
     `locale/*/ui/*.py` (dict/tuple/list literals, numbers and strings,
     arithmetic, `float()`/`int()`, `SCREEN_WIDTH`/`SCREEN_HEIGHT`, constants
     such as `uiScriptLocale.*` and `localeInfo.*` from the locale text files).
     All 350 files are data: no `def`, `class` or loops. Differential-check the
     evaluated trees against CPython on every file.
   - The toolkit is `bevy_ui` (D18). Editable text and IME are its weak spot:
     edit lines are `m2-ui-core`'s ported edit logic fed by Bevy keyboard and
     `Ime` events; widgets reproduce the client's own, so none come from a crate.
   - `m2-ui`: a window tree on `bevy_ui` built from those layouts (image,
     expanded/ani image, board/thinboard, title bar, button/toggle/radio, text,
     edit line, scroll bar, slot/grid slot, list box), `.sub` sprite sheets, the
     client fonts, focus, drag and drop, tooltips. Reuse `m2-ui-core` for text
     layout, edit and slot logic.
   - Rebuild login and select on it first: a small surface that proves the
     approach before the in-game windows depend on it, and retires the egui
     stopgaps. Visible result: they look like the original, from the same
     layout files.
8. **Milestone 5, in-game windows**, in dependency order: task bar, chat, target,
   character and inventory (items from `item_proto`), skills and quick slots,
   minimap, shops, exchange and safebox, quest letters and dialogs (`SCRIPT`
   packets), party, guild, messenger, and the rest.
9. **Milestone 6, parity and cut-over.** The headless bot gate (log in, select,
   walk, attack, driven from Rust), frame-time budget against the C++ client,
   then remove the C++ client and the Python host from the build and package the
   Rust binary with Nix.

## 9. UTF-8 policy (D14): what it means for the code

- **Phase 1 stays as is.** `pack`, `script`, `race`, … keep `Vec<u8>` names and
  tokens. That is what makes them verifiable byte for byte, and changing it now
  would throw the comparison away. 31 file names in `bin/pack` are CP949 bytes.
- **New code is UTF-8.** `m2-formats::utf8` uses `encoding_rs` for the shipped
  legacy code pages. UI text decodes to Unicode scalar values through this
  facade; the glibc-exact implementation is behind `glibc-tables` for reference
  tests.
- **Pack names are a known boundary limitation.** `m2-assets` exposes UTF-8
  asset paths, so the 31 CP949-only names in the client pack cannot be addressed
  by that Bevy source yet. Keep raw names available in `m2-formats`; add a
  reversible canonical mapping before the game or viewer needs those files.
- **Decode proto and locale display text on load.** Keep raw bytes for keys and
  differential comparisons; expose the decoded form to UI and game code.
- **Layout files:** the `uiscript` layouts are read as data (D17), so their
  string literals (window names, image paths, locale keys) are decoded to UTF-8
  when they are loaded, like every other legacy text file.

## 10. Crate-reuse policy (D16): what it means for the code

Rule of thumb when a format or codec is needed:

1. Look for an existing crate that is reputable (maintained, widely used).
2. Check it builds for all targets: desktop, `wasm32-unknown-unknown`, Android
   (`aarch64-linux-android`) and iOS (`aarch64-apple-ios`). A C/C++ build step
   or a system library disqualifies it, as does `std::fs`/thread assumptions
   that do not hold on wasm.
3. Use it only if the differential test against C++ still passes on the real
   data. If it differs on valid data, port the C++ logic or get the user's
   explicit OK for the difference.

Already in line with the policy: `zip` (with `deflate-flate2-zlib-rs`, pure
Rust), `crc32fast`, and the no-C-dependencies choice for `m2-formats`.

Candidates to revisit under D16 (each needs a differential check, none is urgent):

- `crypt`: own LZO1X decoder and XTEA; a maintained pure-Rust LZO crate may
  exist.
- `locale`: `encoding_rs` is the default display codec; the glibc-exact tables
  remain optional as the differential reference (see §9).
- `gr2`'s decompressor (Oodle1-style) and `spt` have no crate equivalents;
  they stay as own code.
- `image`: evaluated, see below. BMP is the one case where a crate matched on all real files but own code was kept; switch to the `image` crate if you want strict D16.

With D17 the client no longer links the Python 2.7 host (`neoeter-python`, C and
C++ through `m2-script`), so nothing in the client stops wasm32 or iOS except the
`std::net` transport, which `m2-net` keeps behind its `tcp` feature (a WebSocket
`Transport` would replace it on wasm).

### Crate evaluation for `image` (D16, measured)

| Codec | Used | Evidence |
|---|---|---|
| JPEG | own port of stb's decoder | `image` 0.25.10 (zune-jpeg 0.5.15) differs from C++ on 468 of 471 real files (max channel delta 4) |
| DXT1/3/5 | own port | The conventional high-bit-replicating decode differs on 6,525 of 6,549 DXT1 files: `CDXTCImage` expands 5/6-bit channels with `<< 3`/`<< 2`. `ddsfile` only parses containers. `texpresso`/`bcdec` could not be fetched offline, so an equivalent decoder written inline was measured instead |
| TGA | own port | `image`'s TGA differs on all 4,191 files (max delta 255) |
| BMP | own port | `image`'s BMP matched all 8 real files; kept own code for stb's header/mask edge cases (goes against D16 on a small scale) |
| PNG | `png` 0.18.1 | Pure Rust, lossless; stb's 8-bit RGBA conversions applied on top. No PNG in the packs, so only synthetic coverage |

Evidence lives in `crates/m2-formats/tests/crate_candidates.rs` (ignored tests;
`image` is a dev-dependency only). `cargo check -p m2-formats --lib --target
wasm32-unknown-unknown` passes. Android and iOS were not built, but nothing in
the crate needs a C compiler or system library.

Lesson for later: "reputable crate" and "identical to the original client"
conflict for pixel-exact codecs. The original decoders have quirks (stb
rounding, `<< 3` expansion) that every texture in the game depends on, so a
visually equivalent crate still changes pixels. D16 therefore only helps for
containers and lossless formats unless pixel-exactness is relaxed.

## 11. Status after the parallel wave (2026-11-01, evening)

All branches below are merged into `main`. The workspace compiles
(`cargo check --workspace --all-targets`). Tests were run for every crate except
`m2-script`/`m2-client`: everything passes except two `m2-assets` real-data tests.
The agents stopped at the usage limit, so several parts are **unfinished**.

| Crate | What it is | State |
|---|---|---|
| `m2-formats` (+ `utf8`) | Phase 1 formats; `encoding_rs` decoding, `glibc-tables` now an optional feature | done; `encoding_rs` equals the glibc port on all real text (§utf8 findings in `c++_rust_bugs.md`) |
| `m2-net` | packets (225 structs), stream framing, phase machine, fake-server tests | done, 15 tests; layouts/headers/sequence bytes/framing verified against C++ |
| `m2-render` | fixed-function Bevy material, golden render test | partly done: golden test passes (1 test); report not received, so supported-state table and tolerances are only in its `README.md` |
| `m2-viewer` | pack browser shell, orbit camera, texture preview, viewer registry | shell merged, 23 tests; not yet wired to `m2-assets`/`m2-render` |
| `m2-assets` | pack asset source, coords, textures, gr2 model+motion, spt, race loaders | **WIP**: 21 unit tests pass, 2 real-data tests fail (`bind pose mismatch`; non-UTF-8 name count is 0 where the test expects the CP949 names, likely because `utf8::virtual_name` now recovers them) |
| `m2-effect` | effect runtime simulation | **WIP**: unit + determinism tests pass (15); the C++ differential suite is committed but its result is unknown |
| `m2-ui-core` | window tree, text tags, slots | **WIP**: first compile only (3 unit tests); differential suite committed, result unknown |
| `m2-modgen` | Python module spec extractor and generator | **WIP**: real-build comparison for dbg/time/math/pack and conformance tests pass (14); `MODULES.md` report not confirmed |
| `m2-world` | heightfield/attribute queries, terrain meshes, splat | **WIP**: 6 unit tests pass; first map matched C++ in the differential, full run unknown |

Resuming: send each agent a message to continue (their worktrees are in
`.claude/worktrees/`, git-ignored). The Phase 2 gate (a skinned character playing
`.msa` motions in the viewer) still needs `m2-assets` finished and wired into
`m2-viewer` with the `m2-render` material.
