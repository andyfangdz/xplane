"""Effective inner-loop lead time (command lead / achieved pitch rate) by height."""
import csv, glob, sys
import numpy as np
bins = [0, 5, 10, 15, 20, 25, 30, 36]
per_bin = {i: [] for i in range(len(bins) - 1)}
per_flight = []
for path in sys.argv[1:]:
    for f in glob.glob(path):
        rows = [{k: float(v) for k, v in r.items()} for r in csv.DictReader(open(f))]
        fl = [r for r in rows if r['roundout_sim_time'] >= 0 and r['contact_latched'] == 0 and r['cut_sim_time'] >= 0]
        t = np.array([r['sim_time'] for r in fl]); p = np.array([r['pitch_deg'] for r in fl])
        c = np.array([r['pitch_command'] for r in fl]); h = np.array([r['agl_ft'] for r in fl])
        e = np.array([r['elevator_input'] for r in fl])
        flight = []
        for i in range(len(fl)):
            # rate over the next 0.15 s (the response to the current lead)
            j = np.searchsorted(t, t[i] + 0.15)
            k = np.searchsorted(t, t[i] + 0.35)
            if k >= len(fl): break
            rate = (p[k] - p[j]) / (t[k] - t[j])
            lead = c[i] - p[i]
            if rate > 0.8 and lead > 0.2:
                tau = lead / rate
                for b in range(len(bins) - 1):
                    if bins[b] <= h[i] < bins[b + 1]:
                        per_bin[b].append(tau)
                flight.append((h[i], tau, e[i]))
        per_flight.append((f.split('\\')[-2], flight))
for b in range(len(bins) - 1):
    d = np.array(per_bin[b])
    if len(d):
        print(f'{bins[b]:3d}-{bins[b+1]:3d} ft n={len(d):5d} tau median {np.median(d):5.2f} p25 {np.percentile(d,25):5.2f} p75 {np.percentile(d,75):5.2f}')
for name, fl in per_flight:
    if fl:
        a = np.array(fl)
        hi = a[a[:, 0] > 20][:, 1]; lo = a[a[:, 0] < 10][:, 1]
        print(f'{name:24s} tau>20ft {np.median(hi) if len(hi) else float("nan"):5.2f}  tau<10ft {np.median(lo) if len(lo) else float("nan"):5.2f}')
