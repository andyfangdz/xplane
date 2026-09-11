"""Check the native Shuttle HUD catalog against the project's public API contract."""
import argparse
import json
from pathlib import Path
import sys


def validate(catalog, contract):
    datarefs = catalog['datarefs']
    commands = catalog['commands']
    for name, expected in contract['datarefs'].items():
        if name not in datarefs:
            raise ValueError(f'Missing dataref: {name}')
        actual = datarefs[name]
        if actual['value_type'] != expected['value_type']:
            raise ValueError(f'Type mismatch: {name}')
        if bool(actual['is_writable']) != expected['is_writable']:
            raise ValueError(f'Writability mismatch: {name}')
    for name in contract['commands']:
        if name not in commands:
            raise ValueError(f'Missing command: {name}')
    return {
        'project': contract['project'],
        'expected_datarefs': contract['datarefs'],
        'native_catalog': datarefs,
        'commands': {name: commands[name] for name in contract['commands']},
        'additional': sorted(set(datarefs) - set(contract['datarefs'])),
        'passed': True,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--port', type=int, default=8144)
    parser.add_argument('--harness', type=Path, help='Directory containing the flight_test package')
    parser.add_argument('--catalog', type=Path, help='Validate a saved JSON catalog instead of connecting')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    contract = json.loads((Path(__file__).resolve().parents[1] / 'api-contract.json').read_text())
    if args.catalog:
        catalog = json.loads(args.catalog.read_text())
    else:
        if args.harness:
            sys.path.insert(0, str(args.harness))
        from flight_test.api import XPlaneApi
        with XPlaneApi(port=args.port, timeout=5) as api:
            api.refresh_catalogs()
            catalog = {
                'datarefs': {name: spec for name, spec in api.datarefs.items() if name.startswith('fsim_hud/')},
                'commands': api.commands,
            }
    result = validate(catalog, contract)
    args.output.write_text(json.dumps(result, indent=2) + '\n')
    print(f"Passed: {len(contract['datarefs'])} datarefs and {len(contract['commands'])} commands")


if __name__ == '__main__':
    main()
