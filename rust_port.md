# Rust / Bevy port: progress, findings and decisions

Living log of the port described in [docs/bevy-port-plan.md](docs/bevy-port-plan.md).
The plan says *what we intend*; this file says *what is done, what we learned
and what we decided*. Update it as work lands.

Last updated: 2026-11-01.

## 1. Status

| Phase | State |
|---|---|
| 0. Groundwork | done (`1b5b551`) |
| 1. Formats, no Bevy | in progress: 11 of 12 format families merged; `image` running |
| 2. Asset viewer in Bevy | not started |
| 3. World | not started |
| 4. Game logic and networking | not started |
| 5. Python bridge and UI | not started |
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
| `effect` | `.mse`, particles, `.mde` meshes, flying data `.msf` | 1,148 `.mse`, 98 `.msf`, 112 `.mde` (1.57 M lines) | merged |
| `image` | DXT codecs, DDS/TGA/JPG (`neoeter/src/image`) | | agent running |

Current totals on `main`: 131 tests pass, `cargo clippy -p m2-formats
--all-targets` is clean, and nine differential suites pass
(`scripts/differential.sh`).

## 2. What exists

- **Cargo workspace** at the repo root (`Cargo.toml`, `crates/*`). Bevy is
  pinned to `=0.20.1`. Dev builds optimise dependencies (`opt-level = 3`).
- **`crates/m2-script`**: `build.rs` builds `neoeter_python_abi` (shared C
  library over the CPython 2.7 host) with the repo's CMake and links it. A safe
  `PythonHost` wrapper (one per process, main thread only). `cargo test -p
  m2-script` boots the interpreter headless and runs `dbg`/`time`/`math`.
- **`crates/m2-client`**: minimal Bevy window with the Python host running
  beside it. Runs; not yet the real client.
- **`crates/m2-formats`**: pure Rust, no Bevy dependency. Names and contents are
  bytes, never `String`, because the data is CP949/CP1252.
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

## 3. Findings

### Architecture
1. **Python owns the main loop, so Bevy must run inside `app.Loop`.**
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
| D1 | Python 2.7 and the 38 module shims stay in C++ | PyO3 does not support 2.7; this was the brief |
| D2 | `m2-script` links one shared library (`neoeter_python_abi`) | Avoids Rust having to link the engine's static archives and their system deps |
| D3 | Pin Bevy `=0.20.1` | Avoid churn mid-phase; isolate Bevy behind `m2-render`/`m2-assets` later |
| D4 | Rust ports must match C++ *byte for byte on real data*, proven by differential tests | The only way to port 100k+ lines of binary/text format code without regressions |
| D5 | Names and contents are `&[u8]`/`Vec<u8>` | Data is CP949/CP1252 |
| D6 | Reference = Linux C++ build (glibc `atoi`/`atof`/`iconv`) | That is what builds here; MSVC would differ on `atoi` overflow, hex floats, some code pages |
| D7 | Malformed input returns an error, never panics; C++ undefined behaviour is *not* mirrored | Several C++ readers over-read or divide by zero on corrupt files; deviations are documented per module |
| D8 | Real C++ bugs are mirrored when they affect output on real data, with a comment | e.g. Arabic shaping skips one index after LAM-ALEF; CP1255/CP1258 decoder loses its held character at end of input |
| D9 | Dependencies pinned per crate, exact versions in `crates/m2-formats/Cargo.toml`; `zip` uses `deflate-flate2-zlib-rs` (pure Rust) | Keep the format crate free of C builds |
| D10 | Bevy runs inside `app.Loop` (§3.1) | Keeps `prototype.py` unmodified |
| D11 | Differential dumpers live in `tools/differential/` (opt-in `M2_BUILD_DIFF_TOOLS`), not in the engine tree | Keeps test tooling out of the C++ libraries |
| D12 | Parallel module ports run as subagents in git worktrees, one module each, merged by hand | Modules are independent once `script` exists |
| D13 | Not ported: `parser.cpp` (`script::Group`), skill icon images, the Vietnamese weapon-vnum conversion, the LZO compressor, CRC32 (no loader uses it), `Arabic_ConvEnglishModeSymbol` | Unused or only in other builds |
| D14 | **UTF-8 everywhere it is possible** (user decision, 2026-11-01). New Rust APIs (Phase 2 onwards) expose text as `String`/`&str`. Legacy code pages are converted to UTF-8 once, at the file boundary; other encodings are not a goal and are not preserved on output | The user does not care about non-UTF-8 encodings. Phase 1 modules stay byte-oriented because they are proven byte-identical to C++; a UTF-8 façade is added on top, not a rewrite |
| D15 | C++ bugs that are replicated or deliberately not replicated are tracked in [c++_rust_bugs.md](c++_rust_bugs.md) | One ledger, stable IDs, referenced from Rust comments |

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

