from pathlib import Path
import sys,json,time,base64,shutil,hashlib
R=Path(__file__).parent;ROOT=R.parents[1];sys.path.insert(0,str(R));from cards import XPlaneApi,XPlaneConnectionError,config,advance
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
dst=ROOT/'Aircraft/OrgForum/Space Shuttle F-SIM HUD';sup=ROOT/'Support/Shuttle-HUD';build=R/'build-142'
assert json.loads((R/'analysis-206.json').read_text())['accepted']
assert all(sha(sup/n)==sha(R/'baseline-136/support'/n) for n in ['landing_guidance.hpp','ValidationController.lua','hud-optics.txt'])
with XPlaneApi(port=8144,timeout=8) as a:
 a.refresh_catalogs();a.command('sim/operation/pause_on',.2)
 path=base64.b64decode(a.get_raw('sim/aircraft/view/acf_relative_path')).decode().rstrip('\0')
 assert 'Aircraft/Testing/Shuttle Symbology Trial/' in path
 a.set_dataref('shuttle_demo/armed',0);a.set_dataref('sim/operation/override/override_planepath',[0]*20);a.set_dataref('sim/operation/override/override_joystick',0)
 old=ROOT/'Output/shuttle-hud-20260909';backup=R/'baseline-136/build-runtime';backup.mkdir(parents=True,exist_ok=True)
 for n in ['win.xpl','ShuttleHUD.dll','build-hashes.json']:
  if not (backup/n).exists():shutil.copy2(old/n,backup/n)
  shutil.copy2(build/n,old/n)
 shutil.copy2(build/'win.xpl',dst/'plugins/ShuttleHUD/64/win.xpl')
 for source,target in [('README.md','HUD_README.md'),('LIFECYCLE.md','HUD_LIFECYCLE.md'),('ACCEPTANCE.md','HUD_ACCEPTANCE.md')]:shutil.copy2(sup/source,dst/target)
 (dst/'START_HERE.md').write_text('# Space Shuttle — native HUD, release 142\n\nSelect **Space Shuttle - F-SIM HUD**, then press **Shift+W**.\n\nRelease 142 refines HUD symbols and their behavior from approach through rollout. Read [HUD_README.md](HUD_README.md) for controls and limits. The native phase gallery, manual comparisons and full test ledger are in `D:/X-Plane 12/Output/shuttle-symbology-20260910/REPORT.html`.\n\nThe aircraft geometry, landing calibration and ball/bar scenery remain at release 136. No test pilot is installed here. The untouched original remains in `Aircraft/OrgForum/Space Shuttle-FX-V12`.\n',encoding='utf-8')
 config('disable')
 try:
  p=json.loads((ROOT/'Output/shuttle-hud-20260909/release-flight-request.json').read_text())
  try:a.load_flight(p)
  except XPlaneConnectionError:pass
  end=time.monotonic()+100
  while time.monotonic()<end:
   try:
    a.refresh_catalogs();a.command('sim/operation/pause_on',.2)
    path=base64.b64decode(a.get_raw('sim/aircraft/view/acf_relative_path')).decode().rstrip('\0')
    if path=='Aircraft/OrgForum/Space Shuttle F-SIM HUD/Orbiter_Glider.acf' and a.get_scalar('fsim_hud/version')==142:break
   except XPlaneConnectionError:pass
   time.sleep(.3)
  else:raise RuntimeError('Installed release failed to load')
  advance(a,.8)
  a.command('fsim_hud/view',.2);a.set_dataref('sim/graphics/view/field_of_view_deg',65)
  a.set_dataref('sim/cockpit2/electrical/battery_on',[1]*len(a.get_raw('sim/cockpit2/electrical/battery_on')))
  a.set_dataref('sim/cockpit2/switches/HUD_on',1);a.set_dataref('sim/cockpit2/switches/HUD_brightness_ratio',1)
  print('Installed release 142 loaded and paused.',flush=True)
 finally:config('restore')

# Preserve the previous verifier's measured checks without its automatic quit.
source=(ROOT/'Output/shuttle-flare-20260910/verify_release_exit.py').read_text().replace('136','142')
(R/'verify_release.py').write_text(source[:source.index(' process=json.loads')])
