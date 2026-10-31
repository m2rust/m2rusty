# Script hosts — Python today, C# tomorrow

## What exists today

The client embeds **CPython 2.7** in-process:

- `neoeter-python/src/ScriptLib` — `CPythonLauncher`: `Py_Initialize`,
  `Py_Finalize`, `RunFile`, `RunLine`, trace hook, `dbg`/`time`/`math` modules.
- `neoeter-python/src/EterPythonLib` — the UI object model (`wndMgr`, `grp*`,
  `wndSlot`, `wndText`).
- `client/src/UserInterface` — the game modules: `player`, `item`, `skill`,
  `chrmgr`, `chat`, `event`, `quest`, `guild`, `shop`, `safebox`, `exchange`,
  `net`, `snd`, `effect`, `miniMap`, `textTail`, `ime`, `systemSetting`, …
  (38 distinct `Py_InitModule` names, ~42k lines of binding code).

Every module is a flat C function table: `PyObject* f(PyObject* self, PyObject* args)`.
Game content (`bin/root/*.py`, `bin/pack/*.epk`) calls those modules by name.

That is the surface a C# host has to reproduce — not the C++ classes.

## What a host must provide

| Concern | Python today | C# equivalent |
|---|---|---|
| lifecycle | `CPythonLauncher::Create/Clear` | host init/shutdown at the same call site |
| module registration | `Py_InitModule("player", …)` | registry: name → managed module |
| call direction in | `PyObject*` C functions | `UnmanagedCallersOnly` entry points or a managed dispatcher |
| call direction out | direct C++ calls from the binding | `[LibraryImport]` over the C ABI (`docs/abi.md`) |
| strings | `char*` bytes (UTF-8-ish) | UTF-16 in managed, UTF-8 at the boundary |
| lifetime | `Py_INCREF`/`Py_DECREF` | `GCHandle` table + `SafeHandle`, explicit `retain`/`release` |
| threads | GIL, all on main thread | main-thread dispatcher; no managed callbacks under engine locks |
| errors | `PyErr_SetString` | `neoeter_result` + a managed exception adapter |
| reload | `RunFile` again | collectible `AssemblyLoadContext` |

## Three ways to host .NET

| | How | Pros | Cons |
|---|---|---|---|
| **A. CoreCLR in-process (recommended)** | engine loads `hostfxr` (`nethost` or `LoadLibrary`), gets `load_assembly_and_get_function_pointer`, calls `UnmanagedCallersOnly` entry points | modern .NET, one process, good perf, cross-platform, no MSVC | needs the .NET runtime deployed (self-contained or installed), hosting boilerplate |
| **B. NativeAOT library** | `dotnet publish -r <rid> /p:PublishAot=true` produces a native lib exporting `neoeter_csharp_*`; engine `dlopen`/`LoadLibrary`s it | simplest boundary: plain C calls, no hostfxr, fast startup | per-platform build artifacts, no runtime JIT/reload, larger binary |
| **C. C++/CLI or Mono embedding** | C++/CLI wrapper (Windows/MSVC) or `mono_jit_init` | C++/CLI has trivial marshalling; Mono runs on old OSes | C++/CLI breaks the MinGW/one-toolchain policy (D8); Mono is legacy |

**Recommendation: A first, B as a distribution option.** Both keep the engine
free of managed headers and preserve the Linux + Windows cross-build. C is only
relevant if Windows 7 support becomes a requirement.

## Repository and layers

`neoeter-csharp` is a fourth repository, a peer of `neoeter-python`:

```
neoeter-csharp/
├── src/
│   ├── Neoeter.Interop/     P/Invoke over include/neoeter/*.h (source-generated)
│   ├── Neoeter.Runtime/     handles, dispatcher, module registry, lifetime
│   ├── Neoeter.Packages/    gameplay module SDK (Player, Item, Skill, UI, …)
│   ├── Neoeter.Host/        hostfxr bootstrap / NativeAOT exports
│   └── Neoeter.Tools/       package manifest + builder (optional)
├── tests/
└── docs/
```

Dependency rule: `Host → Runtime → Interop`, `Packages → Runtime`. Nothing in
`neoeter` references .NET; the bridge is downstream, exactly like Python.

## How "packages in C#" would work

A **code package** is a .NET assembly implementing one or more module names and
declaring them in a manifest:

```csharp
[NeoeterModule("player")]
public sealed class PlayerModule : IGameModule
{
    public void Register(IModuleRegistry m) { m.Bind("SetMainCharacterIndex", SetMainCharacterIndex); }
    void SetMainCharacterIndex(uint index) => _engine.Player.SetMainCharacterIndex(index);
}
```

- `Neoeter.Packages` mirrors the existing Python module names one-for-one, so a
  content package can migrate module by module (`player` first, `quest` later)
  without touching the others.
- The engine loads assembly manifests at the same point it loads Python modules;
  a name registered by C# and a name registered by Python are mutually
  exclusive per module, which makes the migration incremental and testable.
- The existing **API snapshots** (`golden/api.sha256`, `TA.6`) become the
  conformance test: the C# module must expose the same names/signatures as the
  Python one before the Python one can be removed.
- Content packs (`* .epk`, `root/*.py`) are unchanged: they are assets, not
  code. A C# package ships as an assembly next to the pack and is loaded by
  manifest.

## What is realistic

- **Doable: yes.** Nothing in the three-repository split blocks it; the split is
  what makes it a clean fourth repository instead of a fork of the engine.
- **The hard part is not C#.** It is the missing C ABI (`docs/abi.md`). Once the
  ABI exists, a C# host is "just another consumer", and the ABI also serves
  Lua/Rust/mod tooling.
- **Scale.** ~42k lines of bindings and 38 modules. Porting all of them is a
  multi-month effort; porting `dbg`/`time`/`math` + one gameplay module is a
  spike of a few weeks on top of the ABI.
- **Coexistence is the safety valve.** Python 2.7 stays until the last module
  is ported. The bridge is additive, so the game keeps shipping during the
  migration.

### Suggested order

1. C ABI slice for boot/files/pack/image/device (`docs/abi.md`).
2. `neoeter-csharp` host + `Neoeter.Runtime` + a console smoke test that boots,
   opens a pack and renders one frame.
3. Port `dbg`, `time`, `math` (no game state).
4. Port `systemSetting` and one UI module (`wndMgr` subset) — proves the object
   model and the marshalling rules.
5. Port gameplay modules in dependency order (`player` → `chrmgr` → `item` →
   `skill` → …), each guarded by the API snapshot.
6. Decide whether Python is removed built-in, optional, or kept for content
   mods.

### Risks

- ABI drift: mitigated by the exported-symbol dump and the C11 conformance test.
- Marshalling cost: keep hot paths (per-frame getters) as small POD calls; batch
  state, do not call per vertex.
- GC pauses: no allocations in per-frame callbacks; pool handles.
- Double runtime: Python and CoreCLR in one process is fine, but startup order
  and shutdown matter (hosts are reference-counted, engine outlives both).
- Platform floor: modern .NET needs Windows 10+/glibc 2.17+; document it before
  committing to A.
