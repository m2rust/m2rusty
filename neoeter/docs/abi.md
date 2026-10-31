# The Neoeter C ABI (`include/Neoeter/`)

The C ABI is the only supported boundary for code that is not compiled as part
of the engine. It is what makes a second script host (`neoeter-csharp`) possible
without forking the engine.

It does not exist yet. This document is the contract the first implementation
must follow; the header will be added package by package, starting with the
minimum a host needs to boot.

## Why a C ABI and not C++

The engine is 16 static C++17 libraries with no export surface. Any consumer
built with a different compiler, standard library or language cannot link
against that. The project is also built with two compilers (GCC on Linux,
MinGW-w64 on Windows), so "just use MSVC and C++/CLI" is off the table (D8).
C is the only ABI both toolchains produce identically.

## Rules

**Calling convention.** `extern "C"`, `__cdecl` (the only x86-64 convention on
Windows), `NEOETER_API` visibility macro. No name mangling, no exceptions, no
`std::` types across the boundary.

**Handles are opaque.**

```c
typedef struct neoeter_device neoeter_device;   /* never dereferenced by callers */
neoeter_result neoeter_device_create(const neoeter_device_desc* desc, neoeter_device** out);
void           neoeter_device_destroy(neoeter_device* device);
```

**Structs are POD, fixed width, explicitly padded.** `uint32_t`, `int32_t`,
`float`, `uint8_t`; no `long`, no `size_t`, no bitfields, no `bool`. Every
struct starts with `uint32_t struct_size` so fields can be appended within a
major version. `static_assert` in the header and in a C11 consumer test.

**Strings.** Input strings are NUL-terminated UTF-8. Output strings are
`(const char* data, uint32_t len)` and remain valid until the next call on the
same handle; anything that outlives the call is returned as a `neoeter_string`
with an explicit `neoeter_string_free`.

**Arrays.** `(const T* items, uint32_t count)`. The caller owns the memory;
the engine copies what it keeps.

**Errors.** `neoeter_result` enum (`OK`, `INVALID_ARGUMENT`, `NOT_FOUND`,
`IO`, `UNSUPPORTED`, `INTERNAL`). No `errno`, no exceptions. Optional
thread-local `neoeter_last_error()` for detail; it is diagnostic only, never
control flow.

**Ownership and lifetime.** Every `create` has a `destroy`; shared objects use
explicit `retain`/`release`. Handles are never freed by the caller.

**Threading.** Every call is main-thread-only unless the header says otherwise.
Cross-thread work is submitted to the engine's frame command queue; the engine
never invokes a foreign callback on a thread the host does not expect. Callbacks
carry `void* userdata` and must not call back into the engine re-entrantly.

**Versioning.** `NEOETER_ABI_VERSION_MAJOR/MINOR`. Within a major version only
additive changes: new functions, new trailing struct fields, new enum values
behind a capability query. A host that does not understand a capability must
still boot.

## First slice (the "host can boot" spike)

| Area | Functions |
|---|---|
| lifecycle | `neoeter_init`, `neoeter_shutdown`, `neoeter_version` |
| logging | `neoeter_log` + sink registration |
| time/frame | `neoeter_frame`, `neoeter_time_ms`, `neoeter_frame_index` |
| files | provider registration, `neoeter_file_open/read/close` |
| packs | open archive, enumerate, read entry |
| images | decode to a POD `neoeter_image` |
| device | create, resize, clear, present, `neoeter_readback` |
| scene | load model, add/remove instance, camera set, stats |

That is enough for a C# host to start, read a pack, load a model, render a
frame and exit — the same first smoke test the Python bridge has (`TA.6`).

## Conformance

- a C11 consumer compiled with both GCC and `x86_64-w64-mingw32-gcc` includes
  only `include/neoeter/*.h` and builds;
- `static_assert` size/offset tests for every struct;
- a deterministic ABI dump (`nm`/exports) checked in CI so accidental signature
  changes fail the build;
- the C# smoke test in `neoeter-csharp` uses the same header dump as the Python
  API snapshot does today.
