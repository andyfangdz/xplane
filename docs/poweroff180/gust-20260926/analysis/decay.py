"""Gust decay through the flare: headwind excess over steady at 50 ft vs touchdown."""
import csv, glob, gzip, json, math, sys
from pathlib import Path

for card in sorted(glob.glob(sys.argv[1] + r'\cards\*')):
    card = Path(card)
    p = json.loads((card / 'effective-config.json').read_text())['parameters']
    m = json.loads((card / 'assessment.json').read_text())
    tr = card / 'trace.csv.gz'
    rows = list(csv.DictReader(gzip.open(tr, 'rt') if tr.exists() else open(card / 'trace.csv')))
    contact = next(i for i, r in enumerate(rows) if r['contact_latched'] == '1')
    heading = None
    def head(r):
        # Runway heading from the track on final, wind-from relative to it.
        return float(r['wind_speed_mps']) * 1.9438 * math.cos(math.radians(float(r['wind_direction_true_deg']) - hdg))
    final = [r for r in rows[:contact] if int(float(r['phase_id'])) == 7]
    hdg = sum(float(r['ground_track_true_deg']) for r in final[-50:]) / 50
    at = lambda h: next(r for r in final if float(r['agl_ft']) <= h)
    steady_head = p['wind_speed_kt'] * math.cos(math.radians(p['wind_offset_deg']))
    r100, r50, r35, rt = at(100), at(50), at(35), rows[contact]
    ro = float(r35['sim_time'])
    print(f"{card.name:30s} {'P' if m['passed'] else '-'} td {m['metrics']['touchdown_ft']:5.0f} "
          f"head(100,50,35,td) {head(r100):5.1f} {head(r50):5.1f} {head(r35):5.1f} {head(rt):5.1f} steady {steady_head:5.1f} | "
          f"ias35 {float(r35['ias_kias']):5.1f} iasTD {float(rt['ias_kias']):5.1f} vvi35 {float(r35['vvi_fpm']):5.0f} flare {float(rt['sim_time'])-ro:4.1f} {','.join(m['reasons'])}")
