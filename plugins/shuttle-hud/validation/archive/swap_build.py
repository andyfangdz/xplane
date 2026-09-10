from pathlib import Path
import sys,json,time,base64,shutil
R=Path(__file__).parent;ROOT=R.parents[1];sys.path.insert(0,str(R));from cards import XPlaneApi,XPlaneConnectionError,config
version=int(sys.argv[1]);build=R/f'build-{version}';dst=ROOT/'Aircraft/Testing/Shuttle Symbology Trial/plugins/ShuttleHUD/64/win.xpl'
with XPlaneApi(port=8144,timeout=8) as a:
 config('disable')
 try:
  a.refresh_catalogs();a.command('sim/operation/pause_on',.2)
  if 'shuttle_demo/armed' in a.datarefs:
   try:a.set_dataref('shuttle_demo/armed',0)
   except Exception:pass
  a.set_dataref('sim/operation/override/override_planepath',[0]*20);a.set_dataref('sim/operation/override/override_joystick',0)
  p=json.loads((ROOT/'Output/shuttle-hud-20260909/release-flight-request.json').read_text())
  try:a.load_flight(p)
  except XPlaneConnectionError:pass
  end=time.monotonic()+90
  while time.monotonic()<end:
   try:
    a.refresh_catalogs();a.command('sim/operation/pause_on',.2)
    path=base64.b64decode(a.get_raw('sim/aircraft/view/acf_relative_path')).decode().rstrip('\0')
    if 'OrgForum/Space Shuttle F-SIM HUD/' in path and a.get_scalar('fsim_hud/version')==136:break
   except XPlaneConnectionError:pass
   time.sleep(.3)
  else:raise RuntimeError('Trial did not unload')
  shutil.copy2(build/'win.xpl',dst)
  for name in ['shuttle_hud.cpp','hud_math.hpp','hud_presentation.hpp','landing_guidance.hpp','test_presentation.cpp','ValidationController.lua','hud-optics.txt','build.ps1']:
   shutil.copy2(ROOT/'Support/Shuttle-HUD'/name,build/name)
  print('Unloaded trial and staged build',version,flush=True)
 finally:config('restore')
