from pathlib import Path
import sys,json,hashlib,shutil,time,base64
R=Path(__file__).parent;sys.path.insert(0,str(R));from cards import XPlaneApi,init,snap
repo=Path('V:/src/xplane');live=R.parents[1]/'Aircraft/OrgForum/Space Shuttle F-SIM HUD'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
manifest=json.loads((R/'build-143/hashes.json').read_text())
for name,digest in manifest.items():
 p=R/'build-143/win.xpl' if name=='binary' else repo/name
 assert sha(p)==digest,name
assert sha(repo/'target/release/shuttle_hud.dll')==manifest['binary']
binary=live/'plugins/ShuttleHUD/64/win.xpl'
assert sha(binary)==sha(R/'baseline-142/plugins/ShuttleHUD/64/win.xpl')
with XPlaneApi(port=8144,timeout=5) as a:
 a.refresh_catalogs()
 actual=base64.b64decode(a.get_raw('sim/aircraft/view/acf_relative_path')).decode().rstrip('\0')
 assert actual=='Aircraft/Testing/Shuttle Rust Trial/Orbiter_Glider.acf',actual
 shutil.copy2(R/'build-143/win.xpl',binary)
 out={'sha256':sha(binary),'before_sha256':sha(R/'baseline-142/plugins/ShuttleHUD/64/win.xpl')}
 out['installed']=init(a,'release-installed-143',height=7000,along=-8150,aircraft_path='Aircraft/OrgForum/Space Shuttle F-SIM HUD/Orbiter_Glider.acf')
 assert out['installed']['fsim_hud/version']==143 and out['installed']['fsim_hud/rust_implementation']==1
 a.command('fsim_hud/fullscreen',.2);time.sleep(.3)
 out['fullscreen']=snap(a,'release-fullscreen-143','Installed Rust 143 full-screen view')
 a.command('sim/view/forward_with_hud',.2);time.sleep(.3)
 out['shift_w']=snap(a,'release-shift-w-143','Installed Rust 143 stock Shift+W command interception')
 assert out['shift_w']['fsim_hud/cockpit_active']==1
 out['final']=a.get_batch(['sim/operation/override/override_joystick','sim/operation/override/override_planepath','sim/time/paused','sim/aircraft/view/acf_relative_path','sim/aircraft/specialcontrols/acf_chute_area'])
 assert out['final']['sim/operation/override/override_joystick']==0
 assert out['final']['sim/operation/override/override_planepath']==[0]*20
 assert not (live/'plugins/xlua/scripts/Final_Demo').exists()
 out['passed']=True
 (R/'release-installed-143.json').write_text(json.dumps(out,indent=2))
 print('Installed Rust 143 and verified aircraft load, fullscreen and Shift+W',flush=True)
