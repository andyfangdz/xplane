"""Prepare pinned bootstrap inputs, and C++ forwarding headers for the oracle.

The production build needs only the pinned inputs. `--forwarding-headers` also
points synthetic headers at the archived native sources, which only the
C++ reference build of the oracle comparisons includes.
"""
import argparse
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


def copy_pinned():
    """Copy each pinned input into generated/bootstrap and check its identity."""
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
    return generated, pinned


def forward_headers(generated, pinned):
    """Write the oracle's forwarding headers; return how many changed."""
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
    return count


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--forwarding-headers', action='store_true',
                        help='also write the C++ forwarding headers the oracle build includes')
    args = parser.parse_args()
    generated, pinned = copy_pinned()
    if args.forwarding_headers:
        count = forward_headers(generated, pinned)
        print(f'Bootstrap inputs ready; updated {count} test-support forwarding headers.')
    else:
        print('Bootstrap inputs ready.')


if __name__ == '__main__':
    main()
