from pathlib import Path
import sys,json,time,base64,datetime,subprocess
R=Path(__file__).parent
sys.path.insert(0,str(R))
import fly_trial as f
from flight_test.api import XPlaneApiError
def utc():return datetime.datetime.now(datetime.timezone.utc).isoformat()
with f.XPlaneApi(port=8144,timeout=10) as a:
 a.refresh_catalogs()
 raw=a.get_raw('sim/aircraft/view/acf_relative_path')
 aircraft=base64.b64decode(raw).decode().rstrip('\x00')
 assert aircraft.replace('\\','/')=='Aircraft/OrgForum/Space Shuttle F-SIM HUD/Orbiter_Glider.acf',aircraft
 names=['fsim_hud/version','fsim_hud/aircraft_match','fsim_hud/renderer','sim/time/paused','sim/operation/override/override_planepath','sim/operation/override/override_joystick']
 snapshot=a.get_batch(names)
 assert snapshot[names[0]]==142 and snapshot[names[1]]==1 and snapshot[names[2]]==2 and snapshot[names[3]]==1
 assert not any(snapshot[names[4]]) and snapshot[names[5]]==0
 released={}
 for name in ['shuttle_demo/armed','shuttle_scenery_probe/camera_active']:
  if name not in a.datarefs:released[name]='absent from catalog'
  else:
   try:a.get_raw(name)
   except XPlaneApiError as e:
    assert '404' in str(e) and 'invalid_dataref_id' in str(e),str(e)
    released[name]='unregistered: stale catalog ID returns HTTP 404 invalid_dataref_id'
   else:raise AssertionError('Validation helper still active: '+name)
 view=['sim/graphics/view/field_of_view_deg','sim/graphics/view/field_of_view_vertical_ratio']
 a.command('fsim_hud/view',.2);time.sleep(.5);before=a.get_batch(view)
 a.command('fsim_hud/fullscreen',.2);time.sleep(.5);during=a.get_batch(view)
 a.command('fsim_hud/view',.2);time.sleep(.5);after=a.get_batch(view)
 assert before==after and abs(during[view[1]]+.5)<1e-5
 result={'utc':utc(),'aircraft':aircraft,'snapshot':snapshot,'validation_helpers':released,'view_before':before,'fullscreen':during,'view_restored':after,'passed':True}
 (R/'release-reload-142.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
 print(json.dumps(result,indent=2),flush=True)
