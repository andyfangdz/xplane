"""Command-line entry point. Flight control never depends on this polling loop."""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path
import sys
import time
import traceback
from .config import ROOT, REPO, atomic_json, resolve, sha

def source_hashes():
    paths=[]
    for relative,pattern in [('xpt','*.py'),('scripts','*.ps1'),('scripts','*.py'),('vendor/flight_test','*.py'),('build/SR20G6TestController','*.xpl'),('build/XPTNativeGuidance','*.xpl'),('build/XPTVideoHUD','*.xpl')]:
        paths.extend((ROOT/relative).rglob(pattern))
    paths.extend([ROOT/'Run-XPlaneTest.ps1',ROOT/'requirements.lock'])
    hashes = {p.relative_to(ROOT).as_posix():hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(set(paths))}
    workspace_paths = [REPO/'Cargo.toml', REPO/'Cargo.lock']
    for relative in ['crates/poweroff180', 'crates/xplane-plugin', 'crates/xplane-attitude', 'plugins/poweroff180-controller', 'plugins/poweroff180-hud', 'plugins/poweroff180-attitude']:
        for pattern in ['*.rs', '*.toml', 'parameters.csv']:
            workspace_paths.extend((REPO/relative).rglob(pattern))
    hashes.update({'workspace/'+p.relative_to(REPO).as_posix():hashlib.sha256(p.read_bytes()).hexdigest()
                   for p in sorted(set(workspace_paths))})
    return hashes

def worker(directory,port):
    from .adapter import NativeAdapter,XPlaneApi
    from .analysis import assess
    from .status import Status
    directory=Path(directory)
    resolved=resolve(directory/'requested-config.json')
    session=json.loads((directory/'session.json').read_text(encoding='utf-8-sig'))
    resolved['record_video']=bool(session.get('record_video',False))
    resolved['xplane_root']=session['xplane_root']
    atomic_json(directory/'resolved-config.json',resolved)
    native_dir=Path(session['xplane_root'])/'Aircraft/X-Aviation/TorqueSim SR20/plugins/XPTNativeGuidance'
    hashes=source_hashes()
    atomic_json(directory/'source-manifest.json',{'harness_sha256':hashes,'resolved_config_sha256':sha(resolved),
                'original_aircraft':session['source_hashes'],'adapter':'vendor/flight_test via xpt/adapter.py'})
    status=Status(directory);print(json.dumps({'status_url':status.start_server()}),flush=True)
    total=resolved['repeats']*len(resolved['cards']);completed=0
    try:
        with XPlaneApi(port=port,timeout=10) as api:
            deadline=time.monotonic()+240
            status.update('waiting_for_simulator',completed=0,total=total)
            while True:
                if (directory/'cancel.request').exists():raise RuntimeError('Cancelled before simulator readiness')
                try:api.request('GET','/datarefs?limit=1');break
                except Exception:
                    if time.monotonic()>deadline:raise TimeoutError('Simulator API did not become ready')
                    status.update('waiting_for_simulator');time.sleep(1)
            for repeat in range(1,resolved['repeats']+1):
                for card in resolved['cards']:
                    if (directory/'cancel.request').exists():raise RuntimeError('Cancelled before the next flight')
                    if source_hashes()!=hashes:raise RuntimeError('Harness source changed during the campaign')
                    label=f'{card["name"]}-{repeat:02d}';path=directory/'cards'/label
                    path.mkdir(parents=True,exist_ok=False)
                    effective={**resolved,'card_name':card['name'],'repeat':repeat,
                               'parameters':{**resolved['parameters'],'run_token':completed+1,
                                             'wind_speed_kt':card['wind_speed_kt'],'wind_offset_deg':card['wind_offset_deg']}}
                    # Remove matrix-only fields from the per-flight authoritative document.
                    for key in ('cards','repeats'):effective.pop(key)
                    atomic_json(path/'effective-config.json',effective)
                    status.update('loading',card=label,completed=completed,total=total)
                    adapter=NativeAdapter(api,effective,path,native_dir,status)
                    document,rows=adapter.execute()
                    assessment=assess(document,rows)
                    atomic_json(path/'result.json',document);atomic_json(path/'assessment.json',assessment)
                    completed+=1
                    status.update('flight_recorded',completed=completed,last_result=assessment,event='flight_recorded')
                    print(json.dumps({'card':label,'completed':completed,'total':total,'passed':assessment['passed'],'reasons':assessment['reasons']}),flush=True)
                    if document['terminal']['phase']=='aborted':
                        raise RuntimeError('Native test aborted: '+document['terminal']['reason']+'; restart from a fresh session')
        status.update('awaiting_restoration',completed=completed,total=total)
    except BaseException as error:
        atomic_json(directory/'worker-error.json',{'error':str(error),'traceback':traceback.format_exc()})
        status.update('failed',error=str(error));raise
    finally:
        status.close()

def main():
    parser=argparse.ArgumentParser(description='Native X-Plane test harness')
    sub=parser.add_subparsers(dest='command',required=True)
    validate=sub.add_parser('validate');validate.add_argument('--config',type=Path,required=True)
    work=sub.add_parser('worker');work.add_argument('--run',type=Path,required=True);work.add_argument('--port',type=int,default=8153)
    report=sub.add_parser('report');report.add_argument('--run',type=Path,required=True);report.add_argument('--baseline',type=Path)
    status=sub.add_parser('status');status.add_argument('--run',type=Path,required=True);status.add_argument('--watch',action='store_true')
    cancel=sub.add_parser('cancel');cancel.add_argument('--run',type=Path,required=True)
    args=parser.parse_args()
    if args.command=='validate':
        config=resolve(args.config);print(json.dumps({'valid':True,'config_sha256':sha(config),'cards':len(config['cards']),'repeats':config['repeats']}))
    elif args.command=='worker':worker(args.run,args.port)
    elif args.command=='report':
        from .report import build
        baseline=args.baseline
        if baseline is None:
            session=json.loads((args.run/'session.json').read_text(encoding='utf-8-sig'))
            baseline=Path(session['xplane_root'])/'Output/performance-tests/TORQUESIM_SR20_SMOOTH_20260908'
        result=build(args.run,baseline)
        print(json.dumps({k:result[k] for k in ['flights','strict_passes','measurement_valid','restoration']}))
    elif args.command=='status':
        previous=None
        while True:
            path=args.run/'status.json'
            data=json.loads(path.read_text(encoding='utf-8')) if path.exists() else {'phase':'preparing_session','revision':0}
            data['age_seconds']=time.time()-data.get('updated_epoch',time.time())
            if previous!=data.get('revision') or not args.watch:print(json.dumps(data),flush=True);previous=data.get('revision')
            if not args.watch or data['phase'] in ('restored','recovery_pending'):break
            time.sleep(.5)
    elif args.command=='cancel':
        atomic_json(args.run/'cancel.request',{'requested_epoch':time.time()});print('Cancellation requested; recovery will follow.')

if __name__=='__main__':main()
