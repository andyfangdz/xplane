import gzip,hashlib,json,shutil
from pathlib import Path

H=Path(__file__).resolve().parent
DOC=H.parents[1]/'docs/compatibility/xplane-12.4.4-b1/full-flight-stutters-20260914'
DEST=DOC/'presentation-evidence'
DEST.mkdir(exist_ok=True)
BASE=Path('D:/X-Plane 12/Output/performance-tests')
def read(p):return json.loads(p.read_text(encoding='utf-8-sig'))
def sha(p):
    h=hashlib.sha256()
    with p.open('rb') as f:
        for block in iter(lambda:f.read(1024*1024),b''):h.update(block)
    return h.hexdigest()
audit=read(H/'final-audit.json')
prior=DOC/'wpr-vram-final-audit.json'
expected={str(Path(x['path'])).lower():x['sha256'] for x in read(prior)['persistent_files']}
for x in audit['critical_files']:
    x['expected_sha256']=expected[str(Path(x['path'])).lower()]
    x['matches_expected']=x['sha256'].lower()==x['expected_sha256'].lower()
    assert x['matches_expected']
audit['expected_hash_source']=str(prior)
assert all(not p['alive'] for p in audit['owned_processes'])
assert not audit['wpr_host']['ready'] and audit['task_state']=='Ready'
original,final=read(H/'nvidia-original.json'),read(H/'nvidia-final.json')
assert original['profile_settings']==final['profile_settings'] and original['setting']==final['setting']
audit['nvidia_profile_verified_original']=True
(H/'final-audit.json').write_text(json.dumps(audit,indent=2)+'\n',encoding='utf-8')
evidence=[]
def keep(p,target):
    target.parent.mkdir(parents=True,exist_ok=True)
    if p.suffix=='.csv' or p.name in ('dxgi-worst.json','dxgi-early.json'):
        target=target.with_name(target.name+'.gz')
        with p.open('rb') as src,gzip.open(target,'wb') as dst:shutil.copyfileobj(src,dst)
    else:shutil.copy2(p,target)
    evidence.append(dict(source=str(p),destination=str(target.relative_to(DOC)),source_sha256=sha(p),artifact_sha256=sha(target)))
files=['presentation-results.json','dxgi-coverage.json','dxgi-early.json','dxgi-worst.json','dxgi-waits.json',
       'resolved-symbols.json','symbol-image-verification.json','symbol-queries.json','cpu-inspection.txt',
       'nvidia-original.json','nvidia-final.json','final-audit.json','harness-tests.log',
       'nv_present.py','Run-PresentTest.ps1','summarize_present.py','extract_waits.py','inspect_cpu.py','verify_images.py','Resolve-Symbols.ps1','bundle_evidence.py']
files += [p.name for p in H.glob('*-nv-*.json')]
for file in files:keep(H/file,DEST/'analysis'/file)
for p in (H/'nvapi').iterdir():
    if p.is_file():keep(p,DEST/'analysis/nvapi'/p.name)
for name in ['XPT_DXGI_B1_20260914','XPT_DXGI_TRACE_20260914']:
    run=BASE/name
    names=['session.json','source-manifest.json','restoration.json','flight-pacing.json','recovery-error.json','Log.txt',
           'gpu-memory/collector.json','gpu-memory/finished.json','gpu-memory/memory.csv']
    names += [f'cards/{c}/{f}' for c in ['calm-01','calm-02'] for f in ['pacing.csv','pacing.json','assessment.json','pacing-readiness.json','rendering-readback.json']]
    names += ['segments/calm-02-diagnostic/'+f for f in ['observer.json','wpr-start.json','wpr-stop.json','stop-request.json','presentmon.csv','presentmon-decode.json','presentmon-decode.log']]
    for file in names:
        p=run/file
        if p.exists():keep(p,DEST/name/file)
etl=BASE/'XPT_DXGI_TRACE_20260914/segments/calm-02-diagnostic/system.etl'
manifest=dict(evidence=evidence,etl=dict(path=str(etl),sha256=sha(etl),bytes=etl.stat().st_size),
              diagnostic_only=True,not_a_completed_new_ab_benchmark=True)
(DEST/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n',encoding='utf-8')
print(json.dumps(dict(artifacts=len(evidence),etl_bytes=etl.stat().st_size,all_critical_settings_original=True,nvidia_original=True)))
