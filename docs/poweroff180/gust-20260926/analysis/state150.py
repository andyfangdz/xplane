"""Which state at a height on final predicts touchdown (v10 flights)?"""
import csv, glob, gzip, json, math, sys
from pathlib import Path
import numpy as np

H = float(sys.argv[1])
rows = []
for pattern in sys.argv[2:]:
    for card in sorted(glob.glob(pattern)):
        card = Path(card)
        a = json.loads((card / 'assessment.json').read_text())['metrics']
        if 'touchdown_ft' not in a:
            continue
        tr = card / 'trace.csv.gz'
        f = gzip.open(tr, 'rt') if tr.exists() else open(card / 'trace.csv')
        r = next(r for r in csv.DictReader(f) if int(float(r['phase_id'])) == 7 and float(r['agl_ft']) <= H)
        p = json.loads((card / 'effective-config.json').read_text())['parameters']
        head = p['wind_speed_kt'] * math.cos(math.radians(p['wind_offset_deg']))
        ias = float(r['ias_kias']); gs = float(r['groundspeed_kt'])
        tas = float(r['true_airspeed_mps']) * 1.94384
        # Energy height above the 35 ft roundout at the landing target, feet.
        e = float(r['agl_ft']) + ((tas * 1.6878) ** 2 - ((70 + head * 0) * 1.6878) ** 2) / 64.35
        rows.append(dict(name=card.name, td=a['touchdown_ft'], x=float(r['runway_along_ft']), ias=ias, gs=gs, tas=tas,
                         vvi=float(r['vvi_fpm']), e=e, head=head, gust=p['wind_gust_kt']))
y = np.array([r['td'] for r in rows])
print(f'{len(rows)} flights at {H:.0f} ft; touchdown sd {y.std():.0f}')
def fit(keys):
    X = np.column_stack([np.ones(len(rows))] + [[r[k] for r in rows] for k in keys])
    coef, *_ = np.linalg.lstsq(X, y, rcond=None)
    res = y - X @ coef
    # leave-one-out
    loo = []
    for i in range(len(rows)):
        m = np.arange(len(rows)) != i
        c, *_ = np.linalg.lstsq(X[m], y[m], rcond=None)
        loo.append(y[i] - X[i] @ c)
    print(f'  {"+".join(keys):28s} resid sd {res.std():4.0f}  loo sd {np.std(loo):4.0f}  coef {np.round(coef[1:], 2)}')
for keys in [['x'], ['ias'], ['x', 'ias'], ['x', 'gs'], ['x', 'ias', 'head'], ['x', 'tas', 'vvi'], ['x', 'ias', 'vvi', 'head'], ['head'], ['x', 'head']]:
    fit(keys)
