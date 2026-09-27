"""Energy lost per foot of air distance against sideslip on final (full flaps)."""
import csv, glob, gzip, sys
import numpy as np

G = 32.174
X = []
for pattern in sys.argv[1:]:
    for f in glob.glob(pattern):
        rows = list(csv.DictReader(gzip.open(f, 'rt') if f.endswith('.gz') else open(f)))
        seg = [r for r in rows if int(float(r['phase_id'])) == 7 and 90 < float(r['agl_ft']) < 450
               and float(r['flap_actual_ratio']) > 0.99 and r['contact_latched'] == '0']
        # 2 s windows: air-relative specific energy change per air distance.
        i = 0
        while i < len(seg):
            j = i
            t0 = float(seg[i]['sim_time'])
            while j < len(seg) and float(seg[j]['sim_time']) - t0 < 2.0:
                j += 1
            if j >= len(seg):
                break
            a, b = seg[i], seg[j]
            tas = lambda r: float(r['true_airspeed_mps']) / 0.3048
            gs = lambda r: float(r['groundspeed_kt']) * 1.6878
            # Ground-referenced energy is immune to gust TAS jumps; per ground foot.
            e = lambda r: float(r['agl_ft']) + gs(r) ** 2 / (2 * G)
            dist = float(b['runway_along_ft']) - float(a['runway_along_ft'])
            win = seg[i:j]
            beta = np.mean([abs(float(r['beta_deg'])) for r in win])
            ias = np.mean([float(r['ias_kias']) for r in win])
            head = np.mean([tas(r) - gs(r) for r in win])
            if dist > 50:
                X.append((beta, ias, head, (e(a) - e(b)) / dist))
            i = j
X = np.array(X)
print(len(X), 'windows')
for lo, hi in [(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 8)]:
    m = (X[:, 0] >= lo) & (X[:, 0] < hi)
    if m.sum():
        print(f'beta {lo}-{hi}: n {m.sum():4d}  energy loss per ground ft {X[m, 3].mean():.4f} +/- {X[m, 3].std() / np.sqrt(m.sum()):.4f}  ias {X[m, 1].mean():.1f}  head {X[m,2].mean():.1f}')
A = np.column_stack([np.ones(len(X)), X[:, 0] ** 2, X[:, 1] - 78, X[:, 2]])
coef, *_ = np.linalg.lstsq(A, X[:, 3], rcond=None)
print('fit: loss = %.4f + %.5f*beta^2 + %.5f*(ias-78) + %.5f*head_fps' % tuple(coef))
