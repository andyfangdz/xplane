"""Read-only frame pacing analysis, with local-baseline outlier classification."""
import csv
import json
import statistics
import sys
from pathlib import Path

BASE = Path('D:/X-Plane 12/Output/performance-tests')
sys.path.insert(0, str(Path(__file__).resolve().parent.parent / 'stutters-and-cache-20260913'))
from analyze import run

def analyze(directory):
    report = run(directory)
    session = json.loads((directory/'session.json').read_text(encoding='utf-8-sig'))
    report['rendering_profile'] = session['rendering_profile']
    report['keep_custom_scenery'] = session.get('keep_custom_scenery', False)
    report['visible_simulator'] = session.get('visible_simulator', False)
    report['rendering_preferences'] = session.get('rendering_preferences', {})
    for card in report['cards']:
        rows = list(csv.DictReader((directory/'cards'/card['card']/'trace.csv').open()))
        dt = [float(r['wall_frame_dt_s']) for r in rows]
        spikes = []
        for i in range(1, len(rows)):
            # Compare with surrounding ~2 seconds, excluding this candidate.
            lo, hi = max(1, i-60), min(len(rows), i+61)
            local = statistics.median(dt[lo:i]+dt[i+1:hi])
            if dt[i] > max(.025, 2*local) and dt[i]-local > .016:
                spikes.append({'row':i,'wall_elapsed_s':float(rows[i]['wall_elapsed_s']),
                    'sim_time':float(rows[i]['sim_time']),'phase':rows[i]['phase_id'],
                    'interval_ms':dt[i]*1000,'local_median_ms':local*1000,
                    'gpu_ms_approx':float(rows[i]['gpu_frame_time_s_approx'])*1000,
                    'smoothed_render_ms':float(rows[i]['render_frame_period_s'])*1000})
        card['relative_spikes'] = spikes
        card['relative_spikes_per_minute'] = len(spikes)*60/card['observed_wall_seconds']
        card['gpu_median_ms_approx'] = statistics.median(float(r['gpu_frame_time_s_approx']) for r in rows)*1000
    return report

if __name__ == '__main__':
    directory = BASE/sys.argv[1]
    data = analyze(directory)
    target = BASE/'STUTTER_FOLLOWUP_20260914'/(directory.name+'.json')
    target.write_text(json.dumps(data, indent=2)+'\n')
    for c in data['cards']:
        print(json.dumps({k:v for k,v in c.items() if k not in ['largest_frames','relative_spikes']}))
        print('relative_spikes', len(c['relative_spikes']), 'top', sorted(c['relative_spikes'],key=lambda r:r['interval_ms'], reverse=True)[:8])
    print('restored',data['restored'])
