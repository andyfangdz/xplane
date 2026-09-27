from pathlib import Path
from datetime import datetime,timezone
import gzip,hashlib,json,shutil

H=Path(__file__).resolve().parent
RUN=Path('D:/X-Plane 12/Output/performance-tests/XPT_HAGSOFF_WPR_20260915')
DEST=Path('V:/src/xplane/docs/compatibility/xplane-12.4.4-b1/full-flight-stutters-20260914/hags-trace-evidence')
DEST.mkdir(exist_ok=True)
entries=[]
def digest(p):
    h=hashlib.sha256()
    with p.open('rb') as f:
        for block in iter(lambda:f.read(1024*1024),b''):h.update(block)
    return h.hexdigest()
def copy(p,rel,compress=False):
    out=DEST/rel;out.parent.mkdir(parents=True,exist_ok=True)
    if compress:
        with p.open('rb') as src,gzip.open(out,'wb',compresslevel=5) as dst:shutil.copyfileobj(src,dst)
    else:shutil.copy2(p,out)
    entries.append(dict(path=str(out.relative_to(DEST)).replace('\\','/'),bytes=out.stat().st_size,
        sha256=digest(out),source=str(p),source_sha256=digest(p)))
for name in ('plan.json','trace-summary.json','coverage.json','hitch-analysis.json','worker-analysis.json',
             'symbol-queries.json','resolved-symbols.json','symbol-image-verification.json','final-audit.json',
             'host-stop-request.json','blocking-chain.png','blocking-chain.svg','prepare_analysis.py',
             'inspect_hitches.py','inspect_worker.py','verify_symbols.py','make_figure.py','Resolve-Symbols.ps1','bundle_evidence.py'):
    copy(H/name,'analysis/'+name)
for n in (1,2,3):copy(H/f'hitch-{n}.json',f'analysis/hitch-{n}.json.gz',True)
for name in ('session.json','source-manifest.json','requested-config.json','resolved-config.json','restoration.json',
             'flight-pacing.json','Log.txt','protected-install-state.json','XPT_HAGSOFF_WPR_20260915-nv-before.json',
             'XPT_HAGSOFF_WPR_20260915-nv-applied.json','XPT_HAGSOFF_WPR_20260915-nv-restored.json'):
    copy(RUN/name,'run/'+name)
for card in ('calm-01','calm-02'):
    for name in ('pacing.csv','trace.csv'):
        copy(RUN/'cards'/card/name,f'run/cards/{card}/{name}.gz',True)
    for name in ('pacing.json','pacing-assessment.json','pacing-readiness.json','pacing-start.json','assessment.json','effective-config.json'):
        copy(RUN/'cards'/card/name,f'run/cards/{card}/{name}')
for folder,names in [('gpu-memory',('memory.csv','collector.json','finished.json')),
                     ('segments/calm-02-diagnostic',('observer.json','wpr-start.json','stop-request.json','wpr-stop.json','presentmon.csv','presentmon-decode.json'))]:
    for name in names:
        p=RUN/folder/name
        if p.exists():copy(p,f'run/{folder}/{name}')
etl=RUN/'segments/calm-02-diagnostic/system.etl'
manifest=dict(utc=datetime.now(timezone.utc).isoformat(),files=entries,
    raw_etl=dict(path=str(etl),bytes=etl.stat().st_size,sha256=digest(etl)))
(DEST/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n',encoding='utf-8')
print(json.dumps(dict(files=len(entries),raw_etl=manifest['raw_etl']),indent=2))
