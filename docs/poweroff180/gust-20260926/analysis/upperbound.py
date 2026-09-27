"""Within-condition predictability of outcome from state at successive path points."""
import csv, glob, sys
from collections import defaultdict
from pathlib import Path
import numpy as np

def point(rows, axis, value):
    for i, a in enumerate(rows):
        if a['cut_sim_time'] < 0 or int(a['phase_id']) < 4:
            continue
        if axis == 'y' and int(a['phase_id']) in (4, 5, 6) and a['runway_cross_ft'] <= value and int(a['phase_id']) >= 5:
            return a
        if axis == 'x' and int(a['phase_id']) >= 6 and abs(a['runway_cross_ft']) < 300 and a['runway_along_ft'] >= value:
            return a

data = defaultdict(list)
for pattern in sys.argv[1:]:
    for f in glob.glob(pattern):
        card = Path(f).parent
        cond = card.name.rsplit('-', 1)[0].replace('_early', '').replace('_late', '')
        rows = [{k: float(v) for k, v in r.items()} for r in csv.DictReader(open(f))]
        a = __import__('json').loads((card / 'assessment.json').read_text())['metrics']
        pts = {}
        for axis, v in [('y', 4000), ('y', 3000), ('y', 2000), ('x', -1200), ('x', -600), ('x', 0)]:
            p = point(rows, axis, v)
            pts[f'{axis}{v}'] = (p['agl_ft'], p['runway_along_ft'] if axis == 'y' else 0.0) if p else None
        data[cond].append((pts, a['roundout_entry_along_ft'], a['touchdown_ft']))

for key in ['y4000', 'y3000', 'y2000', 'x-1200', 'x-600', 'x0']:
    hd, xd, rod, tdd = [], [], [], []
    for cond, v in data.items():
        v = [x for x in v if x[0][key]]
        if len(v) < 3:
            continue
        h = np.array([x[0][key][0] for x in v]); xx = np.array([x[0][key][1] for x in v])
        ro = np.array([x[1] for x in v]); td = np.array([x[2] for x in v])
        hd += list(h - h.mean()); xd += list(xx - xx.mean()); rod += list(ro - ro.mean()); tdd += list(td - td.mean())
    hd, xd, rod, tdd = map(np.array, (hd, xd, rod, tdd))
    X = np.c_[hd, xd] if key.startswith('y') else hd[:, None]
    def r2(y):
        coef, *_ = np.linalg.lstsq(X, y, rcond=None); res = y - X @ coef
        return 1 - res.var() / y.var(), coef
    r2ro, cro = r2(rod); r2td, ctd = r2(tdd)
    print(f'{key:7s} n={len(hd)} height sd {hd.std():5.1f} ft | roundout within-sd {rod.std():4.0f}, R2 {r2ro:4.2f} | touchdown within-sd {tdd.std():4.0f}, R2 {r2td:4.2f} coef {np.round(ctd,1)}')

print('--- y3000 split')
hd, xd, tdd = [], [], []
for cond, v in data.items():
    v = [x for x in v if x[0]['y3000']]
    h = np.array([x[0]['y3000'][0] for x in v]); xx = np.array([x[0]['y3000'][1] for x in v]); td = np.array([x[2] for x in v])
    hd += list(h - h.mean()); xd += list(xx - xx.mean()); tdd += list(td - td.mean())
hd, xd, tdd = map(np.array, (hd, xd, tdd))
for name, X in [('height only', hd[:, None]), ('position only', xd[:, None]), ('both', np.c_[hd, xd])]:
    coef, *_ = np.linalg.lstsq(X, tdd, rcond=None); res = tdd - X @ coef
    print(f'{name:14s} R2 {1 - res.var() / tdd.var():4.2f} coef {np.round(coef, 2)}')
print(f'height sd {hd.std():.1f} ft, position sd {xd.std():.0f} ft, corr(height, position) {np.corrcoef(hd, xd)[0,1]:.2f}')
