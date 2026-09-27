"""Score base-leg energy estimators against the eventual roundout position.

Each estimator returns a height surplus (ft) from quantities guidance has on
base. Good estimators predict (roundout - aim) with small residuals and small
bias between wind conditions.
"""
import csv, glob, json, math, sys
from collections import defaultdict
from pathlib import Path
import numpy as np

def load(patterns):
    out = []
    for pattern in patterns:
        for f in glob.glob(pattern):
            card = Path(f).parent
            cfg = json.loads((card / 'effective-config.json').read_text())['parameters']
            rows = [{k: float(v) for k, v in r.items()} for r in csv.DictReader(open(f))]
            cond = card.name.rsplit('-', 1)[0].replace('_early', '').replace('_late', '')
            out.append((cond, cfg, rows))
    return out

def context(cfg):
    g = cfg['wind_gust_kt']; off = math.radians(cfg['wind_offset_deg'])
    mean = cfg['wind_speed_kt'] + 0.35 * g
    head = mean * math.cos(off); cross = mean * math.sin(off)
    tail = max(0.0, -math.cos(off))
    travel = 680 - 11.7 * head - 0.28 * head * head - 0.9 * cross + 30 * g * tail
    aim = 1100 - 40 - travel
    approach = 75 + 0.7 * g
    return head, cross, aim, approach

def vy_filtered(rows, i, span=2.0):
    t0 = rows[i]['sim_time']
    vals = [r['vertical_speed_mps'] / 0.3048 for r in rows[:i + 1] if t0 - r['sim_time'] <= span]
    return float(np.mean(vals))

def estimators(r, rows, i, cfg):
    head, cross, aim, approach = context(cfg)
    gs = max(60.0, r['groundspeed_kt'] * 1.68781)
    final_gs = max(40.0, approach * 1.02 - head) * 1.68781
    radius = max(500.0, r['turn_lead_ft'])
    base_left = max(0.0, r['runway_cross_ft'] - radius)
    arc = radius * math.pi / 2
    final_left = max(0.0, aim - (r['runway_along_ft'] + radius))
    t = base_left / gs + arc / (0.5 * (gs + final_gs)) + final_left / final_gs
    sink_meas = max(5.0, -vy_filtered(rows, i))  # ft/s, measured over 2 s
    est = {}
    est['const900'] = r['agl_ft'] - 35 - 15.0 * t
    est['measured_sink'] = r['agl_ft'] - 35 - sink_meas * t
    # Path-distance form: height per foot of ground path on base and final
    grad_base = sink_meas / gs
    grad_final = sink_meas / final_gs
    est['path_gradients'] = r['agl_ft'] - 35 - grad_base * (base_left + arc / 2) - grad_final * (arc / 2 + final_left)
    return est, aim

def main():
    data = load(sys.argv[1:])
    for y in (4000, 3500, 3000, 2500, 2000):
        rows_out = defaultdict(list)
        for cond, cfg, rows in data:
            idx = next((i for i, a in enumerate(rows) if int(a['phase_id']) == 5 and a['runway_cross_ft'] <= y), None)
            ro = next((a for a in rows if a['roundout_sim_time'] >= 0), None)
            if idx is None or ro is None:
                continue
            est, aim = estimators(rows[idx], rows, idx, cfg)
            for name, e in est.items():
                rows_out[name].append((cond, e, ro['runway_along_ft'] - aim))
        print(f'--- y={y}')
        for name, v in rows_out.items():
            e = np.array([x[1] for x in v]); o = np.array([x[2] for x in v])
            slope, icpt = np.polyfit(e, o, 1)
            res = o - (slope * e + icpt)
            conds = defaultdict(list)
            for (c, ee, oo), rr in zip(v, res):
                conds[c].append(rr)
            bias = np.array([np.mean(r) for r in conds.values()])
            within = np.concatenate([np.array(r) - np.mean(r) for r in conds.values()])
            print(f'{name:16s} n={len(e)} r={np.corrcoef(e, o)[0,1]:5.2f} slope {slope:5.1f} ft/ft  residual sd {res.std():4.0f} ft '
                  f'(between-condition bias sd {bias.std():4.0f}, within sd {within.std():4.0f})  estimator sd {e.std():4.1f} ft')

main()
