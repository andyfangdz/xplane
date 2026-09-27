"""Repeatable rendering trials; the native plugin owns each measured interval."""
import argparse
import base64
from datetime import datetime
import json
import math
from pathlib import Path
import shutil
import time
import traceback
import uuid
from .config import atomic_json, sha
from .perf_config import resolve, segments, flight_request
from .perf_capture import PresentMon, WprHost, check_presentmon, qpc, decode_etl

SNAPSHOT='xpt/perf/snapshot'

def preflight(c,root):
    tools={'presentmon':check_presentmon(),'wpr_requested':c['wpr']}
    if c['wpr']:
        host=WprHost(root);tools['wpr_host']=host.request('ping')
    return tools

def worker(directory,port):
    from .launch import wait_for_launch
    from .adapter import XPlaneApi
    from .cli import source_hashes
    from .readiness import check_startup, SimulatorStartupError
    from .status import Status
    directory=Path(directory)
    session=wait_for_launch(directory)
    root=Path(session['xplane_root']);c=resolve(directory/'requested-config.json')
    native=root/'Aircraft/X-Aviation/TorqueSim SR20/plugins/XPTPerformanceRecorder'
    tools=preflight(c,root);hashes=source_hashes()
    atomic_json(directory/'resolved-config.json',c)
    atomic_json(directory/'source-manifest.json',{'harness_sha256':hashes,'resolved_config_sha256':sha(c),'tools':tools})
    status=Status(directory);status.start_server()
    pm=None;host=WprHost(root) if c['wpr'] else None
    active_wpr=None;active_path=None;measurement=False
    launched_epoch=datetime.fromisoformat(session['simulator']['start_time']).timestamp()
    def guard(read_log=True):
        if (directory/'cancel.request').exists():raise RuntimeError('Cancelled; partial evidence retained')
        if read_log:check_startup(root,launched_epoch)
    def wait_for(label,action,timeout=600):
        status.update(label)
        deadline=time.monotonic()+timeout;last=None
        while True:
            guard()
            try:
                result=action()
                if result:
                    status.update(label,last_wait_error=None)
                    return result
            except SimulatorStartupError:raise
            except Exception as e:last=str(e)
            if time.monotonic()>deadline:raise TimeoutError(f'{label}: {last or "readback gate not reached"}')
            status.update(label,last_wait_error=last)
            time.sleep(1)
    try:
        with XPlaneApi(port=port,timeout=5) as api:
            wait_for('waiting_for_simulator',lambda:api.request('GET','/datarefs?limit=1'))
            request=flight_request(c);atomic_json(directory/'flight-request.json',request)
            loading_qpc=session.get('launch_qpc',qpc());loading_epoch=launched_epoch;status.update('loading_scene')
            # A transport timeout cannot safely be retried: the load may have started.
            from flight_test.api import XPlaneConnectionError
            if not session.get('performance_launch_json'):
                try:api.load_flight(request)
                except XPlaneConnectionError as e:
                    atomic_json(directory/'load-transport.json',{'uncertain_acceptance':True,'error':str(e)})
            def catalog_ready():
                api.refresh_catalogs()
                return SNAPSHOT in api.datarefs and 'xpt/perf/start' in api.commands
            wait_for('waiting_for_recorder',catalog_ready)
            path=api.get_raw('sim/aircraft/view/acf_relative_path')
            if isinstance(path,str):path=base64.b64decode(path).decode(errors='replace').rstrip('\0')
            elif isinstance(path,list):path=bytes(path).decode(errors='replace').rstrip('\0')
            if str(path).replace('\\','/')!=request['data']['aircraft']['path']:
                raise RuntimeError(f'Aircraft readback mismatch: {path!r}')
            prior_heartbeat=None;stable_samples=0
            def scene_ready():
                nonlocal prior_heartbeat,stable_samples
                s=api.get_raw(SNAPSHOT)
                valid=len(s)==16 and s[12]==1 and s[14]==0
                stable_samples=stable_samples+1 if valid and prior_heartbeat is not None and s[15]!=prior_heartbeat else 0
                prior_heartbeat=s[15]
                return s if stable_samples>=3 else None
            wait_for('waiting_for_scenery_and_advancing_frames',scene_ready,600)
            api.command('sim/operation/pause_on',.2)
            wait_for('waiting_for_paused_scene',lambda:api.get_scalar('sim/time/paused')==1,30)
            from .config import PARAMETERS
            lat=api.get_scalar('sim/flightmodel/position/latitude')
            lon=api.get_scalar('sim/flightmodel/position/longitude')
            heading=api.get_scalar('sim/flightmodel/position/psi')
            target_lat=PARAMETERS['threshold_lat'][0];target_lon=PARAMETERS['threshold_lon'][0]
            north=(lat-target_lat)*111320;east=(lon-target_lon)*111320*math.cos(math.radians(target_lat))
            runway_heading=math.degrees(math.atan2((PARAMETERS['end_lon'][0]-target_lon)*math.cos(math.radians(target_lat)),PARAMETERS['end_lat'][0]-target_lat))%360
            position={'latitude':lat,'longitude':lon,'heading_true_deg':heading,
                      'threshold_distance_m':math.hypot(north,east),'heading_error_deg':(heading-runway_heading+180)%360-180}
            atomic_json(directory/'scene-position.json',position)
            if position['threshold_distance_m']>100 or abs(position['heading_error_deg'])>3:
                raise RuntimeError(f'Requested runway scene was not achieved: {position}')
            api.command('sim/view/3d_cockpit_cmnd_look',.2)
            api.set_dataref('sim/graphics/view/pilots_head_psi',0)
            api.set_dataref('sim/graphics/view/pilots_head_the',0)
            scene_ready_qpc=qpc();scene_ready_epoch=time.time()
            def ready():
                s=api.get_raw(SNAPSHOT)
                if len(s)!=16 or s[12]!=1:raise RuntimeError('Performance plugin protocol mismatch')
                if s[7]!=0:raise RuntimeError('Benchmark requires actual 2D mode')
                return s if s[6]==1 and s[8]==1 and abs(s[9])<.5 and s[14]==0 else None
            s=wait_for('waiting_for_foreground_parked_scene',ready,180)
            atomic_json(directory/'load-timing.json',{'request_qpc':loading_qpc,'ready_qpc':qpc(),
                        'request_epoch':loading_epoch,'seconds_to_gated_scene':time.time()-loading_epoch,
                        'scene_ready_qpc':scene_ready_qpc,'seconds_to_paused_plugin_ready':scene_ready_epoch-loading_epoch,
                        'focus_wait_seconds':time.time()-scene_ready_epoch,
                        'definition':'Request to paused, foreground, plugin-ready scene; includes focus wait. Not disk-only load time.'})
            atomic_json(directory/'readiness.json',{'aircraft':path,'native_snapshot':s,'log':check_startup(root),
                        'mass_kg':api.get_scalar('sim/flightmodel/weight/m_total')})
            # A single external stream spans all segments; exact native QPC windows
            # exclude startup, warmup, export, WPR saving, and between-segment work.
            if not host:
                pm=PresentMon(directory,session['simulator']['pid']);pm.start()
            for index,segment in enumerate(segments(c),1):
                guard()
                if pm:pm.check()
                if source_hashes()!=hashes:raise RuntimeError('Harness source changed during the campaign')
                label=segment['label'];active_path=directory/'segments'/label
                active_path.mkdir(parents=True,exist_ok=False)
                token=index
                atomic_json(active_path/'segment.json',{**segment,'token':token,'config_sha256':sha(c),
                    'duration_seconds':c['duration_seconds'],'amplitude_degrees':c['amplitude_degrees']})
                # Every repetition starts at the same camera orientation.
                api.set_dataref('sim/graphics/view/pilots_head_psi',0)
                api.set_dataref('sim/graphics/view/pilots_head_the',0)
                wait_for('waiting_for_foreground_parked_scene',ready,60)
                if host:
                    # Persist stop authority before submitting a start. Instance names
                    # are deterministic even if the response is interrupted.
                    active_wpr={'nonce':host.host['nonce'],'instance':'XPT_'+host.host['nonce']+'_'+session['token'][:8]+'_'+label,
                                'run_directory':str(directory),'segment':label}
                    atomic_json(active_path/'wpr-start.json',active_wpr)
                    active_wpr.update(host.request('start',run_directory=str(directory),segment=label))
                    atomic_json(active_path/'wpr-start.json',active_wpr)
                config={'token':token,'warmup_seconds':c['warmup_seconds'],'duration_seconds':c['duration_seconds'],
                        'record':int(segment['record']),'sweep':1,'amplitude_degrees':c['amplitude_degrees'],
                        'period_seconds':c['period_seconds'],'expected_vr':0,'require_foreground':1}
                text=''.join(f'{k}={v}\n' for k,v in config.items())
                (active_path/'performance.ini').write_text(text)
                temporary=native/'performance.ini.tmp';temporary.write_text(text);temporary.replace(native/'performance.ini')
                api.command('xpt/perf/configure',.1);time.sleep(.15)
                s=api.get_raw(SNAPSHOT)
                if s[0]!=0 or int(s[3])!=token:raise RuntimeError(f'Native configuration rejected: {s}')
                status.update('recording',card=label,completed=index-1,total=c['repeats']*len(c['variants']))
                api.command('xpt/perf/start',.1);measurement=True
                deadline=time.monotonic()+c['duration_seconds']+c['warmup_seconds']+30
                marks=[]
                while True:
                    # No screen capture, profile dump, CSV polling, or per-frame
                    # Python telemetry in this loop. Native callback controls time.
                    guard(read_log=False)
                    if pm:pm.check()
                    for marker in directory.glob('mark-*.request'):
                        if marker.with_suffix('.sent.json').exists():continue
                        sent=qpc();api.command('xpt/perf/mark',.1)
                        record={'request':json.loads(marker.read_text()),'sent_qpc':sent,'segment':label}
                        atomic_json(marker.with_suffix('.sent.json'),record);marks.append(record)
                    s=api.get_raw(SNAPSHOT)
                    if s[0] in (3,4):break
                    if time.monotonic()>deadline:raise TimeoutError('Native segment did not terminate')
                    time.sleep(1)
                measurement=False
                atomic_json(active_path/'terminal.json',{'snapshot':s,'marker_requests':marks})
                for suffix in ['csv','json']:
                    source=native/f'performance-{token}.{suffix}'
                    if source.exists():shutil.copy2(source,active_path/f'native.{suffix}')
                if host:
                    status.update('saving_system_trace',card=label)
                    atomic_json(active_path/'wpr-stop.json',host.request('stop',instance=active_wpr['instance']))
                    active_wpr=None
                    status.update('decoding_presentations',card=label)
                    decode_etl(active_path,session['simulator']['pid'])
                if s[0]!=3 or s[4]!=0:raise RuntimeError(f'Invalid native segment; state={s[0]}, flags={s[4]}')
                status.update('segment_recorded',completed=index,event='segment_recorded')
            status.update('awaiting_restoration')
    except BaseException as e:
        error={'error':str(e),'traceback':traceback.format_exc()}
        atomic_json(directory/'worker-error.json',error)
        if active_path:atomic_json(active_path/'error.json',error)
        status.update('failed',error=str(e));raise
    finally:
        # Simulator shutdown also exports an interrupted native buffer. Recovery
        # retains the entire retired helper directory if Python cannot copy it.
        if active_wpr and host:
            try:atomic_json(active_path/'wpr-stop.json',host.request('stop',instance=active_wpr['instance']))
            except Exception as e:atomic_json(active_path/'wpr-error.json',{'error':str(e)})
        if pm:
            try:pm.stop()
            except Exception as e:atomic_json(directory/'presentmon-error.json',{'error':str(e)})
        status.close()

