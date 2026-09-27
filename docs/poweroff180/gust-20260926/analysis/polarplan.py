"""Polar-planned energy surplus on base, calibrated only on steady-wind flights.

surplus = h + (TAS^2 - V_approach^2)/2g - (35 + sum(segment energy loss)),
segment losses from the steady-wind polar at planned speeds, flaps and bank,
times from the remaining base, final-turn arc and final at mean-wind speeds.
"""
import csv, glob, json, math, sys
from collections import defaultdict
from pathlib import Path
import numpy as np
G = 32.174; KT = 1.68781

def rows_of(f):
    return [{k: float(v) for k, v in r.items()} for r in csv.DictReader(open(f))]
def energy(r):
    tas = r['true_airspeed_mps'] / 0.3048
    return r['agl_ft'] + tas * tas / (2 * G)
def feats(ias, flap, bank_deg):
    d = ias - 80; load = 1 / max(0.5, math.cos(math.radians(bank_deg)))
    return np.array([1.0, d, d * d, flap, flap * d, load * load - 1])
def segment(rows):
    return [r for r in rows if r['cut_sim_time'] >= 0 and r['roundout_sim_time'] < 0 and int(r['phase_id']) < 8]

steady, gusty = sys.argv[1].split(';'), sys.argv[2].split(';')
X, y = [], []
for p in steady:
    for f in glob.glob(p):
        seg = segment(rows_of(f))
        for a, b in zip(seg[::20], seg[20::20]):
            dt = b['sim_time'] - a['sim_time']
            if dt > 0:
                X.append(feats(a['ias_kias'], a['flap_actual_ratio'], a['bank_deg'])); y.append((energy(b) - energy(a)) / dt)
coef, *_ = np.linalg.lstsq(np.array(X), np.array(y), rcond=None)
loss = lambda ias, flap, bank: -float(feats(ias, flap, bank) @ coef)  # ft/s of energy height

def surplus(r, cfg):
    g = cfg['wind_gust_kt']; off = math.radians(cfg['wind_offset_deg'])
    mean = cfg['wind_speed_kt'] + 0.35 * g
    head = mean * math.cos(off); cross = mean * math.sin(off); tail = max(0.0, -math.cos(off))
    travel = 680 - 11.7 * head - 0.28 * head * head - 0.9 * cross + 30 * g * tail
    aim = 1100 - 40 - travel
    approach = 75 + 0.7 * g
    gs_base = max(60.0, r['groundspeed_kt'] * KT); gs_final = max(40.0, approach * 1.02 - head) * KT
    radius = max(500.0, r['turn_lead_ft'])
    t_base = max(0.0, r['runway_cross_ft'] - radius) / gs_base
    t_turn = radius * math.pi / 2 / (0.5 * (gs_base + gs_final))
    t_final = max(0.0, aim - (r['runway_along_ft'] + radius)) / gs_final
    need = (loss(r['ias_kias'], 1.0, 0.0) * t_base + loss(0.5 * (r['ias_kias'] + approach), 1.0, 20.0) * t_turn
            + loss(approach, 1.0, 0.0) * t_final)
    v_app = approach * 1.02 * KT
    return energy(r) - v_app * v_app / (2 * G) - 35 - need

data = defaultdict(list)
for p in gusty:
    for f in glob.glob(p):
        card = Path(f).parent
        cfg = json.loads((card / 'effective-config.json').read_text())['parameters']
        m = json.loads((card / 'assessment.json').read_text())['metrics']
        rows = rows_of(f)
        cond = card.name.rsplit('-', 1)[0].replace('_early', '').replace('_late', '')
        out = {}
        for y_ in (4000, 3000, 2000):
            r = next((a for a in rows if int(a['phase_id']) == 5 and a['runway_cross_ft'] <= y_), None)
            if r:
                out[y_] = surplus(r, cfg)
        data[cond].append((out, m['touchdown_ft'] - 1060))
for y_ in (4000, 3000, 2000):
    means, tdm, D, T = [], [], [], []
    for cond, v in data.items():
        v = [x for x in v if y_ in x[0]]
        s = np.array([x[0][y_] for x in v]); t = np.array([x[1] for x in v])
        means.append(s.mean()); tdm.append(t.mean())
        D += list(s - s.mean()); T += list(t - t.mean())
    D, T = np.array(D), np.array(T)
    print(f'y={y_}: condition mean surplus {np.round(means, 0)} (sd {np.std(means):.0f} ft); corr(condition surplus, condition touchdown error) '
          f'{np.corrcoef(means, tdm)[0,1]:.2f}; within corr {np.corrcoef(D, T)[0,1]:.2f} slope {np.polyfit(D, T, 1)[0]:.1f} ft/ft, surplus within-sd {D.std():.1f}')
