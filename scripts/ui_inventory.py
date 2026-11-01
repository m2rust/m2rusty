#!/usr/bin/env python3
"""Read-only source audit. Never imports or evaluates the client Python code.
Run from the repository root: python3 scripts/ui_inventory.py
"""
import ast
import collections
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
PACK = ROOT / 'bin/pack'
OUT = ROOT / 'docs'
API = r'\b(net|player|item|skill|chat|quest|event|guild|party|messenger|shop|exchange|safebox|miniMap|wndMgr|app|systemSetting)\.([A-Za-z_]\w*)\s*\('

def read(path):
    return path.read_bytes().decode('utf-8', errors='replace')

def source(path):
    text = read(path)
    classes = []
    current = None
    for number, line in enumerate(text.splitlines(), 1):
        match = re.match(r'^(\s*)class\s+(\w+)(?:\((.*?)\))?:', line)
        if match:
            current = dict(name=match[2], bases=match[3] or '', line=number, methods=[])
            classes.append(current)
        match = re.match(r'^\s*def\s+(\w+)\(', line)
        if match and current:
            current['methods'].append(dict(name=match[1], line=number))
    result = dict(path=str(path.relative_to(ROOT)), classes=classes,
                  apis=sorted(set('.'.join(m) for m in re.findall(API,text))),
                  layouts=sorted(set(re.findall(r'[\"\']([^\"\'\n]*\.py)[\"\']', text, re.I))),
                  assets=sorted(set(re.findall(r'[\"\']([^\"\'\n]*\.(?:sub|tga|dds|jpg|png|ttf|otf))[\"\']',text,re.I))))
    result['widgets'] = []
    if '/ui/' in result['path'] or '/uiscript/' in result['path']:
        try:
            tree = ast.parse(text.expandtabs(4))
            expanded = text.expandtabs(4)
            for node in ast.walk(tree):
                if not isinstance(node, ast.Dict): continue
                fields = {k.value: v for k, v in zip(node.keys,node.values) if isinstance(k,ast.Constant) and isinstance(k.value,str)}
                if 'name' not in fields: continue
                result['widgets'].append(dict(line=node.lineno, fields={k:ast.get_source_segment(expanded,v) for k,v in fields.items() if k!='children'}))
        except SyntaxError as error:
            result['parse_error'] = str(error)
    return result

files = sorted(p for p in (ROOT/'bin').rglob('*') if p.is_file())
py = [source(p) for p in files if p.suffix.lower()=='.py' and (p.parent==PACK/'root' or '/uiscript/' in str(p) or re.search(r'/locale/[^/]+/ui/',str(p)))]
assets = []
virtual = {}
for p in files:
    if PACK not in p.parents or len(p.relative_to(PACK).parts)<2: continue
    name = '/'.join(p.relative_to(PACK).parts[1:]).lower()
    virtual.setdefault(name, []).append(str(p.relative_to(ROOT)))
for p in files:
    name = str(p.relative_to(ROOT))
    if p.suffix.lower() not in ('.sub','.tga','.dds','.jpg','.png','.ttf','.otf','.fnt','.bmp'): continue
    if '/ui/' not in name.lower() and '/icon/' not in name.lower() and p.suffix.lower() not in ('.ttf','.otf','.fnt'): continue
    entry = dict(path=name, bytes=p.stat().st_size)
    if p.suffix.lower()=='.sub':
        tokens = {}
        for line in read(p).splitlines():
            match = re.match(r'\s*(\w+)\s+"?(.*?)"?\s*$',line)
            if match: tokens[match[1].lower()] = match[2].lower()
        entry['sub'] = tokens
        image = tokens.get('image','').replace('\\','/')
        directory = '/'.join(p.relative_to(PACK).parts[1:-1]).lower()
        candidates = [directory+'/'+image] if tokens.get('version')=='2.0' else ['ymir work/ui/'+image,directory+'/'+image]
        entry['image_candidates'] = candidates
        entry['resolved_sources'] = next((virtual[c] for c in candidates if c in virtual), [])
    assets.append(entry)
cpp = []
for base in ('client/src/ui','neoeter/src/python','neoeter/src/render'):
    for p in sorted((ROOT/base).glob('*.cpp')):
        text = read(p)
        callbacks = sorted(set(re.findall(r'PyCallClassMemberFunc\([^;]*?"([A-Za-z_]\w*)"', text,re.S)))
        exports = sorted(set(re.findall(r'\{\s*"([A-Za-z_]\w*)"\s*,\s*\w+\s*,\s*METH_',text)))
        if callbacks or exports:
            cpp.append(dict(path=str(p.relative_to(ROOT)),callbacks=callbacks,exports=exports))
report = dict(bin_file_count=len(files),extensions=dict(collections.Counter(p.suffix.lower() for p in files)),
              python_sources=py,cpp_bindings=cpp,assets=assets,
              note='Static extraction only. Method membership for nested classes is approximate. Assets include unused/localized variants; no dynamic Python expression is evaluated.')
