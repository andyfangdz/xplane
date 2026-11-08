"""Run existing visual hooks using maintained source and explicit scratch output."""
import runpy
import sys
import os
from pathlib import Path

ROOT = Path(os.environ.get('C182_WORKSPACE', Path.cwd())).resolve()
sys.path.insert(0, str(Path(__file__).resolve().parent))
for relative in ('src/visual-model', 'src/xplane-host', 'src'):
    sys.path.insert(0, str(ROOT / relative))

if __name__ == '__main__':
    runpy.run_path(str(ROOT / 'src/xplane-host/assemble_conversion_package.py'), run_name='__main__')
