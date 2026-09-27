"""Energy shift at roundout (ft) that keeps each flight landable in the box.

A perfect flare lands between 1020 and 1180 ft at an IAS in [63.5+m, ceiling-m].
Feasible shift interval: [lo, hi] in feet of energy height.
"""
import csv, glob, gzip, json, math, sys
from pathlib import Path
from collections import defaultdict

KT = 1.68781; G = 32.174; K = 0.0993
M = float(sys.argv[1])  # speed margin inside the window, kt
out = []
for pattern in sys.argv[2:]:
    for card in sorted(glob.glob(pattern)):
        card = Path(card)
        tr = card / 'trace.csv.gz'
        rows = list(csv.DictReader(gzip.open(tr, 'rt') if tr.exists() else open(card / 'trace.csv')))
        contact = next(i for i, r in enumerate(rows) if r['contact_latched'] == '1')
        ro = next(i for i, r in enumerate(rows[:contact]) if float(r['roundout_sim_time']) >= 0)
        m = json.loads((card / 'assessment.json').read_text())
        a, b = rows[ro], rows[contact - 1]
        tas = lambda r: float(r['true_airspeed_mps']) / 0.3048
        gs = lambda r: float(r['groundspeed_kt']) * KT
        seg = rows[ro:contact]
        ratio = sum(tas(r) / max(gs(r), 50) for r in seg) / len(seg)
        ipg = float(b['ias_kias']) / (gs(b) / KT)
        h, x, g0 = float(a['agl_ft']), float(a['runway_along_ft']), gs(a)
        ceil = m['metrics']['maximum_touchdown_kias']
        def energy(v_ias):  # ground energy at touchdown for an IAS
            return (v_ias / ipg * KT) ** 2 / (2 * G)
        e0 = h + g0 ** 2 / (2 * G)
        # Landing at X with IAS V requires e0 + d - K*ratio*(X - x) = energy(V).
        hi = energy(ceil - M) + K * ratio * (1180 - x) - e0   # most energy removable... (max shift allowed)
        lo = energy(63.5 + M) + K * ratio * (1020 - x) - e0  # least
        out.append((card.parent.parent.name, card.name, lo, hi, m['passed']))
cond = lambda n: n.rsplit('-', 1)[0]
print(f'{"card":30s}   lo     hi  (feasible energy shift at roundout, ft)')
for run, name, lo, hi, p in sorted(out, key=lambda o: o[1]):
    print(f'{name:30s} {lo:6.1f} {hi:6.1f} {"" if lo <= 0 <= hi else "<- needs shift"}  {run[-20:]}')
L = max(o[2] for o in out); H = min(o[3] for o in out)
print(f'common shift for all: [{L:.1f}, {H:.1f}] {"FEASIBLE" if L <= H else "none"}')
for grp, test in [('tail (offset>=135)', lambda n: 'tail' in n), ('others', lambda n: 'tail' not in n)]:
    sel = [o for o in out if test(o[1])]
    print(f'  {grp}: [{max(o[2] for o in sel):.1f}, {min(o[3] for o in sel):.1f}]')