OUT.mkdir(exist_ok=True)
(OUT/'metin2_ui_sources.json').write_text(json.dumps(report,indent=2,ensure_ascii=True)+'\n')
md = ['# Metin2 UI inventory','',
      'Generated from the actual client by `scripts/ui_inventory.py` (static parsing; no client Python execution). The detailed, machine-readable inventory is [metin2_ui_sources.json](metin2_ui_sources.json). It contains every discovered UI controller class, layout dictionary and widget field expression, asset reference, C++ callback and exported binding. Discovery does not mean implementation.', '',
      '## Scope and resource audit','',
      f'- Recursive `bin/` scan: **{len(files):,} files**; **{len(py)} UI/support Python files**, **{len(assets):,} UI/icon/font assets**. Includes all locale layouts, seasonal variants and patched UI art, not only `uiscript/`.',
      '- Bevy is pinned to **0.20.1**. The game client and `m2-ui` no longer depend on egui. The separate asset-viewer utility still does; it is not part of the game client dependency graph.',
      '- Pack data is available in unpacked directories via `pack/Index.dev`. `m2-assets` mounts the existing pack reader at `m2://`; runtime does not import or execute Python.',
      '- `.sub` version 1 searches `d:/ymir work/ui/` before the file\'s own directory. Version 2 uses the file\'s directory. Source: `neoeter/src/render/GrpSubImage.cpp:74`. See asset records for exact region coordinates and resolved parent images.',
      '- UI locale currently follows the game\'s `locale/en` configuration. Shipped locale variants are all inventoried and must not be mistaken for missing duplicate systems.',
      '', '## Implementation status', '',
      'See [metin2_ui_validation.md](metin2_ui_validation.md) for current implementation and verification results. All rows below are **Discovered** unless that validation document explicitly records stronger coverage. Native login/select and preliminary HUD existed before the comprehensive source audit; they are not visually verified ports.', '',
      '## Controllers and dynamically created UI', '',
      '| UI identifier | Python source | Inheritance | Triggers / interactions | Data / packet APIs | Status |',
      '|---|---|---|---|---|---|']
for s in py:
    for c in s['classes']:
        methods=', '.join(m['name'] for m in c['methods'] if re.search(r'Open|Close|Show|Hide|On|Select|Accept|Cancel|Refresh|Use|Over|Drop',m['name']))
        md.append(f"| `{c['name']}` | `{s['path']}:{c['line']}` | `{c['bases']}` | {methods or 'See source methods in JSON'} | {', '.join(s['apis']) or 'Local UI state'} | Discovered |")
md += ['', '## Every layout file', '', '| Layout source | Window/widget names | Layout, assets and interactions | Status |','|---|---|---|---|']
for s in py:
    if not s['widgets'] and '/root/' in s['path']: continue
    names = [w['fields'].get('name','?').replace('"','').replace("'",'') for w in s['widgets']]
    md.append(f"| `{s['path']}` | {', '.join(names)} | {len(s['widgets'])} dictionaries; exact x/y/width/height/align/image/button state/slot/input expressions in JSON. {s.get('parse_error','')} | Discovered |")
md += ['', '## C++ to Rust integration map', '',
       '| Original subsystem | Rust boundary | Current coverage |','|---|---|---|',
       '| `PythonWindow`, `PythonWindowManager`, `ui.py` | `m2-ui` native nodes, input, window manager | See validation; no Python binding emulation |',
       '| `PythonNetworkStreamPhaseLogin/Select/Loading` | `m2-game::net`, `m2-net::phase`, `m2-ui::login/select/frontend` | Login/select transport exists; creation/deletion/empire UI pending |',
       '| `PythonPlayer`, `PythonItem`, `PythonNetworkStreamPhaseGameItem` | Rust player state + typed inventory actions | Server packets decoded; widget integration under development |',
       '| `PythonChat`, `PythonMessenger`, `PythonGuild`, `PythonExchange`, `PythonShop`, `PythonSafeBox`, `PythonQuest`, `PythonMiniMap` | `NetMessage::Game` and typed game resources | Most domain UI remains unimplemented; decoded packet availability is not UI coverage |',
       '', '## Asset and binding accounting', '',
       'The JSON lists every matching UI/icon/font asset, its physical source, file size, `.sub` metadata and parent image candidates. It also lists all exported Python API names and server-to-Python callbacks found in the scanned C++ files. Missing dynamic/expression resolution is recorded as a gap, not represented as a completed port.', '',
       '## Completion gate', '',
       'Migration is **not complete**. Each controller/layout still needs a Rust owner, exact visual/interaction mapping and verification. Do not mark a row implemented merely because a generic layout tree renders. The validation document separates visuals, interactions, networking and live verification.']
(OUT/'metin2_ui_inventory.md').write_text('\n'.join(md)+'\n')
print(f'Audited {len(files)} files, {len(py)} Python UI/support sources, {sum(len(s["classes"]) for s in py)} classes, {sum(len(s["widgets"]) for s in py)} widget dictionaries, {len(assets)} assets, {len(cpp)} C++ binding files.')
