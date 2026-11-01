# C++ bugs found during the Rust port

A ledger of bugs in the C++ client that the port ran into, and what the Rust
side does about each one. Entries come from the module agents' reports and the
code comments in `crates/m2-formats`; the C++ line numbers are not recorded
because the C++ tree is about to be retired.

How to read it:

- **Replicated**: the Rust code reproduces the C++ behaviour on purpose,
  because it changes output on real data or on inputs the game can produce.
  Each has a comment in the Rust source and is covered by a differential or
  unit test. When the C++ is retired these can be fixed in Rust; until then they
  are what "identical to C++" means.
- **Not replicated**: the Rust code deliberately differs (usually undefined
  behaviour on malformed input, where mirroring is impossible or unsafe).
  Listed so the difference is never a surprise.
- **Latent**: real C++ bug that never triggers on the shipped data.

IDs are stable; add new entries at the end of a section. Keep the Rust location
as a module path, not a line number.

## 1. Replicated bugs

| ID | Module | C++ function | Bug | Rust handling |
|---|---|---|---|---|
| R1 | `locale` | `StringCodec` (iconv loop) | A character still held by the CP1255/CP1258 decoder at the end of input is lost: the iconv state is never flushed | Mirrored; `locale::iconv` |
| R2 | `locale` | `Ymir_MultiByteToWideChar` | With CP1258 there is no size query: an empty output buffer returns 0 | Mirrored |
| R3 | `locale` | `EL_String_*_Vietnamese` | `wchar_t` values from 0x80000000 up count as negative and are written as their low byte | Mirrored; `locale::vietnamese::encode` |
| R4 | `locale` | `Arabic_MakeShape` | After a LAM-ALEF ligature only one index is skipped, even when marks sat between the two letters; U+063B..U+063F become 0 | Mirrored; `locale::arabic` |
| R5 | `locale` | `ShiftJIS_StringCompareCI` | A double-byte character counts against the length only on the right-hand side | Mirrored; `locale::japanese` |
| R6 | `script` | `CMemoryTextFileLoader::SplitLine/SplitLine2` | Quote handling quirks: a `"` only matters at the start of a token; the token after a closing quote starts right after it even without a delimiter | Mirrored; `text::split_line2` |
| R7 | `script` | `CMemoryTextFileLoader::Bind` | A byte >= 0x80 starts a two-byte character and hides a following `\n`/`\r` from the line splitter | Mirrored; `text::lines` |
| R8 | `script` | `CTextFileLoader` | Duplicate keys with the same CRC-32: the first wins (`std::map::insert`); lookups compare only the 32-bit hash | Mirrored; `script::Node` |
| R9 | `spt` | seed in `GenerateGeometry` | `static_cast<unsigned int>(float)` of a negative value is undefined behaviour | Copies what x86-64 GCC produces; values >= 2^63 give 0 |
| R10 | `gr2` | `Gr2Decompress.c` | The literal window is chosen from the *buffer address* mod 4 (always 0 in practice, buffers are 16-aligned) | Mirrored as output offset mod 4; `gr2::decompress` |
| R11 | `gr2` | `Gr2Decompress.c` | `u16`/`u32` counters wrap silently | Wrapping arithmetic kept |
| R12 | `race` | `CRaceMotionData` event frames | Frame = float-to-`DWORD` truncation of `time / (1/60)`; `0.5 / (1.0f/60)` gives frame 29, not 30 | Mirrored; unit tests assert it |
| R13 | `race` | motion-event loaders | Results of event sub-loads are ignored, and a failed load can leave the script cursor inside a child group | Mirrored, with tests |
| R14 | `race` | `CPropertyManager::Initialize` | 14 property pairs share a CRC; which file wins depends on `unordered_map` iteration order | The C++ dumper prints its order and Rust replays it (`property::PropertyManager::initialize_in_order`) |
| R15 | `effect` | `CParticleSystemData` load | If `EmitterAdvancedType` is missing, `m_byEmitterShape` is reset to 0 instead of the advanced type | Mirrored |
| R16 | `effect` | `CGotoChild` | Always calls `SetParentNode`, so a missing group lets the cursor climb to the root (and the child count is re-read each iteration) | `Cursor` type reproduces it |
| R17 | `effect` | `CFlyingData::LoadScript` | The shared `temp` integer keeps its old value when `TailFlag` is missing | Mirrored |
| R18 | `effect` | `CParticleSystemData` | A particle with an unknown `MOVING_TYPE` skips `OnLoadScript` and keeps the positions read so far | Mirrored |
| R19 | `map` | `CMapOutdoor::LoadSetting` | Mis-tests whether the texture-set name already has its `textureset\` prefix: any name starting with t, e, x, u, r or s is used as is | Mirrored; `map::setting` |
| R20 | `map` | `CArea` rotation token | Drops the byte before the first `#` (`50#6#7` reads yaw 5) | Mirrored |
| R21 | `map` | `water.wtr` loader | Files with `u16` heights always fail, as do files with zero layers: the map is read twice. 844 of 1,189 real water files fail on both sides | Mirrored |
| R22 | `map` | `AtlasInfo.txt` `sscanf` | A failed scan keeps the previous line's values, so a blank line repeats the previous entry | Mirrored |
| R23 | `map` | `LoadArea`, `LoadTerrain` | Path buffers are cut to 64 bytes (except `AreaProperty.txt`) | Mirrored |
| R24 | `map` | `SetTerrainCount` | Ignores sizes of 0 or above 256 | Mirrored |
| R25 | `map` | `CTerrain::LoadHeightMap` | Computes normals even when reading the height file failed | Mirrored |

