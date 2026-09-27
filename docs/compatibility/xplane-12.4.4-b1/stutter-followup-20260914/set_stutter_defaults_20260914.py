"""Apply the user's two persistent preferences, preserving all other bytes."""
from pathlib import Path
import hashlib
import json
import re
import xml.etree.ElementTree as ET
from datetime import datetime, timezone

out = Path('D:/X-Plane 12/Output/performance-tests/STUTTER_FOLLOWUP_20260914')
out.mkdir(exist_ok=True)
backup = out / 'settings-before'
backup.mkdir(exist_ok=False)
paths = [Path('D:/xOrganizer/XoData/XoData.xml'),
         Path('D:/xOrganizer/XoProfiles X-Plane preferences/default/X-Plane.prf'),
         Path('D:/X-Plane 12/Output/preferences/X-Plane.prf')]
changes = []
for i, path in enumerate(paths):
    old = path.read_bytes()
    saved = backup / f'{i}-{path.name}'
    saved.write_bytes(old)
    if i == 0:
        pattern = rb'(<Name>XpOtherArguments</Name>\s*<Preference>)([^<]*)(</Preference>)'
        matches = list(re.finditer(pattern, old))
        assert len(matches) == 1
        before = matches[0][2]
        assert b'--gfx_no_defragment' not in before
        after = before.rstrip() + b' --gfx_no_defragment'
        new = re.sub(pattern, lambda m: m[1] + after + m[3], old)
        assert ET.fromstring(new) is not None
    else:
        pattern = rb'(?m)^_tex_cache_gb [^\r\n]*'
        matches = list(re.finditer(pattern, old))
        assert len(matches) <= 1
        before = matches[0][0] if matches else b'(absent; default 5 GB)'
        after = b'_tex_cache_gb 10'
        newline = b'\r\n' if b'\r\n' in old else b'\n'
        new = re.sub(pattern, after, old) if matches else old + (b'' if old.endswith(b'\n') else newline) + after + newline
    changes.append({'path': str(path), 'backup': str(saved), 'before': before.decode(),
                    'after': after.decode(), 'before_sha256': hashlib.sha256(old).hexdigest(),
                    'after_sha256': hashlib.sha256(new).hexdigest()})
    tmp = path.with_name(path.name + '.codex-stutter-update')
    assert not tmp.exists()
    tmp.write_bytes(new)
    tmp.replace(path)
    assert path.read_bytes() == new
(out / 'settings-change.json').write_text(json.dumps({'utc': datetime.now(timezone.utc).isoformat(),
    'changes': changes}, indent=2))
print(json.dumps(changes, indent=2))
