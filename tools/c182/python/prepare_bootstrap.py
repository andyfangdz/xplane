"""Prepare pinned inputs and C++ test-support forwarding headers."""
import json
import os
import re
import shutil
import hashlib
import tomllib
from pathlib import Path

ROOT = Path(os.environ.get('C182_WORKSPACE', Path.cwd())).resolve()


def native_header(relative):
    # Native aircraft code is test support after the Rust cutover. Python,
    # assets and the actual Lua vendor build intentionally keep their src root.
    for folder in ('tests/support/retired-aircraft-src', 'src'):
        source = ROOT / folder / relative
        if source.is_file():
            return source
    return None


def forward(target, source):
    relative = os.path.relpath(source, target.parent).replace('\\', '/')
    content = f'#pragma once\n#include "{relative}"\n'
    if target.is_file() and target.read_text(encoding='utf-8') == content:
        return 0
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(content, encoding='utf-8', newline='\n')
    return 1


def main():
    records = json.loads((ROOT / 'config/bootstrap-inputs.json').read_text(encoding='utf-8'))
    config = tomllib.loads((ROOT / 'c182.local.toml').read_text(encoding='utf-8'))
    inputs = Path(config['paths']['legacy_study'])
    if not inputs.is_absolute():
        inputs = ROOT / inputs
    generated = ROOT / 'generated/bootstrap'
    pinned = {row['path'] for row in records}
    for row in records:
        dest = generated / row['path']
        if not dest.is_file():
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(inputs / row['path'], dest)
        if hashlib.sha256(dest.read_bytes()).hexdigest() != row['sha256']:
            raise RuntimeError(f'Bootstrap input identity changed: {row["path"]}')
    count = 0
    for header in list(generated.rglob('*.hpp')) + list(generated.rglob('*.h')):
        relative = header.relative_to(generated).as_posix()
        content = header.read_text(encoding='utf-8')
        # Repoint existing synthetic stubs too; never rewrite a pinned input.
        if relative not in pinned and re.fullmatch(r'\s*#pragma once\s*\n\s*#include\s*"[^"\n]+"\s*', content):
            source = native_header(relative)
            if source is not None:
                count += forward(header, source)
                content = header.read_text(encoding='utf-8')
        for include in re.findall(r'^\s*#include\s*"([^"\n]+)"', content, re.M):
            target = (header.parent / include).resolve()
            if target.is_file() or not target.is_relative_to(generated):
                continue
            source = native_header(target.relative_to(generated))
            if source is not None:
                count += forward(target, source)
    print(f'Bootstrap headers ready; updated {count} test-support forwarding headers.')


if __name__ == '__main__':
    main()
