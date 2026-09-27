"""Fit base-leg touchdown predictors on 12 gust conditions; leave one condition out.

A predictor f(state on base, wind) estimates touchdown - aim. A controller that
steers f to zero removes the predictable part; its held-out bias would be
injected. Score: held-out RMS error vs the do-nothing RMS (touchdown - aim).
"""
import csv, glob, json, math, sys
from collections import defaultdict
from pathlib import Path
import numpy as np
KT = 1.68781

def rows_of(f):
    return [{k: float(v) for k, v in r.items()} for r in csv.DictReader(open(f))]

def wind(cfg):
    g = cfg['wind_gust_kt']; off = math.radians(cfg['wind_offset_deg'])
    mean = cfg['wind_speed_kt'] + 0.35 * g
    return mean * math.cos(off), mean * math.sin(off), g, max(0.0, -math.cos(off))

def base_state(r, cfg):
    head, cross, g, tail = wind(cfg)
    travel = 680 - 11.7 * head - 0.28 * head * head - 0.9 * cross + 30 * g * tail
    aim = 1100 - 40 - travel
    approach = 75 + 0.7 * g
    gs_base = max(60.0, r['groundspeed_kt'] * KT); gs_final = max(40.0, approach * 1.02 - head) * KT
    radius = max(500.0, r['turn_lead_ft'])
    t_base = max(0.0, r['runway_cross_ft'] - radius) / gs_base
    t_turn = radius * math.pi / 2 / (0.5 * (gs_base + gs_final))
    t_final = max(0.0, aim - (r['runway_along_ft'] + radius)) / gs_final
    return dict(h=r['agl_ft'], t_base=t_base, t_turn=t_turn, t_final=t_final, head=head, cross=abs(cross), gust=g, tail=tail,
                ias=r['ias_kias'])

Y = float(sys.argv[1])
data = []
for p in sys.argv[2:]:
    for f in glob.glob(p):
        card = Path(f).parent
        if not (card / 'assessment.json').exists():
            continue
        cfg = json.loads((card / 'effective-config.json').read_text())['parameters']
        m = json.loads((card / 'assessment.json').read_text())['metrics']
        if 'touchdown_ft' not in m:
            continue
        rows = rows_of(f)
        r = next((a for a in rows if int(a['phase_id']) == 5 and a['runway_cross_ft'] <= Y), None)
        if r is None:
            continue
        cond = card.name.rsplit('-', 1)[0].replace('_early', '').replace('_late', '')
        data.append((cond, base_state(r, cfg), m['touchdown_ft'] - 1060))
conds = sorted({c for c, _, _ in data})
names = np.array([c for c, _, _ in data]); y = np.array([t for _, _, t in data])
print(f'y={Y:.0f}: {len(data)} flights, {len(conds)} conditions; do-nothing RMS {np.sqrt(np.mean(y**2)):.0f} ft (sd {y.std():.0f})')
sets = {
    'intercept only': [],
    'head only': ['head'],
    'head + tail': ['head', 'tail'],
    'energy only': ['h', 't_base', 't_turn', 't_final'],
    'energy + wind': ['h', 't_base', 't_turn', 't_final', 'head', 'cross', 'gust', 'tail'],
    'wind only (no base state)': ['head', 'cross', 'gust', 'tail'],
}
for name, keys in sets.items():
    X = np.array([[1.0] + [s[k] for k in keys] for _, s, _ in data])
    held = np.zeros_like(y)
    for c in conds:
        tr = names != c
        coef, *_ = np.linalg.lstsq(X[tr], y[tr], rcond=None)
        held[~tr] = X[~tr] @ coef
    # A controller steering the held-out prediction to zero leaves y - pred.
    after = y - held
    bias = [np.mean(after[names == c]) for c in conds]
    print(f'  {name:28s} held-out RMS after correction {np.sqrt(np.mean(after**2)):4.0f} ft; held-out condition-bias sd {np.std(bias):4.0f}; '
          f'within-condition sd {np.std(np.concatenate([after[names == c] - np.mean(after[names == c]) for c in conds])):4.0f}')

