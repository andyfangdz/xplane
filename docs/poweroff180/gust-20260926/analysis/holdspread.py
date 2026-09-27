"""Per-flight holding lead near the runway with the response time fixed at 0.4 s."""
import csv, glob, sys
import numpy as np
out = []
for pattern in sys.argv[1:]:
    for f in glob.glob(pattern):
        rows = [{k: float(v) for k, v in r.items()} for r in csv.DictReader(open(f))]
        fl = [r for r in rows if r['roundout_sim_time'] >= 0 and r['contact_latched'] == 0 and r['cut_sim_time'] >= 0]
        t = np.array([r['sim_time'] for r in fl]); p = np.array([r['pitch_deg'] for r in fl])
        c = np.array([r['pitch_command'] for r in fl]); h = np.array([r['agl_ft'] for r in fl])
        vals = []
        for i in range(0, len(fl), 3):
            j = np.searchsorted(t, t[i] + 0.15); k = np.searchsorted(t, t[i] + 0.35)
            if k >= len(fl): break
            if h[i] < 12:
                rate = (p[k] - p[j]) / (t[k] - t[j])
                model_hold = 1.8 * (33 - h[i]) / 33
                vals.append((c[i] - p[i] - 0.4 * rate) / model_hold)
        if vals:
            out.append((f.split('\\')[-4][4:28] + '/' + f.split('\\')[-2], np.median(vals)))
for name, v in sorted(out, key=lambda x: x[1]):
    print(f'{name:60s} holding/model {v:4.2f}')
a = np.array([v for _, v in out]); print(f'n={len(a)} median {np.median(a):.2f} min {a.min():.2f} max {a.max():.2f} p10 {np.percentile(a,10):.2f} p90 {np.percentile(a,90):.2f}')
