#!/usr/bin/env python3
"""Collect notices from the exact downloaded registry sources and npm packages."""
import json
import os
from pathlib import Path
import re
import shutil
root = Path(__file__).resolve().parent.parent
cargo = Path(os.environ.get('CARGO_HOME', Path.home()/'.cargo'))
output = root/'licenses'; output.mkdir(exist_ok=True)
rows = []
lock = (root/'src-tauri/Cargo.lock').read_text()
for package in lock.split('[[package]]')[1:]:
    name = re.search(r'^name = "([^"]+)"',package,re.M)
    version = re.search(r'^version = "([^"]+)"',package,re.M)
    if not name or not version: continue
    name, version = name[1], version[1]
    locations = list((cargo/'registry/src').glob(f'*/{name}-{version}'))
    if not locations: continue # Dependencies for other platforms are not distributed.
    source = locations[0]
    manifest = (source/'Cargo.toml').read_text()
    license_match = re.search(r'^license = "([^"]+)"',manifest,re.M)
    label = f'rust-{name}-{version}'; dest = output/label
    notices = [p for p in source.iterdir() if p.is_file() and p.name.upper().startswith(('LICENSE','LICENCE','COPYING','NOTICE','COPYRIGHT'))]
    if notices:
        dest.mkdir(exist_ok=True)
        for p in notices: shutil.copy2(p,dest/p.name)
    rows.append((name,version,license_match[1] if license_match else 'See upstream manifest',f'https://crates.io/crates/{name}/{version}',label if notices else ''))
for manifest in (root/'node_modules').glob('**/package.json'):
    if 'node_modules' in manifest.relative_to(root/'node_modules').parts[:-1] and len(manifest.relative_to(root/'node_modules').parts)>4: continue
    data=json.loads(manifest.read_text()); name=data.get('name',''); version=data.get('version','')
    if not name or not version: continue
    label=f'npm-{name.replace("/","-").replace("@","")}-{version}'; dest=output/label
    notices=[p for p in manifest.parent.iterdir() if p.is_file() and p.name.upper().startswith(('LICENSE','LICENCE','COPYING','NOTICE','COPYRIGHT'))]
    if notices:
        dest.mkdir(exist_ok=True)
        for p in notices: shutil.copy2(p,dest/p.name)
    rows.append((name,version,str(data.get('license','See upstream package')),f'https://www.npmjs.com/package/{name}/v/{version}',label if notices else ''))
text='# Dependency licenses\n\nExact downloaded dependencies for this Apple Silicon build. Cross-platform lockfile entries may not be distributed. Original license and notice files are preserved in `licenses/`. Apple system frameworks are provided by macOS.\n\n| Package | Version | Declared license | Original notices |\n|---|---|---|---|\n'
for name,version,license_name,url,label in sorted(set(rows)):
    notice=f'[files](licenses/{label}/)' if label else '[upstream]('+url+')'
    text+=f'| [{name}]({url}) | {version} | {license_name} | {notice} |\n'
(root/'DEPENDENCY_LICENSES.md').write_text(text)
print(f'Collected licenses for {len(rows)} downloaded packages.')
