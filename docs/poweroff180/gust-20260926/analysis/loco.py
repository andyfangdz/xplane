"""Leave-one-condition-out test of a physics-structured base-leg energy estimator.

Features at base (y = Y): height, and the time to fly the rest of base, the
final-turn arc and final to the roundout aim at mean-wind groundspeeds. Target:
touchdown - aim. A usable estimator has small held-out-condition bias.
"""
import csv, glob, json, math, sys
from collections import defaultdict
from pathlib import Path
import numpy as np

Y = float(sys.argv[1])

def features(r, cfg):
    g = cfg['wind_gust_kt']; off = math.radians(cfg['wind_offset_deg'])
    mean = cfg['wind_speed_kt'] + 0.35 * g
    head = mean * math.cos(off); cross = mean * math.sin(off); tail = max(0.0, -math.cos(off))
    travel = 680 - 11.7 * head - 0.28 * head * head - 0.9 * cross + 30 * g * tail
    aim = 1100 - 40 - travel
    approach = 75 + 0.7 * g
    gs_base = max(60.0, r['groundspeed_kt'] * 1.68781)
    gs_final = max(40.0, approach * 1.02 - head) * 1.68781
    radius = max(500.0, r['turn_lead_ft'])
    t_base = max(0.0, r['runway_cross_ft'] - radius) / gs_base
    t_turn = radius * math.pi / 2 / (0.5 * (gs_base + gs_final))
    t_final = max(0.0, aim - (r['runway_along_ft'] + radius)) / gs_final
    return [r['agl_ft'], t_base, t_turn, t_final], aim

data = []
for pattern in sys.argv[2:]:
    for f in glob.glob(pattern):
        card = Path(f).parent
        cfg = json.loads((card / 'effective-config.json').read_text())['parameters']
        m = json.loads((card / 'assessment.json').read_text())['metrics']
        rows = [{k: float(v) for k, v in r.items()} for r in csv.DictReader(open(f))]
        r = next((a for a in rows if int(a['phase_id']) == 5 and a['runway_cross_ft'] <= Y), None)
        if r is None:
            continue
        x, aim = features(r, cfg)
        cond = card.name.rsplit('-', 1)[0].replace('_early', '').replace('_late', '')
        data.append((cond, x, m['touchdown_ft'] - (1100 - 40)))
conds = sorted({c for c, _, _ in data})
X = np.array([[1.0] + x for _, x, _ in data]); y = np.array([t for _, _, t in data])
names = np.array([c for c, _, _ in data])
coef, *_ = np.linalg.lstsq(X, y, rcond=None)
res = y - X @ coef
print(f'y={Y:.0f} all-data fit: residual sd {res.std():.0f} ft (target sd {y.std():.0f}); coef {np.round(coef, 2)}')
print(f'  implied sink rates (ft/s): base {-coef[2]/coef[1]:.1f}, turn {-coef[3]/coef[1]:.1f}, final {-coef[4]/coef[1]:.1f}; touchdown ft per ft height {coef[1]:.1f}')
held = []
for c in conds:
    train = names != c
    k, *_ = np.linalg.lstsq(X[train], y[train], rcond=None)
    pred = X[~train] @ k
    bias = np.mean(y[~train] - pred); within = np.std((y[~train] - pred) - bias)
    held.append(bias)
    print(f'  held out {c:22s} bias {bias:+6.0f} ft  within-residual sd {within:4.0f}  (raw sd {np.std(y[~train]):4.0f})')
print(f'  held-out bias sd {np.std(held):.0f} ft, mean |bias| {np.mean(np.abs(held)):.0f} ft')
