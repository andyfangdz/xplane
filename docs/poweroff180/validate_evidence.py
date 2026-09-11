"""Reassess the retained Rust migration flights and verify their source hashes.

Run with tools/flight-test-harness/.venv/Scripts/python.exe after Bootstrap.ps1.
No simulator process or installation mutation is involved.
"""
from collections import defaultdict
import csv
import gzip
import hashlib
import json
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
EVIDENCE = HERE / 'verification'
HARNESS = REPO / 'tools' / 'flight-test-harness'
sys.path.insert(0, str(HARNESS))
from xpt.analysis import assess
from xpt.report import aligned, divergence


def read(path):
    return json.loads(path.read_text(encoding='utf-8-sig'))


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def main():
    hashes = read(EVIDENCE / 'evidence-sha256.json')
    for relative, expected in hashes.items():
        path = EVIDENCE / relative
        require(path.is_file(), f'Missing evidence: {relative}')
        require(hashlib.sha256(path.read_bytes()).hexdigest() == expected,
                f'Evidence changed: {relative}')

    groups = defaultdict(list)
    trials = []
    campaigns = []
    reference_sources = None
    runtime_replay_frames = 0
    for campaign in sorted((EVIDENCE / 'release').iterdir()):
        restoration = read(campaign / 'restoration.json')
        require(restoration['restored'] and restoration['source_hashes_match'],
                f'Unverified restoration: {campaign.name}')
        source_hashes = read(campaign / 'source-manifest.json')['harness_sha256']
        if reference_sources is None:
            reference_sources = source_hashes
        require(source_hashes == reference_sources, f'Release source changed: {campaign.name}')
        for relative, expected in source_hashes.items():
            if relative.startswith('build/'):
                continue  # Recorded binary hashes are retained; a fresh clone builds its own.
            path = REPO / relative[10:] if relative.startswith('workspace/') else HARNESS / relative
            require(path.is_file() and hashlib.sha256(path.read_bytes()).hexdigest() == expected,
                    f'Release source differs: {relative}')
        for card in sorted((campaign / 'cards').iterdir()):
            result = read(card / 'result.json')
            with gzip.open(card / 'trace.csv.gz', 'rt', encoding='utf-8') as stream:
                rows = [{key: float(value) for key, value in row.items()} for row in csv.DictReader(stream)]
            actual = assess(result, rows)
            require(actual == read(card / 'assessment.json'), f'Assessment differs: {campaign.name}/{card.name}')
            require(actual['measurement_valid'] and actual['passed'], f'Failed release flight: {campaign.name}/{card.name}')
            replay = read(card / 'cpp-guidance-replay.json')
            require(replay['frames'] == len(rows) and replay['outputs_per_frame'] == 18
                    and replay['f32_bit_mismatches'] == 0,
                    f'C++ runtime replay failed: {campaign.name}/{card.name}')
            require(hashlib.sha256(gzip.decompress((card / 'trace.csv.gz').read_bytes())).hexdigest() == replay['trace_sha256'],
                    f'C++ replay trace differs: {campaign.name}/{card.name}')
            require(hashlib.sha256((card / 'native-effective.ini').read_bytes()).hexdigest() == replay['config_sha256'],
                    f'C++ replay configuration differs: {campaign.name}/{card.name}')
            for relative, expected in replay['reference_source_sha256'].items():
                path = REPO / 'crates' / 'poweroff180' / 'tests' / 'reference' / relative
                require(hashlib.sha256(path.read_bytes()).hexdigest() == expected,
                        f'C++ replay reference changed: {relative}')
            runtime_replay_frames += replay['frames']
            name = result['effective_config']['card_name']
            groups[name].append(aligned(rows))
            trials.append({'campaign': campaign.name, 'card': card.name, 'wind': name, **actual})
        campaigns.append({'name': campaign.name, 'restoration': restoration})

    require(len(trials) == 14 and len(groups) == 7 and all(len(rows) == 2 for rows in groups.values()),
            'Expected 14 release flights, two for each of seven winds')
    comparisons = {name: divergence(rows) for name, rows in sorted(groups.items())}
    diagnostic_trials = []
    for category in ['diagnostics', 'recovery']:
        for campaign in sorted((EVIDENCE / category).iterdir()):
            require(read(campaign / 'restoration.json')['restored'], f'Recovery incomplete: {campaign.name}')
            for card in sorted((campaign / 'cards').glob('*')):
                if not (card / 'trace.csv.gz').exists():
                    continue
                with gzip.open(card / 'trace.csv.gz', 'rt', encoding='utf-8') as stream:
                    rows = [{key: float(value) for key, value in row.items()} for row in csv.DictReader(stream)]
                assessment = assess(read(card / 'result.json'), rows)
                require(assessment == read(card / 'assessment.json'), f'Diagnostic assessment differs: {campaign.name}/{card.name}')
                diagnostic_trials.append({'category': category, 'campaign': campaign.name,
                                          'card': card.name, 'passed': assessment['passed'],
                                          'measurement_valid': assessment['measurement_valid'],
                                          'reasons': assessment['reasons']})
    summary = {'schema_version': 1, 'flights': len(trials), 'strict_passes': len(trials),
               'measurement_valid': len(trials), 'campaigns': campaigns, 'comparisons': comparisons,
               'trials': trials, 'diagnostic_trials': diagnostic_trials,
               'recorded_guidance_cpp_frames': runtime_replay_frames,
               'recorded_guidance_f32_bit_mismatches': 0,
               'source_files_verified': len(reference_sources) - 3, 'evidence_files_verified': len(hashes)}
    print(json.dumps(summary, indent=2))


if __name__ == '__main__':
    main()
