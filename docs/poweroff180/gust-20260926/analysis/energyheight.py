"""Air-relative energy-height deviation as a base-leg energy estimator.

1. Fit the idle energy-loss rate dE/dt (E = h + TAS^2/2g) against IAS, flap
   ratio and bank on steady-wind flights, from the power cut to roundout.
2. For gust flights, deviation = E(t) - (E_cut + integral of fitted loss rate).
3. Report between-condition bias and within-condition correlation with the
   touchdown and roundout at base (y = 3000) and turn-final start.
"""
import csv, glob, json, math, sys
from collections import defaultdict
from pathlib import Path
import numpy as np
G = 32.174

def rows_of(f):
    return [{k: float(v) for k, v in r.items()} for r in csv.DictReader(open(f))]

def energy(r):
    tas = r['true_airspeed_mps'] / 0.3048
    return r['agl_ft'] + tas * tas / (2 * G)

def features(r):
    ias = r['ias_kias']; flap = r['flap_actual_ratio']; bank = math.radians(r['bank_deg'])
    load = 1 / max(0.5, math.cos(bank))
    d = ias - 80
    return [1.0, d, d * d, flap, flap * d, load * load - 1]

def segment(rows):
    return [r for r in rows if r['cut_sim_time'] >= 0 and r['roundout_sim_time'] < 0 and int(r['phase_id']) < 8]

steady = sys.argv[1].split(';'); gusty = sys.argv[2].split(';')
X, y = [], []
for pattern in steady:
    for f in glob.glob(pattern):
        seg = segment(rows_of(f))
        for a, b in zip(seg[::20], seg[20::20]):
            dt = b['sim_time'] - a['sim_time']
            if dt > 0:
                X.append(features(a)); y.append((energy(b) - energy(a)) / dt)
X = np.array(X); y = np.array(y)
coef, *_ = np.linalg.lstsq(X, y, rcond=None)
res = y - X @ coef
print(f'polar fit on steady flights: n={len(y)} residual sd {res.std():.2f} ft/s (dE/dt sd {y.std():.2f}); coef {np.round(coef, 4)}')

data = defaultdict(list)
for pattern in gusty:
    for f in glob.glob(pattern):
        card = Path(f).parent
        m = json.loads((card / 'assessment.json').read_text())['metrics']
        cond = card.name.rsplit('-', 1)[0].replace('_early', '').replace('_late', '')
        seg = segment(rows_of(f))
        e0 = energy(seg[0]); pred = e0; out = {}
        for a, b in zip(seg, seg[1:]):
            pred += float(np.dot(features(a), coef)) * (b['sim_time'] - a['sim_time'])
            dev = energy(b) - pred
            if 'y3000' not in out and int(b['phase_id']) == 5 and b['runway_cross_ft'] <= 3000:
                out['y3000'] = (dev, b['runway_along_ft'])
            if 'turn_final' not in out and int(b['phase_id']) >= 6:
                out['turn_final'] = (dev, b['runway_along_ft'])
        data[cond].append((out, m['touchdown_ft'], m['roundout_entry_along_ft']))

for key in ['y3000', 'turn_final']:
    print(f'--- {key}')
    means = []
    D, X_, TD, RO = [], [], [], []
    for cond, v in sorted(data.items()):
        v = [x for x in v if key in x[0]]
        d = np.array([x[0][key][0] for x in v]); xx = np.array([x[0][key][1] for x in v])
        td = np.array([x[1] for x in v]); ro = np.array([x[2] for x in v])
        means.append(d.mean())
        print(f'  {cond:22s} energy deviation mean {d.mean():+6.1f} ft sd {d.std():4.1f}  touchdown mean {td.mean():6.0f}')
        D += list(d - d.mean()); X_ += list(xx - xx.mean()); TD += list(td - td.mean()); RO += list(ro - ro.mean())
    D, X_, TD, RO = map(np.array, (D, X_, TD, RO))
    A = np.c_[D, X_]
    c2, *_ = np.linalg.lstsq(A, TD, rcond=None); r2 = 1 - (TD - A @ c2).var() / TD.var()
    print(f'  between-condition sd of mean deviation {np.std(means):.1f} ft; within: corr(dev, touchdown) {np.corrcoef(D, TD)[0,1]:.2f} '
          f'slope {np.polyfit(D, TD, 1)[0]:.1f} ft/ft; corr(dev, roundout) {np.corrcoef(D, RO)[0,1]:.2f}; with position R2 {r2:.2f} coef {np.round(c2, 2)}')
