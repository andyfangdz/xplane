"""Fit a longitudinal point-mass model of the SR20 (full flaps, idle) on final.

Lift coefficient proxy: n / TAS^2 against angle of attack and ground effect.
Drag proxy: along-path deceleration against CL^2 and TAS^2.
"""
import csv, glob, gzip, math, sys
import numpy as np

G = 9.80665
rows = []
for path in sys.argv[1:]:
    for f in glob.glob(path):
        data = list(csv.DictReader((gzip.open(f, 'rt') if f.endswith('.gz') else open(f))))
        contact = next((i for i, r in enumerate(data) if r['contact_latched'] == '1'), None)
        if contact is None:
            continue
        seg = [r for r in data[:contact] if int(float(r['phase_id'])) == 7 and float(r['agl_ft']) < 250
               and float(r['flap_actual_ratio']) > 0.99 and abs(float(r['beta_deg'])) < 2.5]
        # Resample to 0.1 s and difference.
        last = None
        for r in seg:
            t = float(r['sim_time'])
            if last is not None and t - float(last['sim_time']) < 0.1:
                continue
            if last is not None:
                dt = t - float(last['sim_time'])
                v0, v1 = float(last['true_airspeed_mps']), float(r['true_airspeed_mps'])
                rows.append(dict(
                    h=float(r['agl_ft']), v=v1, vy=float(r['vertical_speed_mps']), n=float(r['normal_g']),
                    pitch=float(r['pitch_deg']), dv=(v1 - v0) / dt, dvy=(float(r['vertical_speed_mps']) - float(last['vertical_speed_mps'])) / dt,
                    bank=float(r['bank_deg'])))
            last = r
print(len(rows), 'samples')
h = np.array([r['h'] for r in rows]); v = np.array([r['v'] for r in rows]); vy = np.array([r['vy'] for r in rows])
n = np.array([r['n'] for r in rows]); pitch = np.array([r['pitch'] for r in rows]); dv = np.array([r['dv'] for r in rows])
gamma = np.degrees(np.arcsin(np.clip(vy / v, -1, 1)))
alpha = pitch - gamma
V0 = 38.0  # ~74 kt reference
cl = n / (v / V0) ** 2
for span in [8, 12, 18, 25]:
    ge = np.exp(-h / span)
    X = np.column_stack([np.ones_like(h), alpha, ge, alpha * ge])
    coef, res, *_ = np.linalg.lstsq(X, cl, rcond=None)
    pred = X @ coef
    print(f'CL span {span}: coef {np.round(coef, 4)} R2 {1 - np.var(cl - pred) / np.var(cl):.3f} rms {np.std(cl - pred):.4f}')
# Drag: along-path deceleration plus gravity component gives D/W.
dw = -(dv + G * np.sin(np.radians(gamma))) / G
for span in [8, 12, 18]:
    ge = np.exp(-h / span)
    X = np.column_stack([(v / V0) ** 2, cl ** 2 * (v / V0) ** 2, cl ** 2 * (v / V0) ** 2 * ge])
    coef, *_ = np.linalg.lstsq(X, dw, rcond=None)
    pred = X @ coef
    print(f'CD span {span}: coef {np.round(coef, 4)} R2 {1 - np.var(dw - pred) / np.var(dw):.3f} rms {np.std(dw - pred):.4f}')
print('alpha range', np.percentile(alpha, [1, 50, 99]).round(2), 'cl range', np.percentile(cl, [1, 50, 99]).round(3))

