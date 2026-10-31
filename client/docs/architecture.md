# Architecture — OpenMT2 client

## Role

The game, in the sense the engine is not: Metin2 data formats and the
application that turns content into a running client.

| Directory | Responsibility |
|---|---|
| `src/game` | item/race/skill tables, network packet structs, game-side data managers |
| `src/ui` | application loop, scene setup, the 38 Python game modules, `metin2client` |
| `src/webbrowser` | no-op stub keeping the UI's web-page entry points linkable |

Include roots: `<Client/Game/...>`, `<Client/UI/...>`,
`<Client/WebBrowser/...>`. Engine headers are `<Neoeter/...>`; the Python bridge
is `<Neoeter/Python/...>`.

## Dependency boundaries

```
client ──► neoeter-python ──► neoeter
   └──────────────────────────►
```

- the client may include engine and bridge headers through their public names;
- everything game-side is here; the engine and the bridge never include a
  client header. A change that needs that direction belongs in `neoeter` as a
  generic service.

## Module registration

`src/ui` owns the game modules. Each is a flat C function table
(`Py_InitModule("player", …)`, `"item"`, `"skill"`, `"chrmgr"`, `"quest"`, …),
registered after `neoeter-python` has created the interpreter. This is the seam
where a future C# host can take over one module at a time
(`../../neoeter/docs/scripting-hosts.md`).

## Assets

Not in this repository. `bin/` lives in the devkit (asset fetcher) and is
mounted/copied at dist assembly time. Code and content have different
provenance and licenses; keeping them apart is deliberate.

## Executable

`src/ui` defines the `metin2client` target, built from `src/ui/UserInterface.cpp`.
The devkit pins its binary directory to `build/src/ui`, which is what `build.sh`,
`smoke.sh` and the dist assembly use.
