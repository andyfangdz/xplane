"""Prepare ignored generated inputs and forward their relative source includes."""
import json
import os
import re
import shutil
import hashlib
import tomllib
from pathlib import Path

ROOT = Path(os.environ.get('C182_WORKSPACE', Path.cwd())).resolve()

def main():
    manifest = ROOT / 'config/bootstrap-inputs.json'
    records = json.loads(manifest.read_text(encoding='utf-8'))
    config = tomllib.loads((ROOT / 'c182.local.toml').read_text(encoding='utf-8'))
    inputs = Path(config['paths']['legacy_study'])
    if not inputs.is_absolute(): inputs = ROOT / inputs
    for row in records:
        dest = ROOT / 'generated/bootstrap' / row['path']
        if not dest.is_file():
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(inputs / row['path'], dest)
        if hashlib.sha256(dest.read_bytes()).hexdigest() != row['sha256']:
            raise RuntimeError(f'Bootstrap input identity changed: {row["path"]}')
    generated = ROOT / 'generated/bootstrap'
    count = 0
    for header in list(generated.rglob('*.hpp')) + list(generated.rglob('*.h')):
        for include in re.findall(r'^\s*#include\s*"([^"]+)"', header.read_text(encoding='utf-8'), re.M):
            target = (header.parent / include).resolve()
            if target.is_file() or not target.is_relative_to(generated):
                continue
            source = ROOT / 'src' / target.relative_to(generated)
            if source.is_file():
                target.parent.mkdir(parents=True, exist_ok=True)
                relative = os.path.relpath(source, target.parent).replace('\\', '/')
                target.write_text(f'#pragma once\n#include "{relative}"\n', encoding='utf-8', newline='\n')
                count += 1
    print(f'Bootstrap headers ready; created {count} source forwarding headers.')

if __name__ == '__main__': main()
