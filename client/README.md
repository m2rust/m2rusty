# OpenMT2 client

Game layer: the Metin2 client libraries and the `metin2client` executable.

**Status:** consolidated.

## Layout

```
include/Client/
├── Game/            item/race/skill data, net packet structs
├── UI/              application, game modules, UI runtime
└── WebBrowser/      embedded web-page stub
src/
├── game/
├── ui/              metin2client (UserInterface.cpp), Cursors/, icon, manifest
└── webbrowser/
```

Each directory holds the code directly; tests sit next to their code.

| Path | Target | Alias |
|---|---|---|
| `src/game/` | `client_game` | `Client::Game` |
| `src/ui/` | `client_ui` | `Client::UI` |
| `src/webbrowser/` | `client_webbrowser` | `Client::WebBrowser` |

The executable is `build/src/ui/metin2client`. Headers are included as
`<Client/Game/...>`, `<Client/UI/...>`, `<Client/WebBrowser/...>`; engine and
Python-bridge headers keep their `Neoeter::*` roots.

Game assets (`bin/`) are devkit-owned and are not committed here. See
`docs/architecture.md`.
