from pathlib import Path
import sys,time,json,base64,shutil
R=Path(__file__).parent;sys.path.insert(0,str(R));from cards import XPlaneApi,XPlaneConnectionError,init,snap,config
trial=R.parents[1]/'Aircraft/Testing/Shuttle Rust Trial'
wrong=trial/'Orbiter_Mismatch.acf'
if not wrong.exists():shutil.copy2(trial/'Orbiter_Glider.acf',wrong)
assert wrong.read_bytes()==(trial/'Orbiter_Glider.acf').read_bytes()
out={}
with XPlaneApi(port=8144,timeout=5) as a:
 a.refresh_catalogs()
 config('disable')
 try:
  p=json.loads((R/'verified-143-optical-center-request.json').read_text())
  wanted='Aircraft/Testing/Shuttle Rust Trial/Orbiter_Mismatch.acf'
  p['data']['aircraft']['path']=wanted
  (R/'mismatch-request.json').write_text(json.dumps(p,indent=2))
  try:a.load_flight(p)
  except XPlaneConnectionError:pass
  end=time.monotonic()+100
  while time.monotonic()<end:
   try:
    a.refresh_catalogs();a.command('sim/operation/pause_on',.2)
    actual=base64.b64decode(a.get_raw('sim/aircraft/view/acf_relative_path')).decode().rstrip('\0')
    if actual==wanted and a.get_scalar('fsim_hud/aircraft_match')==0:break
   except XPlaneConnectionError:pass
   time.sleep(.3)
  else:raise RuntimeError('Mismatched ACF load did not settle')
  time.sleep(.3)
  a.set_dataref('shuttle_demo/armed',0)
  a.set_dataref('sim/operation/override/override_planepath',[0]*20)
  a.set_dataref('sim/operation/override/override_joystick',0)
  s=snap(a,'mismatch-143','Same-folder ACF mismatch; plugin loaded and enabled but inactive')
  assert s['fsim_hud/version']==143 and s['fsim_hud/plugin_enabled']==1 and s['fsim_hud/active']==0
  area=a.get_scalar('sim/aircraft/specialcontrols/acf_chute_area')
  assert s['fsim_hud/chute_reefing_active']==0 and abs(area-480*.09290304)<.0001
  out['mismatch']={'aircraft':actual,'state':s,'chute_area':area,'expected_native_area_m2':480*.09290304}
 finally:config('restore')
 out['reload']=init(a,'reload-143',height=7000,along=-8150)
 assert out['reload']['fsim_hud/version']==143 and out['reload']['fsim_hud/rust_implementation']==1
 out['passed']=True
 (R/'reload-mismatch-143.json').write_text(json.dumps(out,indent=2))
 print('Mismatch and correct-aircraft reload passed',flush=True)
