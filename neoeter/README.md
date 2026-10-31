# Neoeter

Engine (L0–L3) of the OpenMT2 client.

**Status:** consolidated engine tree (`TB.1`).

## Layout

```
include/
├── Neoeter/
│   ├── Platform/{Base,Locale,Math}/
│   ├── IO/  Image/  Render/  Audio/
│   ├── Model/{Grn,SpeedTree}/
│   └── Terrain/  Effect/
└── Platform/                 shared D3D9 compatibility shim
src/
├── platform/{base,locale,math}/
├── io/  image/  render/  audio/  terrain/  effect/
└── model/{grn,speedtree}/
```

Each directory holds the code directly; `platform` and `model` have more than
one package and therefore use subpackages. Tests live next to their code.

| Role | Target | Alias |
|---|---|---|
| platform/base | `neoeter_platform_base` | `Neoeter::Platform::Base` |
| platform/locale | `neoeter_platform_locale` | `Neoeter::Platform::Locale` |
| platform/math | `neoeter_platform_math` | `Neoeter::Platform::Math` |
| io | `neoeter_io` | `Neoeter::IO` |
| image | `neoeter_image` | `Neoeter::Image` |
| render | `neoeter_render` | `Neoeter::Render` |
| audio | `neoeter_audio` | `Neoeter::Audio` |
| model/grn | `neoeter_model_grn` | `Neoeter::Model::Grn` |
| model/speedtree | `neoeter_model_speedtree` | `Neoeter::Model::SpeedTree` |
| terrain | `neoeter_terrain` | `Neoeter::Terrain` |
| effect | `neoeter_effect` | `Neoeter::Effect` |

Headers are included as `<Neoeter/<Role>/...>`; the shared D3D9 shim stays
`<Platform/...>`.

Docs: `docs/architecture.md`, `docs/abi.md`, `docs/scripting-hosts.md`.