## 2. Not replicated: C++ bug, Rust differs

Real bugs where mirroring would be wrong or impossible.

| ID | Module | C++ function | Bug | Rust handling |
|---|---|---|---|---|
| N1 | `race` | `CRaceData::LoadRaceData` | **Shapes and hair never load as built.** `const DWORD dwShapeDataCount = 0` is written through `GetTokenDoubleWord`'s `const DWORD *`, so the compiler folds the loops away. Introduced by the clang-tidy commit (`bdcbc8e`, "clang tidy + format"). `.msm` files with `ShapeDataCount`/`HairDataCount` are silently ignored | Rust loads them as the code intends. The C++ dumper has a `race` mode with `const` removed (used by the tests) and `race-real` (as built: 0 shapes, 0 hairs). **This is a live regression in the C++ client, worth fixing there too** |
| N2 | `pack` | `PackInitialize` | The last line of `Index` is always skipped, so a file without a final newline loses its last pack | Rust reads an unterminated last line |
| N3 | `proto` | `CItemManager::Destroy` | Does not clear `m_vec_ItemRange`, which keeps dangling pointers across locale reloads | Rust starts fresh per load |
| N4 | `gr2` | animation loading | Each animation's track groups point into a shared vector that later animations reallocate: dangling pointer when a file has more than one animation | Each animation owns its track groups |
| N5 | `spt` | tree-cache normals loop | `unsigned short` counter: with more than 21,845 vertices the loop never ends | Normals are skipped (C++ never uses them) |
| N6 | `race` | `HitPosition` | Read in groups of seven numbers; reads past the end if the length is not a multiple of seven | Trailing partial group ignored |
| N7 | `race` | event-time map | NaN times are not ordered the way `std::map` would order them | Not reproduced |
| N8 | `locale` | `LoadInsultList` input / `Bind` | A file ending in a lone lead byte (>= 0x80) makes `Bind` read one byte past the buffer; `locale/hu/insult.txt` does this | `text::lines` clamps. The differential dumpers pad with NUL and print as C strings |
| N9 | `locale` | CP949 `A2 E8` at the end of input | glibc rejects the pair after consuming both bytes; the loop then skips one more byte past the end and converts whatever follows | Rust stops at the end; with a following byte it matches C++ |
| N10 | `crypt` | LZO decode | Unchecked `lzo1x_decompress` over-reads on corrupt streams and on headers whose sizes exceed the buffer | Bounds-checked; returns errors |
| N11 | `crypt` | `tea_decrypt` | Reads past the end for a partial last block | Zero-pads |
| N12 | `proto` | element counts | A proto file whose element count exceeds the decompressed data reads past the buffer | Rejected |
| N13 | `proto` | `RegisterSkillDesc` | Columns 2-12 read without a length check; a negative grade count writes before `GradeData` | Missing columns read as empty; only four grades written |
| N14 | `proto` | item name hash | `strlen` from `szName` continues into `szLocaleName` and the numeric fields | Stops at the end of `szLocaleName` |
| N15 | `gr2` | reader bounds checks | `off + 84 > size` and similar wrap at 32 bits, pass, then read out of bounds | No wraparound; record skipped |
| N16 | `gr2` | relocations / root-type names | Relocation into a section past the table indexes a fixup vector out of bounds; names read until NUL beyond their section | Treated as missing / bounded to the section |
| N17 | `gr2` | decompressor | Divides by zero (SIGFPE) on a zero weight total | Clean error |
| N18 | `gr2` | vertex data | Hands out a raw pointer; the engine reads `VertexCount` records from it regardless of section size | Decoded only as far as the section reaches |
| N19 | `effect` | `.mde` reader | Truncated buffer, out-of-range index or negative counts read out of bounds | `MeshError::Malformed`; element count above 65,536 refused; unterminated diffuse-map name cut at 128 bytes |
| N20 | `effect` | time-event / sound tables | An odd-length time-event list, a truncated `TimeEventPosition` (`std::out_of_range`) or a negative `SoundDataCount` (`std::length_error`) throws | `Error::Throws` (the dumper prints `throw`) |
| N21 | `race` | `SoundDataCount` | Below 0, aborts when the vector resizes | Treated as a failure |
| N22 | `race` | property reader | Data shorter than 5 bytes, or `get_string` on a key with no values, reads past the end | Returns `None` |
| N23 | `script` | `LoadMultipleTextData` | A `start` line without a name reads out of bounds | Empty name |
| N24 | `script` | `atof("nan(123)")` | glibc stores the payload in the NaN | Default quiet NaN, sign applied |
| N25 | `map` | token reads | Missing token values read out of bounds | Read as empty tokens |
| N26 | `map` | `TextureCount` | Below -1 is undefined | Empty set |
| N27 | `map` | default environment file | A map name shorter than 2 bytes throws | Whole name used |
| N28 | `map` | object sort | Order of equal-CRC objects is unspecified | Stable sort (file order) |
| N29 | `map` | `AtlasInfo.txt` | Lines before the first scanned name read an uninitialised buffer | Skipped |

## 3. Reference-platform choices (not bugs)

C++ behaviour that depends on the platform. The Rust port follows the Linux
build that runs here.

- `atoi`/`atof`: glibc semantics (saturating `strtol`, correctly rounded
  `strtod`, hex floats). MSVC differs on overflow and hex floats.
- Text conversion: glibc `iconv` tables (glibc 2.44), not Windows code pages.
  Differences exist for CP1252 0x81, GBK/Big5 and the CP1255/CP1258 combining
  logic.
- Float-to-integer casts: x86-64 GCC behaviour for out-of-range values.

## 4. How to add an entry

1. Add a row with the next free ID in section 1 or 2 (module, C++ function,
   what goes wrong, what Rust does).
2. Put a short comment at the Rust site naming the ID (`// C++ bug R12: ...`).
3. Make sure a test pins the behaviour: a differential test when it shows on
   real data, a unit test otherwise.