def main():
    p=argparse.ArgumentParser(description='X-Plane performance capture and analysis')
    sub=p.add_subparsers(dest='command',required=True)
    for name in ['validate','preflight','emit-flight']:
        s=sub.add_parser(name);s.add_argument('--config',type=Path,required=True)
        if name=='preflight':s.add_argument('--root',type=Path,required=True)
        if name=='emit-flight':s.add_argument('--output',type=Path,required=True)
    for name in ['worker','report','mark']:
        s=sub.add_parser(name);s.add_argument('--run',type=Path,required=True)
        if name=='worker':s.add_argument('--port',type=int,default=8153)
        if name=='mark':s.add_argument('--note',default='felt stutter')
    args=p.parse_args()
    if args.command=='validate':print(json.dumps(resolve(args.config)))
    elif args.command=='emit-flight':atomic_json(args.output,flight_request(resolve(args.config))['data'])
    elif args.command=='preflight':print(json.dumps(preflight(resolve(args.config),args.root)))
    elif args.command=='worker':worker(args.run,args.port)
    elif args.command=='report':
        from .perf_report import build
        result=build(args.run);print(json.dumps(result))
        if not result['complete']:raise SystemExit(1)
    elif args.command=='mark':
        atomic_json(args.run/f'mark-{uuid.uuid4().hex}.request',{'requested_qpc':qpc(),'epoch':time.time(),'note':args.note})
        print('Stutter marker queued. Native recording must be active to receive it.')

if __name__=='__main__':main()
