# Neoeter Python

CPython 2.7 host and Python bindings for the OpenMT2 client.

**Status:** consolidated.

## Layout

```
include/
├── Neoeter/Python/Host/        CPython host API
├── Neoeter/Python/Bindings/    UI object model (wndMgr, grp*, wndSlot, wndText)
└── python2.7/                  vendored headers (Linux)
src/
├── host/                       launcher, dbg/time/math
└── bindings/                   window/graphic bindings
```

Each directory holds the code directly; tests sit next to their code.

| Path | Target | Alias |
|---|---|---|
| `src/host/` | `neoeter_python_host` | `NeoeterPython::Host` |
| `src/bindings/` | `neoeter_python_bindings` | `NeoeterPython::Bindings` |

Headers are included as `<Neoeter/Python/Host/...>` and
`<Neoeter/Python/Bindings/...>`. The engine is consumed through its
`Neoeter::*` targets and `<Neoeter/...>` headers.

See `docs/architecture.md`.
