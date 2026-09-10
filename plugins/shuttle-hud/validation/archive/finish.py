from pathlib import Path
import json,hashlib,shutil,datetime,subprocess,re
R=Path(__file__).parent;ROOT=R.parents[1];SUP=ROOT/'Support/Shuttle-HUD';DST=ROOT/'Aircraft/OrgForum/Space Shuttle F-SIM HUD'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
proc=json.loads((R/'process.json').read_text());pid=int(proc['pid'])
result=subprocess.run(['powershell','-NoProfile','-Command',f'Get-Process -Id {pid} -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id'],capture_output=True,text=True)
assert not result.stdout.strip(),'Dedicated simulator is still running'
log=ROOT/'Log.txt';shutil.copy2(log,R/'Log-142-clean-exit.txt');text=log.read_text(errors='replace')
required=['[ShuttleHUD] v142 registered','[ShuttleHUD] cleanup complete; native panel callback released.','[ShuttleHUD] clean stop; presentation settings restored.','Clean exit from threads.','----- X-Plane has shut down -----']
assert all(s in text for s in required)
assert '--=={This application has crashed!}==--' not in text
assert (ROOT/'Custom Scenery/scenery_packs.ini').read_bytes()==(R/'scenery_packs.ini.original').read_bytes()
clean={'process':proc,'normal_quit':True,'forced_termination':False,'required_log_markers':required,'log_sha256':sha(log),'scenery_restored_exactly':True,'warnings':['Missing external XPME library/resources at startup; notice dismissed during inspection.','Missing aircraft VR configuration and OpenXR startup warning.','Third-party scenery texture gamma warnings.'],'completed_utc':datetime.datetime.now(datetime.timezone.utc).isoformat()}
(R/'clean-exit-142.json').write_text(json.dumps(clean,indent=2))
snapshot={'source':{p.name:sha(p) for p in SUP.iterdir() if p.is_file()},'installed':{str(p.relative_to(DST)).replace(chr(92),'/'):sha(p) for p in DST.rglob('*') if p.is_file()},'final_build':{p.name:sha(p) for p in (R/'build-142').iterdir() if p.is_file()}}
(R/'release-manifest-142.json').write_text(json.dumps(snapshot,indent=2))
print('Verified normal simulator exit, exact scenery restoration and final build identity.')
