"""Touchdown speed that lands each flight at the aim, from its roundout state.

Flare distance = (h + (GS_ro^2 - GS_td^2)/2g) / (K * TAS/GS), K = 0.0993
(fitted on 252 flares, residual 27 ft). GS_td = IAS_td * ratio - head_td.
"""
import csv, glob, gzip, json, math, sys
from pathlib import Path

KT = 1.68781; G = 32.174; K = 0.0993
AIM = float(sys.argv[1])
ok = 0; total = 0
for pattern in sys.argv[2:]:
    for card in sorted(glob.glob(pattern)):
        card = Path(card)
        tr = card / 'trace.csv.gz'
        rows = list(csv.DictReader(gzip.open(tr, 'rt') if tr.exists() else open(card / 'trace.csv')))
        contact = next(i for i, r in enumerate(rows) if r['contact_latched'] == '1')
        ro = next(i for i, r in enumerate(rows[:contact]) if float(r['roundout_sim_time']) >= 0)
        p = json.loads((card / 'effective-config.json').read_text())['parameters']
        m = json.loads((card / 'assessment.json').read_text())
        a, b = rows[ro], rows[contact - 1]
        tas = lambda r: float(r['true_airspeed_mps']) / 0.3048
        gs = lambda r: float(r['groundspeed_kt']) * KT
        seg = rows[ro:contact]
        ratio = sum(tas(r) / max(gs(r), 50) for r in seg) / len(seg)
        # Groundspeed at touchdown that lands on the aim.
        need = lambda aim: math.sqrt(max(gs(a) ** 2 + 2 * G * (float(a['agl_ft']) - K * ratio * (aim - float(a['runway_along_ft']))), 0)) / KT
        gs_td = need(AIM)
        # Airspeed = groundspeed + actual surface headwind (after decay); IAS/TAS at contact.
        ias_per_gs = float(b['ias_kias']) / (gs(b) / KT)
        needed = gs_td * ias_per_gs
        ceiling = m['metrics']['maximum_touchdown_kias']
        near, far = need(1020) * ias_per_gs, need(1180) * ias_per_gs
        inside = far <= ceiling - 0.3 and near >= 63.5
        needed = f'{far:5.1f}-{near:5.1f}'
        ok += inside; total += 1
        print(f"{card.parent.parent.name[-18:]:18s} {card.name:30s} {'P' if m['passed'] else '-'} td {m['metrics']['touchdown_ft']:5.0f} "
              f"ro x {float(a['runway_along_ft']):5.0f} ias {float(a['ias_kias']):5.1f} | actual td ias {float(b['ias_kias']):5.1f}  needed {needed}  window 63.5-{ceiling - 0.3:4.1f} {'ok' if inside else 'OUT'}")
print(f'{ok}/{total} flights could land at {AIM:.0f} ft within the speed window')

