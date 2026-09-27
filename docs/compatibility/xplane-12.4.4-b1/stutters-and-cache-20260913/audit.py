"""Verify retained measurements and recovery; run after all simulator processes close."""
import csv
import hashlib
import json
import math
from pathlib import Path

HERE=Path(__file__).resolve().parent
ROOT=Path('D:/X-Plane 12/Output/performance-tests')
FULL=[f'XPT_STUTTER_{name}_20260913' for name in ['A1','B1','C1','D1','D2','C2']]
LOAD=['XPT_STUTTER_B2_20260913']+[f'XPT_CACHE_LOAD_{name}_20260913' for name in ['L10a','L10b','L5b']]
PROBE=['XPT_STUTTER_PREP_PROBE_20260913']
records=[];traces=[];binary_sets=[]
for name in FULL+LOAD+PROBE:
    path=ROOT/name
    restoration=json.loads((path/'restoration.json').read_text(encoding='utf-8-sig'))
    assert all(restoration[k] is True for k in ['restored','source_hashes_match','helpers_removed','exact_name_sets_verified']),name
    session=json.loads((path/'session.json').read_text(encoding='utf-8-sig'))
    record={'run':name,'kind':'flights' if name in FULL else 'planned_load_only_stop' if name in LOAD else 'injected_prepare_failure',
        'settings':session.get('stutter_test'),'restoration':restoration}
    if session.get('simulator'):
        args=session['simulator']['arguments'];record['arguments']=args
        assert ('--gfx_no_defragment' in args)==session['stutter_test']['no_defragment']
    manifest=path/'source-manifest.json'
    if manifest.exists():
        source=json.loads(manifest.read_text())['harness_sha256']
        record['binary_hashes']={k:v for k,v in source.items() if k.endswith('.xpl')}
        binary_sets.append(record['binary_hashes'])
    error=path/'worker-error.json'
    if error.exists():record['worker_error']=json.loads(error.read_text())['error']
    if name in LOAD:
        assert record.get('worker_error','').startswith('Cancelled'),name
        assert (path/'load-benchmark.json').exists(),name
        assert not list((path/'cards').glob('*/trace.csv')),name
    if name in FULL:
        assert not error.exists(),name
        files=sorted((path/'cards').glob('*/trace.csv'));assert len(files)==2,name
        for file in files:
            raw=file.read_bytes()
            with file.open(newline='') as f:
                reader=csv.DictReader(f);header=reader.fieldnames;rows=list(reader)
            assert len(header)==89 and all(None not in row and None not in row.values() for row in rows)
            assert all(math.isfinite(float(r['wall_frame_dt_s'])) and float(r['wall_frame_dt_s'])>=0 for r in rows)
            assert sum(float(r['wall_frame_dt_s'])==0 for r in rows)==1
            assert all(float(b['native_steps'])==float(a['native_steps'])+1 for a,b in zip(rows,rows[1:]))
            assert all(int(b['flight_loop_counter'])==int(a['flight_loop_counter'])+2 for a,b in zip(rows,rows[1:]))
            assert all(abs(float(b['wall_elapsed_s'])-float(a['wall_elapsed_s'])-float(b['wall_frame_dt_s']))<3e-8 for a,b in zip(rows,rows[1:]))
            assessment=json.loads(file.with_name('assessment.json').read_text())
            assert assessment['measurement_valid'] and assessment['passed']
            assert assessment['native_samples']==len(rows)
            traces.append({'run':name,'card':file.parent.name,'rows':len(rows),'sha256':hashlib.sha256(raw).hexdigest(),
                'passed':assessment['passed'],'measurement_valid':assessment['measurement_valid'],'landing_metrics':assessment['metrics']})
    records.append(record)
assert len(traces)==12
assert all(b==binary_sets[0] for b in binary_sets)
out={'all_checks_passed':True,'flight_count':12,'session_restorations':len(records),
    'planned_load_only_sessions':len(LOAD),'native_rows':sum(t['rows'] for t in traces),
    'binary_hashes_identical':True,'traces':traces,'sessions':records}
(HERE/'audit.json').write_text(json.dumps(out,indent=2)+'\n')
print(json.dumps({k:v for k,v in out.items() if k not in ['traces','sessions']},indent=2))
