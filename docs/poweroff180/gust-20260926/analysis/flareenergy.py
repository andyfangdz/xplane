"""Does flare distance follow energy? Fit float distance from roundout state.

E = h_ro + (V_ro^2 - V_td^2)/2g, speeds as true airspeed (air-relative, so a
decaying gust removes energy), distance along the runway.
"""
import csv, glob, gzip, json, math, sys
from pathlib import Path
import numpy as np

KT = 1.68781; G = 32.174
data = []
for pattern in sys.argv[2:]:
    for card in sorted(glob.glob(pattern)):
        card = Path(card)
        tr = card / 'trace.csv.gz'
        if not tr.exists() and not (card / 'trace.csv').exists():
            continue
        rows = list(csv.DictReader(gzip.open(tr, 'rt') if tr.exists() else open(card / 'trace.csv')))
        try:
            contact = next(i for i, r in enumerate(rows) if r['contact_latched'] == '1')
            ro = next(i for i, r in enumerate(rows[:contact]) if float(r['roundout_sim_time']) >= 0)
        except StopIteration:
            continue
        p = json.loads((card / 'effective-config.json').read_text())['parameters']
        a, b = rows[ro], rows[contact - 1]
        tas = lambda r: float(r['true_airspeed_mps']) / 0.3048
        gs = lambda r: float(r['groundspeed_kt']) * KT
        spd = {'tas': tas, 'gs': gs, 'ias': lambda r: float(r['ias_kias']) * KT}[sys.argv[1]]
        e = float(a['agl_ft']) + (spd(a) ** 2 - spd(b) ** 2) / (2 * G)
        dist = float(b['runway_along_ft']) - float(a['runway_along_ft'])
        # Mean air/ground speed ratio over the flare converts air distance.
        seg = rows[ro:contact]
        ratio = np.mean([tas(r) / max(gs(r), 50) for r in seg])
        data.append(dict(name=card.parent.parent.name[-22:] + '/' + card.name, e=e, dist=dist, ratio=ratio,
                         gust=p.get('wind_gust_kt', 0), vro=tas(a) / KT, vtd=tas(b) / KT,
                         ias_ro=float(a['ias_kias']), ias_td=float(b['ias_kias'])))
d = np.array([[r['e'], r['dist'], r['ratio']] for r in data])
air = d[:, 1] * d[:, 2]
k = np.sum(d[:, 0] * air) / np.sum(air * air)
pred = d[:, 0] / k / d[:, 2]
res = d[:, 1] - pred
print(f'{len(data)} flares; energy per air foot {k:.4f}; distance residual sd {res.std():.0f} ft (distance sd {d[:,1].std():.0f})')
# By gust vs steady
g = np.array([r['gust'] > 0 for r in data])
for name, m in [('gust', g), ('steady', ~g)]:
    if m.sum():
        kk = np.sum(d[m, 0] * air[m]) / np.sum(air[m] ** 2)
        print(f'  {name}: n {m.sum()} k {kk:.4f} residual sd {(d[m,1] - d[m,0]/kk/d[m,2]).std():.0f}')

