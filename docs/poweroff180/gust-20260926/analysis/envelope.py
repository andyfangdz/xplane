"""Tabulate flights inside the 10 kt sustained / 20 kt peak envelope."""
import json, sys
from pathlib import Path
from statistics import mean, pstdev

ENVELOPE = {'head5_gust8', 'head10_gust10', 'cross_left10_gust8', 'cross_right10_gust8', 'tail5_gust5',
            'head7_gust6', 'qleft10_gust6', 'qright8_gust8', 'cross_right6_gust6', 'qtail6_gust4'}

def condition(name):
    base = name.rsplit('-', 1)[0]
    for suffix in ('_early', '_late'):
        base = base.replace(suffix, '')
    return base

def load(runs):
    out = []
    for run in runs:
        for card in sorted(Path(run, 'cards').glob('*')):
            a = card / 'assessment.json'
            if not a.exists() or condition(card.name) not in ENVELOPE:
                continue
            j = json.loads(a.read_text())
            out.append((Path(run).name, card.name, j))
    return out

if __name__ == '__main__':
    flights = load(sys.argv[1:])
    print(f"{'card':32s} pass td    kias  lim   sink  ro_along ro_agl ro_kias flare_s reasons")
    for run, card, j in flights:
        m = j['metrics']
        print(f"{card:32s} {int(j['passed'])}    {m.get('touchdown_ft',0):5.0f} {m.get('kias',0):5.1f} {m.get('maximum_touchdown_kias',0):4.1f} "
              f"{m.get('physical_sink_fpm',0):5.0f} {m.get('roundout_entry_along_ft') or 0:7.0f} {m.get('roundout_entry_agl_ft') or 0:5.1f} "
              f"{m.get('roundout_entry_kias') or 0:6.1f} {m.get('flare_duration_s') or 0:5.1f}  {','.join(j['reasons'])}")
    td = [j['metrics']['touchdown_ft'] for _, _, j in flights if 'touchdown_ft' in j['metrics']]
    print(f"{sum(j['passed'] for _,_,j in flights)}/{len(flights)} passed; touchdown mean {mean(td):.0f} sd {pstdev(td):.0f}")
    reasons = {}
    for _, _, j in flights:
        for r in j['reasons']:
            reasons[r] = reasons.get(r, 0) + 1
    print(reasons)
