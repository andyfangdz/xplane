"""Fit float distance (touchdown - roundout) for gust flights of the rate-commanded flare."""
import csv, json, math, sys
from pathlib import Path
import numpy as np
root = Path(r'D:\X-Plane 12\Output\performance-tests')
rows = []
for run in sys.argv[1:]:
    for card in sorted((root / run / 'cards').iterdir()):
        a = json.loads((card / 'assessment.json').read_text()); m = a['metrics']
        p = json.loads((card / 'effective-config.json').read_text())['parameters']
        if 'touchdown_ft' not in m or not m.get('roundout_entry_along_ft'):
            continue
        g = p['wind_gust_kt']; off = math.radians(p['wind_offset_deg'])
        head = p['wind_speed_kt'] * math.cos(off); cross = abs(p['wind_speed_kt'] * math.sin(off))
        travel = 680 - 11.7 * head - 0.28 * head * head - 0.9 * p['wind_speed_kt'] * math.sin(off)
        rows.append((m['touchdown_ft'] - m['roundout_entry_along_ft'], travel, m['roundout_entry_kias'], head, cross, g, card.name))
d = np.array([r[:6] for r in rows])
flt, travel, ro_kias, head, cross, g = d.T
print(f'n={len(d)} float mean {flt.mean():.0f} sd {flt.std():.0f}; steady travel model error mean {np.mean(flt - travel):+.0f} sd {np.std(flt - travel):.0f}')
for name, X in [('travel + gust', np.c_[travel, g]),
                ('travel + roundout kias', np.c_[travel, ro_kias]),
                ('head + roundout kias', np.c_[np.ones(len(d)), head, head * head, ro_kias]),
                ('head + cross + gust + ro kias', np.c_[np.ones(len(d)), head, head * head, cross, g, ro_kias])]:
    A = X if X.shape[1] > 2 and np.all(X[:, 0] == 1) else np.c_[np.ones(len(d)), X]
    coef, *_ = np.linalg.lstsq(A, flt, rcond=None)
    res = flt - A @ coef
    print(f'{name:32s} residual sd {res.std():5.0f}  coef {np.round(coef, 2)}')