## 6. Open items and risks

- **Phase 1 not finished:** race/property, map and effect are in flight
  (agents resumed after a usage limit); `image` (DXT, DDS/TGA/JPG, stb) has no
  owner yet.
- **No `rustfmt.toml`:** `cargo fmt` would reformat every module. Agents format
  their own files with `max_width = 120, use_small_heuristics = "Max"`. Add that
  as a config before the next big merge.
- **Windows text behaviour:** the locale port reproduces glibc; if Windows
  behaviour matters, regenerate the 320 KB code-page tables
  (`m2diff_locale tables`) from a Windows reference.
- **Assets are not in the repo:** every differential gate needs the client in
  `bin/` (git-ignored). CI needs a way to mount it.
- **Windows/MinGW cross-build** of the Rust client and Python 2.7 linking is
  unverified (plan §7).
- **First Bevy compile is slow** (~11 min); later builds are incremental.
- **Python's Ctrl+C handler** pre-empts Bevy's; revisit in Phase 5.
- **Differential suite location:** it needs `build/differential` in the
  checkout it runs from; agents' worktrees each build their own copy.

## 7. How to run things

```bash
nix-shell                                  # everything below runs inside it
cargo test -p m2-script                    # boots the Python host headless
cargo test -p m2-formats                   # unit + fixture tests
cargo clippy -p m2-formats --all-targets
scripts/differential.sh                    # Rust vs C++ on bin/ (slow: ~1 min per suite)
scripts/differential.sh bin gr2            # one suite
cargo run -p m2-client                     # Bevy window + Python host
```

## 8. Next steps

1. Land `image` (race/property, map and effect are merged); re-run all suites.
2. Port `image` (DXT, DDS/TGA/JPG) with the existing `test_dxt_invariants`
   cases; close Phase 1.
3. Add `rustfmt.toml` and run it once on the whole crate.
4. Phase 2: `m2-assets` (pack `AssetSource`, loaders for textures, gr2, spt),
   `m2-render` materials checked against `neoeter/src/render/tests/golden`.

## 9. UTF-8 policy (D14): what it means for the code

- **Phase 1 stays as is.** `pack`, `script`, `race`, … keep `Vec<u8>` names and
  tokens. That is what makes them verifiable byte for byte, and changing it now
  would throw the comparison away. 31 file names in `bin/pack` are CP949 bytes.
- **New code is UTF-8.** `m2-assets`, `m2-ui`, `m2-game` and the script bridge
  take and return `&str`/`String`. Conversion from the legacy code page
  happens once, where bytes enter from a pack or from the network.
- **Decide at the boundary, not per call site.** Candidates: decode names in
  `PackManager` into UTF-8 virtual names (the Python scripts and the proto
  tables must then agree on the same spelling), and decode `proto`/locale
  text when it is loaded.
- **`locale` can shrink.** It currently reproduces glibc's `iconv` tables for 20
  code pages (320 KB of tables) to be byte-identical with C++. Under D14 that
  fidelity is no longer needed; a later cleanup can replace it with
  `encoding_rs` for the few locales actually shipped and drop the tables and
  the Arabic/Japanese/Vietnamese reproduction that is not needed for display.
  Nothing depends on this today, so it is deferred.
- **Open question for Phase 5:** Python 2.7 `str` is bytes. The scripts and
  the C++ modules pass text in the client's code page. The shim layer converts
  to and from UTF-8 at the boundary; which source encoding the shipped
  `bin/pack/root/*.py` files use has not been checked yet.
