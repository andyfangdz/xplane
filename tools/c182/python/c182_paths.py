"""Shared path contract for migrated Python asset/assembly helpers."""
import os
import tomllib
from pathlib import Path

ROOT = Path(os.environ.get('C182_WORKSPACE', Path.cwd())).resolve()
CONFIG = Path(os.environ.get('C182_CONFIG', ROOT / 'c182.local.toml')).resolve()
with CONFIG.open('rb') as f:
    SETTINGS = tomllib.load(f)

def configured(section, key):
    path = Path(SETTINGS[section][key])
    return path if path.is_absolute() else (CONFIG.parent / path).resolve()

def legacy(name, relative=''):
    return ROOT / 'local/legacy' / name / relative

def inputs(relative=''):
    return configured('paths', 'legacy_study') / relative

def source(relative=''):
    return ROOT / 'src' / relative

def asset(relative):
    generated = ROOT / 'generated/assets' / relative
    return generated if generated.exists() else inputs(relative)

def xplane(relative=''):
    return configured('paths', 'xplane') / relative

def addon(relative=''):
    return configured('paths', 'reference_addon') / relative

def output(kind, relative=''):
    result = configured('outputs', kind) / relative
    if not result.resolve().is_relative_to(ROOT):
        raise ValueError(f'Output must stay in this worktree: {result}')
    return result

def plugin():
    return Path(os.environ.get('C182_PLUGIN', ROOT / 'out/cargo/release/c182_plugin.dll')).resolve()
