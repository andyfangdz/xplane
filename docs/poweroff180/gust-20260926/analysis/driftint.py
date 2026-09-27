"""Does the integrated gust drift (measured wind - expected mean wind, since the
cut) predict touchdown scatter within a condition?

Path shortening L = along-runway drift toward the threshold + cross drift
toward the centerline (the downwind side has y > 0 and base flies toward y = 0).
"""
import csv, glob, json, math, sys
from collections import defaultdict
from pathlib import Path
import numpy as np
RWY = math.radians(209.515)
KT = 1.68781

def drift_until(rows, cfg, stop):
    g = cfg['wind_gust_kt']; off = math.radians(cfg['wind_offset_deg'])
    mean = cfg['wind_speed_kt'] + 0.35 * g
    # Expected wind blows *toward* runway heading + offset + 180 in the runway frame.
    mean_along = -mean * math.cos(off); mean_cross = -mean * math.sin(off)
    dx = dy = 0.0; prev = None
    for r in rows:
        if r['cut_sim_time'] < 0:
            continue
        if prev is not None:
            dt = r['sim_time'] - prev
            w = r['wind_speed_mps'] / 0.3048
            rel = math.radians(r['wind_direction_true_deg']) - RWY
            along = -w * math.cos(rel); cross = -w * math.sin(rel)
            dx += (along - mean_along * KT) * dt
            # Runway frame: +cross is right of the runway direction; y is measured
            # positive on the downwind (left) side here, so y = -cross.
            dy += -(cross - mean_cross * KT) * dt
        prev = r['sim_time']
        if stop(r):
            return dx, dy, r
    return None

data = defaultdict(list)
for pattern in sys.argv[1:]:
    for f in glob.glob(pattern):
        card = Path(f).parent
        cfg = json.loads((card / 'effective-config.json').read_text())['parameters']
        m = json.loads((card / 'assessment.json').read_text())['metrics']
        rows = [{k: float(v) for k, v in r.items()} for r in csv.DictReader(open(f))]
        cond = card.name.rsplit('-', 1)[0].replace('_early', '').replace('_late', '')
        out = {}
        for key, stop in [('turn_base', lambda r: int(r['phase_id']) >= 4),
                          ('y3000', lambda r: int(r['phase_id']) == 5 and r['runway_cross_ft'] <= 3000),
                          ('turn_final', lambda r: int(r['phase_id']) >= 6),
                          ('roundout', lambda r: r['roundout_sim_time'] >= 0)]:
            res = drift_until(rows, cfg, stop)
            if res:
                dx, dy, r = res
                out[key] = (dx - dy, dx, dy)
        data[cond].append((out, m['touchdown_ft'], m['roundout_entry_along_ft']))

for key in ['turn_base', 'y3000', 'turn_final', 'roundout']:
    L, TD, RO, DX, DY = [], [], [], [], []
    for cond, v in data.items():
        v = [x for x in v if key in x[0]]
        l = np.array([x[0][key][0] for x in v]); td = np.array([x[1] for x in v]); ro = np.array([x[2] for x in v])
        dx = np.array([x[0][key][1] for x in v]); dy = np.array([x[0][key][2] for x in v])
        L += list(l - l.mean()); TD += list(td - td.mean()); RO += list(ro - ro.mean()); DX += list(dx - dx.mean()); DY += list(dy - dy.mean())
    L, TD, RO, DX, DY = map(np.array, (L, TD, RO, DX, DY))
    X = np.c_[DX, DY]
    c2, *_ = np.linalg.lstsq(X, TD, rcond=None); r2 = 1 - (TD - X @ c2).var() / TD.var()
    print(f'{key:10s} n={len(L)} path-shortening sd {L.std():5.0f} ft | corr with touchdown {np.corrcoef(L, TD)[0,1]:5.2f} '
          f'(slope {np.polyfit(L, TD, 1)[0]:4.2f}) roundout {np.corrcoef(L, RO)[0,1]:5.2f} | along/cross separately R2 {r2:4.2f} coef {np.round(c2, 2)}')
