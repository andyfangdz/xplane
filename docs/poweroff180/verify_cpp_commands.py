"""Compare saved Rust-flight commands with a compiled frozen C++ v7 generator.

The generator is an optional offline audit tool, not a production dependency.
See crates/poweroff180/tests/reference/README.md for compilation instructions.
"""
import argparse
import csv
import gzip
import hashlib
import io
import json
from pathlib import Path
import struct
import subprocess
import tempfile

FIELDS = ['phase_id', 'reason_id', 'bank_command', 'pitch_command', 'flap_command',
          'throttle_command', 'turn_lead_ft', 'desired_vertical_fps', 'vertical_accel_fps2',
          'wind_pitch_rate_ff', 'roundout_pitch_rate_command', 'control_dt_s', 'cut_sim_time',
          'roundout_sim_time', 'entry_gate_s', 'native_steps', 'predicted_cross_ft', 'cross_accel_fps2']
HERE = Path(__file__).resolve().parent


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--generator', required=True, type=Path,
                        help='Compiled crates/poweroff180/tests/reference/generate.cpp executable')
    args = parser.parse_args()
    generator = args.generator.resolve(strict=True)
    results = []
    with tempfile.TemporaryDirectory(prefix='poweroff180-cpp-replay-') as temporary:
        directory = Path(temporary)
        for card in sorted((HERE / 'verification' / 'release').glob('*/cards/*')):
            trace = gzip.decompress((card / 'trace.csv.gz').read_bytes())
            (directory / 'trace.csv').write_bytes(trace)
            binary = directory / 'expected.bin'
            subprocess.run([str(generator), str(card / 'native-effective.ini'), str(directory / 'trace.csv'), str(binary)],
                           capture_output=True, text=True, check=True)
            raw = binary.read_bytes()
            rows = list(csv.DictReader(io.StringIO(trace.decode('utf-8'))))
            if len(raw) != len(rows) * 216:
                raise RuntimeError(f'C++ record count differs: {card}')
            for frame, row in enumerate(rows):
                expected = struct.unpack_from('<18d', raw, frame * 216 + 72)
                for field, value in zip(FIELDS, expected):
                    if struct.pack('<f', float(row[field])) != struct.pack('<f', value):
                        raise RuntimeError(f'Guidance differs: {card.name}, frame {frame}, field {field}')
            results.append({'campaign': card.parents[1].name, 'card': card.name,
                            'frames': len(rows), 'f32_bit_mismatches': 0,
                            'trace_sha256': hashlib.sha256(trace).hexdigest()})
    if len(results) != 14:
        raise RuntimeError('Expected all 14 release traces')
    print(json.dumps({'flights': len(results), 'frames': sum(row['frames'] for row in results),
                      'outputs_per_frame': len(FIELDS), 'f32_bit_mismatches': 0,
                      'generator_sha256': hashlib.sha256(generator.read_bytes()).hexdigest(),
                      'trials': results}, indent=2))


if __name__ == '__main__':
    main()
