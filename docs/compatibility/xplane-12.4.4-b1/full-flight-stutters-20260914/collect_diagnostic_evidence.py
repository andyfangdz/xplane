"""Offline audit and evidence bundle for the separate 5/4/5 texture diagnostic."""
import csv
import gzip
import hashlib
import json
from pathlib import Path
import re
import shutil
import statistics

HERE = Path(__file__).resolve().parent
BASE = Path('D:/X-Plane 12/Output/performance-tests')
RUNS = [('XPT_FLAG_ON_MICROPROFILE_20260914', 5),
        ('XPT_FLAG_ON_TEX4_DIAGNOSTIC_20260914', 4),
        ('XPT_FLAG_ON_TEX5_CONFIRM_20260914', 5)]

def read(path):
    return json.loads(path.read_text(encoding='utf-8-sig'))

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def main():
    summaries, manifests, captures, files = [], [], [], []
    for name, texture in RUNS:
        run = BASE / name
        report = read(run / 'flight-pacing.json')
        session = read(run / 'session.json')
        restoration = read(run / 'restoration.json')
        source = read(run / 'source-manifest.json')
        assert report['complete'] and restoration['restored'] and restoration['source_hashes_match']
        assert session['stutter_test'] == {'no_defragment': True, 'texture_cache_gb': 10}
        assert session['rendering_profile'] == 'User2D' and session['keep_custom_scenery']
        assert '--gfx_no_defragment' in session['simulator']['arguments']
        assert "'--gfx_no_defragment'" in (run/'Log.txt').read_text(encoding='utf-8-sig',errors='replace')
        prefs = (run / 'after-preferences/X-Plane.prf').read_text(encoding='utf-8-sig')
        for key, expected in [('_tex_res', texture), ('_tex_cache_gb', 10), ('_eq_vr', 0)]:
            assert int(re.search(r'^' + re.escape(key) + r'\s+(\d+)\s*$', prefs, re.M)[1]) == expected
        manifests.append(source['harness_sha256'])
        events_path = run / 'microprofile/capture-events.json'
        events = read(events_path) if events_path.exists() else []
        for flight in report['flights']:
            card = flight['card']
            card_path = run / 'cards' / card
            rows = list(csv.DictReader((card_path / 'pacing.csv').open()))
            event = next((e for e in events if e['card'] == card), None)
            cutoff = event['request_start_qpc'] if event else flight['flight_end_qpc']
            freq = flight['qpc_frequency']
            intervals = [(int(a['qpc']), int(b['qpc'])) for a, b in zip(rows, rows[1:])
                         if a['paused'] == b['paused'] == '0'
                         and int(a['qpc']) >= flight['flight_start_qpc'] + 5*freq
                         and int(b['qpc']) <= cutoff]
            values = [(b-a)*1000/freq for a,b in intervals]
            hitches = [dict(qpc_start=a, qpc_end=b, interval_ms=(b-a)*1000/freq,
                            seconds_from_unpause=(a-flight['flight_start_qpc'])/freq)
                       for a,b in intervals if (b-a)*1000/freq > 50]
            summaries.append({'run': name, 'texture_resolution': texture, 'card': card,
                'timing_valid': flight['valid'], 'landing_passed': flight['landing_passed'],
                'pre_request_seconds_after_activation': sum(values)/1000,
                'pre_request_max_ms': max(values), 'pre_request_over50': len(hitches),
                'pre_request_hitches': hitches, 'captured': event is not None,
                'full_primary_diagnostic_only': flight['primary']})
            if event:
                hit_index = next(i for i,row in enumerate(rows) if int(row['qpc']) == event['hitch_end_qpc'])
                assert hit_index + 1 == int(event['snapshot'][2])
                native_dt = float(rows[hit_index]['wall_frame_dt_s'])*1000
                assert abs(native_dt-event['snapshot'][3]) < .001
                assert event['hitch_end_qpc'] < event['request_start_qpc']
                profile = read(run / 'microprofile' / (card+'.analysis.json'))
                durations = [f['duration_ms'] for f in profile['frames']]
                native = [float(row['wall_frame_dt_s'])*1000 for row in rows]
                fits=[]
                for offset in range(hit_index-len(durations)-20, hit_index-len(durations)+30):
                    v=native[offset:offset+len(durations)]
                    if offset < 0 or len(v)!=len(durations): continue
                    errors=[a-b for a,b in zip(v,durations)]
                    fits.append({'native_offset':offset,'mse_ms2':statistics.mean(x*x for x in errors),
                                 'median_abs_ms':statistics.median(abs(x) for x in errors)})
                fits.sort(key=lambda x:x['mse_ms2'])
                frame_index=hit_index-fits[0]['native_offset']
                captures.append({'run':name,'card':card,'native_dt_ms':native_dt,
                    'hitch_end_qpc':event['hitch_end_qpc'], 'native_row_index':hit_index,
                    'request_delay_ms':(event['request_start_qpc']-event['hitch_end_qpc'])*1000/freq,
                    'request_duration_ms':(event['request_end_qpc']-event['request_start_qpc'])*1000/freq,
                    'alignment_candidates':fits[:5], 'estimated_profiler_frame_index':frame_index,
                    'hitch_within_profiler_capture':0 <= frame_index < len(profile['frames']),
                    'matched_profiler_frame':profile['frames'][frame_index] if 0 <= frame_index < len(profile['frames']) else None,
                    'limitation':'Profiler-relative frame boundaries differ from native callback boundaries; sequence alignment is empirical, not an absolute QPC mapping.'})
        target=HERE/'diagnostic-evidence'/name
        keep=['session.json','source-manifest.json','restoration.json','flight-pacing.json',
              'protected-state-verification.json','requested-config.json','Log.txt','microprofile/observer.json',
              'microprofile/observer-errors.json','microprofile/observer-finished.json',
              'microprofile/capture-events.json']
        for card in ('calm-01','calm-02'):
            keep += [f'cards/{card}/{f}' for f in ('pacing.json','pacing-assessment.json','pacing-readiness.json','assessment.json','trace.csv','pacing.csv')]
            keep += [f'microprofile/{card}.html']
        for relative in keep:
            path=run/relative
            if not path.exists(): continue
            dst=target/relative
            dst.parent.mkdir(parents=True,exist_ok=True)
            if dst.suffix in ('.html','.csv'):
                dst=dst.with_suffix(dst.suffix+'.gz')
                dst.write_bytes(gzip.compress(path.read_bytes(),mtime=0))
            else: shutil.copy2(path,dst)
            files.append({'source':str(path),'source_sha256':sha(path),'retained':str(dst.relative_to(HERE)),
                          'retained_sha256':sha(dst)})
    assert all(s==manifests[0] for s in manifests), 'Sources changed between diagnostic sessions'
    (HERE/'diagnostic-results.json').write_text(json.dumps({'same_sources':True,'flights':summaries,
        'captures':captures,'evidence':files},indent=2),encoding='utf-8')
    print(json.dumps({'flights':summaries,'captures':captures},indent=2))

if __name__=='__main__':main()
