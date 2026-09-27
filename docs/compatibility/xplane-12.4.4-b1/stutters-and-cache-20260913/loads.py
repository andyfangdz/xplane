"""Recover load readiness timings from contemporaneous saved markers and events."""
import csv
from datetime import datetime, timezone
import json
from pathlib import Path
from statistics import mean, median
import collect

HERE=Path(__file__).resolve().parent

def utc(epoch):return datetime.fromtimestamp(epoch,timezone.utc).isoformat()

def build():
    rows=[]
    for name in collect.ORDER:
        path=collect.ROOT/f'XPT_STUTTER_{name}_20260913'
        session=path/'session.json'
        if not session.exists():continue
        if (path/'load-benchmark.json').exists():continue  # Focused capacity study is reported separately.
        manifest=json.loads(session.read_text(encoding='utf-8-sig'))
        if not manifest.get('simulator'):continue
        launch=datetime.fromisoformat(manifest['simulator']['start_time']).timestamp()
        events=[]
        if (path/'events.jsonl').exists():
            for line in (path/'events.jsonl').read_text().splitlines():
                try:events.append(json.loads(line))
                except json.JSONDecodeError:pass  # An in-progress final line is retried later.
        for card in sorted((path/'cards').glob('*')):
            request=card/'flight-request.json';resume=card/'startup-resume.json'
            if not request.exists() or not resume.exists():continue
            start=request.stat().st_mtime_ns/1e9;end=resume.stat().st_mtime_ns/1e9
            resumestate=json.loads(resume.read_text())
            ev=[e for e in events if e.get('card')==card.name]
            loading=next((e['updated_epoch'] for e in ev if e['phase']=='loading'),None)
            activated=next((e['updated_epoch'] for e in ev if e['phase']=='waiting_for_aircraft_activation'),None)
            ready=next((e['updated_epoch'] for e in ev if e['phase']=='downwind'),None)
            result={
                'run':name,'card':card.name,'cache_gb':manifest['stutter_test']['texture_cache_gb'],
                'flag':manifest['stutter_test']['no_defragment'],
                'load_kind':'first_in_process' if card.name.endswith('01') else 'same_process_reload',
                'request_marker_utc':utc(start),'first_resume_marker_utc':utc(end),
                'request_marker_mtime_ns':request.stat().st_mtime_ns,'resume_marker_mtime_ns':resume.stat().st_mtime_ns,
                'load_to_first_resume_seconds':end-start,
                'launch_to_first_resume_seconds':end-launch if card.name.endswith('01') else None,
                'loading_event_to_request_marker_seconds':start-loading if loading else None,
                'initial_load_and_setup_seconds':activated-loading if activated and loading else None,
                'complete_setup_seconds':ready-loading if ready and loading else None,
                'menu_recovery_attempts':resumestate['recovery_attempts'],
                'paused_after_resume':resumestate['paused_after']}
            rows.append(result)
    groups=[]
    for cache in [0,5,10]:
        for kind in ['first_in_process','same_process_reload']:
            values=[r['load_to_first_resume_seconds'] for r in rows if r['cache_gb']==cache and r['load_kind']==kind]
            if not values:continue
            groups.append({'cache_gb':cache,'kind':kind,'n':len(values),'mean_seconds':mean(values),
                'median_seconds':median(values),'min_seconds':min(values),'max_seconds':max(values)})
    data={'method':'Original file mtimes bracket saved request payload immediately before POST /flight and successful first unpause readback. Includes catalog/readiness checks, excludes later fuel/mass setup; not a GPU-ready presentation timestamp. Original ns timestamps are retained before any artifact copying.','rows':rows,'cache_groups':groups}
    (HERE/'load-results.json').write_text(json.dumps(data,indent=2)+'\n')
    if rows:
        with (HERE/'loads.csv').open('w',newline='') as f:
            writer=csv.DictWriter(f,fieldnames=list(rows[0]));writer.writeheader();writer.writerows(rows)
    print(json.dumps(groups,indent=2))
    return data

if __name__=='__main__':build()
