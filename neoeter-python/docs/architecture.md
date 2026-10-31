# Architecture — Neoeter Python

## Role

| Directory | Responsibility |
|---|---|
| `src/host` | owns the interpreter: `Py_Initialize`/`Py_Finalize`, `RunFile`, `RunLine`, trace hook, `dbg`/`time`/`math`, error text |
| `src/bindings` | typed Python wrappers for the renderer's UI objects: `wndMgr`, `grpImage`, `grpText`, `grpThing`, `wndSlot`, `wndText` |

Include roots: `<Neoeter/Python/Host/...>` and
`<Neoeter/Python/Bindings/...>`. The engine is consumed only through its
`Neoeter::*` targets and `<Neoeter/...>` headers.

The game modules (`player`, `item`, `skill`, `chrmgr`, `quest`, …) are not here;
`client/src/ui` registers them through `Py_InitModule` after the interpreter is
up. That is the seam that keeps the bridge a boundary instead of a monolith.

## Host contract

Any script host — Python today, C# in the future — must do exactly these things
in this order:

1. **Start** after the engine device exists and before the first frame.
2. **Register modules** by name (`player`, `item`, …), each a set of callable
   symbols. Name collisions are errors; a module is owned by exactly one host.
3. **Run the boot script** (`root/uibla.py` and friends) on the main thread.
4. **Pump per frame** on the same thread, without holding engine locks across
   the call.
5. **Shut down** before the device is destroyed; release all host objects.

Errors never cross the boundary as exceptions: the host converts them to a
status + message (the C ABI does this with `neoeter_result`,
`../../neoeter/docs/abi.md`).

## Version policy

CPython **2.7** is deliberate (`D3`): the content and the existing modules are
2.7 code, and the Windows build vendors PSF's `python27.dll` + a `dlltool`
import library. This repository is the only place that knows about `Python.h`.

A second host does not change that: `neoeter-csharp` is a sibling repository
that consumes the same engine and registers the same module names. The C#
integration plan is in `../../neoeter/docs/scripting-hosts.md`.

## Tests that keep the boundary honest

- headless interpreter boot (`TA.6`);
- API-surface snapshots for `host`, `bindings` and `client`, hashed in
  `golden/api.sha256`;
- the full client suite (`49` CTest cases).
