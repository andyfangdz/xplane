"""Verify final persistent settings, recovery, and native trace integrity."""
import csv
import hashlib
import json
from pathlib import Path
import sys
from analyze_followup import analyze

base=Path('D:/X-Plane 12/Output/performance-tests')
out=base/'STUTTER_FOLLOWUP_20260914'
runs=['XPT_USER_PROFILE_PREP_20260914','XPT_USER_FLAG10_A_20260914',
      'XPT_USER_FLAG10_B_20260914','XPT_USER_FLAG10_VISIBLE_20260914',
      'XPT_USER_FLAG10_VR_20260914','XPT_USER_FLAG10_TEX4_20260914',
      'XPT_USER_FLAG10_CONTROL_20260914','XPT_USER_FLAG10_TEX4_CONFIRM_20260914']

def read(path):return json.loads(path.read_text(encoding='utf-8-sig'))
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()

if __name__=='__main__':
    result={'settings':[],'sessions':[],'traces':[]}
    lower=base/'XPT_USER_FLAG10_TEX4_20260914'
    control=base/'XPT_USER_FLAG10_CONTROL_20260914'
    confirm=base/'XPT_USER_FLAG10_TEX4_CONFIRM_20260914'
    result['comparison_controls']={
        'requested_config_matches':sha(lower/'requested-config.json')==sha(control/'requested-config.json'),
        'harness_and_binary_hashes_match':read(lower/'source-manifest.json')['harness_sha256']==read(control/'source-manifest.json')['harness_sha256'],
        'original_aircraft_hashes_match':read(lower/'session.json')['source_hashes']==read(control/'session.json')['source_hashes'],
        'original_preferences_match':read(lower/'session.json')['states'][0]['files']==read(control/'session.json')['states'][0]['files'],
        'original_scenery_inventory_matches':read(lower/'session.json')['scenery_inventory']==read(control/'session.json')['scenery_inventory'],
        'original_scenery_index_matches':sha(lower/'original-scenery_packs.ini')==sha(control/'original-scenery_packs.ini'),
        'confirmation_config_matches':sha(lower/'requested-config.json')==sha(confirm/'requested-config.json'),
        'confirmation_harness_matches':read(lower/'source-manifest.json')['harness_sha256']==read(confirm/'source-manifest.json')['harness_sha256'],
        'confirmation_preferences_match':read(lower/'session.json')['states'][0]['files']==read(confirm/'session.json')['states'][0]['files']}
    for change in read(out/'settings-change.json')['changes']:
        actual=sha(Path(change['path']))
        result['settings'].append({'path':change['path'],'sha256':actual,'matches':actual==change['after_sha256']})
    for name in runs:
        directory=base/name;session=read(directory/'session.json');restored=read(directory/'restoration.json')
        index=session.get('scenery_index')
        index_matches=not index or sha(Path(index['live'])).lower()==index['sha256'].lower()
        result['sessions'].append({'run':name,'restoration':restored,'scenery_index_matches':index_matches,
            'error':read(directory/'worker-error.json')['error'] if (directory/'worker-error.json').exists() else None,
            'runner_error':read(directory/'runner-error.json') if (directory/'runner-error.json').exists() else None,
            'diagnostic_stop':read(directory/'diagnostic-stop.json') if (directory/'diagnostic-stop.json').exists() else None,
            'recovery_error_retained':read(directory/'recovery-error.json') if (directory/'recovery-error.json').exists() else None})
        if list((directory/'cards').glob('*/trace.csv')):
            summary=analyze(directory)
            (out/(name+'.json')).write_text(json.dumps(summary,indent=2)+'\n')
        for path in sorted((directory/'cards').glob('*/trace.csv')):
            rows=list(csv.DictReader(path.open()))
            assessment=read(path.parent/'assessment.json')
            continuous=all(float(b['native_steps'])==float(a['native_steps'])+1 for a,b in zip(rows,rows[1:]))
            clock=all(abs(float(b['wall_elapsed_s'])-float(a['wall_elapsed_s'])-float(b['wall_frame_dt_s']))<3e-8 for a,b in zip(rows,rows[1:]))
            result['traces'].append({'run':name,'card':path.parent.name,'rows':len(rows),'sha256':sha(path),
                'continuous':continuous,'clock_consistent':clock,'landing_passed':assessment['passed'],
                'measurement_valid':assessment['measurement_valid'],'landing_reasons':assessment['reasons']})
    result['all_checks_passed']=(all(result['comparison_controls'].values()) and all(x['matches'] for x in result['settings'])
        and all(x['restoration']['restored'] and x['scenery_index_matches'] for x in result['sessions'])
        and all(x['continuous'] and x['clock_consistent'] and x['measurement_valid'] for x in result['traces']))
    (out/'audit.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result,indent=2))
    sys.exit(0 if result['all_checks_passed'] else 1)
