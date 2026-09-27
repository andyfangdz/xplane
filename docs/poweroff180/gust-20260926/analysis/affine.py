"""Fit lead = L0 + tau0 * achieved_rate per height band (response 0.15-0.35 s later)."""
import csv, glob, sys
import numpy as np
bands = [(0, 5), (5, 10), (10, 15), (15, 20), (20, 27), (27, 36)]
data = {b: [] for b in bands}
for pattern in sys.argv[1:]:
    for f in glob.glob(pattern):
        rows = [{k: float(v) for k, v in r.items()} for r in csv.DictReader(open(f))]
        fl = [r for r in rows if r['roundout_sim_time'] >= 0 and r['contact_latched'] == 0 and r['cut_sim_time'] >= 0]
        t = np.array([r['sim_time'] for r in fl]); p = np.array([r['pitch_deg'] for r in fl])
        c = np.array([r['pitch_command'] for r in fl]); h = np.array([r['agl_ft'] for r in fl])
        for i in range(0, len(fl), 3):
            j = np.searchsorted(t, t[i] + 0.15); k = np.searchsorted(t, t[i] + 0.35)
            if k >= len(fl): break
            rate = (p[k] - p[j]) / (t[k] - t[j])
            for b in bands:
                if b[0] <= h[i] < b[1]:
                    data[b].append((rate, c[i] - p[i]))
for b in bands:
    d = np.array(data[b])
    if len(d) > 10:
        A = np.c_[np.ones(len(d)), d[:, 0]]
        (l0, tau), res, *_ = np.linalg.lstsq(A, d[:, 1], rcond=None)
        pred = A @ [l0, tau]; r2 = 1 - ((d[:, 1] - pred) ** 2).sum() / ((d[:, 1] - d[:, 1].mean()) ** 2).sum()
        print(f'{b[0]:2d}-{b[1]:2d} ft n={len(d):5d} L0 {l0:5.2f} deg  tau0 {tau:5.2f} s  R2 {r2:4.2f}  rate range {d[:,0].min():5.1f}..{d[:,0].max():4.1f}')
