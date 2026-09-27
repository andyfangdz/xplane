"""Is the touchdown error visible in ground energy on final, before short final?

At along-runway position X, E = h + GS^2/2g. Within each condition, compare
flights: does E(X) predict touchdown?
"""
import csv, glob, gzip, json, sys
from pathlib import Path
from collections import defaultdict
import numpy as np

G = 32.174; KT = 1.68781
X = float(sys.argv[1])
by = defaultdict(list)
for pattern in sys.argv[2:]:
    for card in sorted(glob.glob(pattern)):
        card = Path(card)
        tr = card / 'trace.csv.gz'
        if not tr.exists() and not (card / 'trace.csv').exists():
            continue
        m = json.loads((card / 'assessment.json').read_text())['metrics']
        if 'touchdown_ft' not in m:
            continue
        rows = csv.DictReader(gzip.open(tr, 'rt') if tr.exists() else open(card / 'trace.csv'))
        r = next((r for r in rows if int(float(r['phase_id'])) >= 6 and float(r['runway_along_ft']) >= X
                  and abs(float(r['runway_cross_ft'])) < 400), None)
        if r is None:
            continue
        e = float(r['agl_ft']) + (float(r['groundspeed_kt']) * KT) ** 2 / (2 * G)
        cond = card.name.rsplit('-', 1)[0].replace('_early', '').replace('_late', '')
        by[cond].append((e, m['touchdown_ft'], float(r['agl_ft']), card.name, card.parent.parent.name[-22:]))
slopes = []; resid = []; raw = []
for cond, v in sorted(by.items()):
    if len(v) < 4:
        continue
    e = np.array([a[0] for a in v]); t = np.array([a[1] for a in v])
    s = np.polyfit(e, t, 1)
    slopes.append(s[0]); resid += list(t - np.polyval(s, e)); raw += list(t - t.mean())
    print(f'{cond:22s} n {len(v):2d} touchdown sd {t.std():4.0f}  E sd {e.std():4.1f} ft  slope {s[0]:5.1f} ft/ft  residual sd {np.std(t - np.polyval(s, e)):4.0f}')
print(f'pooled within-condition: raw sd {np.std(raw):.0f} -> residual sd {np.std(resid):.0f}; median slope {np.median(slopes):.1f} ft per ft of energy')
