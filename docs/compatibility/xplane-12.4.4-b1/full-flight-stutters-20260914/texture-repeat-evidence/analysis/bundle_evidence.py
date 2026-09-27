"""Retain replayable timing evidence and every excluded attempt after flying."""
from pathlib import Path
from datetime import datetime, timezone
import gzip
import hashlib
import json
import shutil

HERE = Path(__file__).resolve().parent
DOCS = Path('V:/src/xplane/docs/compatibility/xplane-12.4.4-b1/full-flight-stutters-20260914')
OUT = DOCS / 'texture-repeat-evidence'
ROOT = Path('D:/X-Plane 12/Output/performance-tests')
result = json.loads((HERE / 'results.json').read_text())
OUT.mkdir(parents=True, exist_ok=True)
files = []


def copy(source, target, compress=False):
    target.parent.mkdir(parents=True, exist_ok=True)
    digest = hashlib.sha256()
    size = 0
    with source.open('rb') as src:
        dest = gzip.open(target, 'wb', compresslevel=6) if compress else target.open('wb')
        with dest as dst:
            for chunk in iter(lambda: src.read(1024 * 1024), b''):
                digest.update(chunk)
                size += len(chunk)
                dst.write(chunk)
    files.append({'path': target.relative_to(OUT).as_posix(), 'source': str(source),
        'source_bytes': size, 'source_sha256': digest.hexdigest(),
        'bytes': target.stat().st_size, 'sha256': hashlib.sha256(target.read_bytes()).hexdigest(),
        'gzip': compress})


for pattern in ('*.json', '*.ps1', '*.py', '*.log', '*.csv', '*.png', '*.svg', '*.md'):
    for source in sorted(HERE.glob(pattern)):
        copy(source, OUT / 'analysis' / source.name)

for trial in result['trials'] + result['excluded_attempts']:
    name = trial['name']
    run = ROOT / name
    for filename in ('session.json', 'source-manifest.json', 'requested-config.json',
                     'resolved-config.json', 'restoration.json', 'flight-pacing.json',
                     'summary.json', 'report.md', 'Log.txt', 'worker-error.json',
                     'worker-error.log', 'runner-error.json', 'recovery-error.json',
                     'startup-ui-intervention.json', 'startup-menu-intervention.json'):
        source = run / filename
        if source.exists():
            copy(source, OUT / 'runs' / name / filename)
    for card in sorted((run / 'cards').glob('*')):
        for source in sorted(card.glob('*.json')):
            copy(source, OUT / 'runs' / name / 'cards' / card.name / source.name)
        for filename in ('pacing.csv', 'trace.csv'):
            source = card / filename
            if source.exists():
                copy(source, OUT / 'runs' / name / 'cards' / card.name / (filename + '.gz'), True)

manifest = {'utc': datetime.now(timezone.utc).isoformat(),
    'description': 'Six accepted two-flight texture trials and two excluded startup attempts. Original shutdown errors retained.',
    'raw_root': str(ROOT), 'files': files}
(OUT / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
print(json.dumps({'files': len(files), 'bytes': sum(f['bytes'] for f in files), 'directory': str(OUT)}, indent=2))
