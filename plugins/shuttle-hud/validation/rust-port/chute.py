from pathlib import Path
import sys,time,json
R=Path(__file__).parent;sys.path.insert(0,str(R));from cards import XPlaneApi
with XPlaneApi(port=8144,timeout=8) as a:
 a.refresh_catalogs();assert a.get_scalar('shuttle_demo/phase')==5
 a.set_dataref('shuttle_demo/armed',0);a.set_dataref('sim/cockpit2/controls/parking_brake_ratio',1)
 names=['sim/time/total_flight_time_sec','sim/time/paused','sim/time/is_in_replay','sim/aircraft/specialcontrols/acf_chute_area','sim/cockpit/switches/parachute_on','fsim_hud/chute_area_ratio','fsim_hud/chute_reefing_active','fsim_hud/enabled','fsim_hud/landing_systems_enabled']
 out={}
 def record(key):
  v=a.get_batch(names);out[key]=v;return v
 def wait_until(t):
  end=time.monotonic()+12
  while time.monotonic()<end:
   if a.get_scalar(names[0])>=t:return
   time.sleep(.025)
  raise RuntimeError('chute card simulation clock stopped')
 try:
  a.set_dataref('sim/cockpit/switches/parachute_on',0);a.set_dataref('fsim_hud/landing_systems_enabled',1)
  a.command('sim/operation/pause_off',.2);time.sleep(.25)
  full=a.get_scalar(names[3]);assert full>0
  a.set_dataref('sim/cockpit/switches/parachute_on',1);start=a.get_scalar(names[0]);wait_until(start+.4)
  s=record('early');assert 0<s['fsim_hud/chute_area_ratio']<.16
  a.set_dataref('fsim_hud/enabled',0);time.sleep(.1);s=record('hud_hidden');assert s['fsim_hud/chute_reefing_active']==1
  a.set_dataref('fsim_hud/enabled',1);wait_until(start+2.0);s=record('plateau');assert abs(s['fsim_hud/chute_area_ratio']-.16)<1e-5
  assert abs(s[names[3]]-full*.16)<.001
  a.set_dataref('fsim_hud/landing_systems_enabled',0);time.sleep(.12);s=record('systems_disabled');assert abs(s[names[3]]-full)<.001
  a.set_dataref('fsim_hud/landing_systems_enabled',1);time.sleep(.12);s=record('systems_restored');assert abs(s['fsim_hud/chute_area_ratio']-.16)<1e-5
  a.command('sim/operation/pause_on',.2);time.sleep(.12);s=record('paused');assert s[names[1]]==1 and abs(s[names[3]]-full)<.001
  a.command('sim/replay/replay_toggle',.2);time.sleep(.3);s=record('replay');assert s[names[2]]==1 and abs(s[names[3]]-full)<.001
  a.command('sim/replay/replay_toggle',.2);a.command('sim/operation/pause_off',.2);wait_until(start+6.0)
  s=record('open');assert s['fsim_hud/chute_area_ratio']==1 and abs(s[names[3]]-full)<.001
  a.set_dataref('sim/cockpit/switches/parachute_on',0);time.sleep(.15);s=record('jettison');assert s['fsim_hud/chute_reefing_active']==0 and abs(s[names[3]]-full)<.001
  out['passed']=True;print('Native chute stages and ownership checks passed.',flush=True)
 finally:
  a.command('sim/operation/pause_on',.2);a.set_dataref('sim/cockpit/switches/parachute_on',0);a.set_dataref('fsim_hud/enabled',1);a.set_dataref('fsim_hud/landing_systems_enabled',1)
  (R/'chute-143.json').write_text(json.dumps(out,indent=2))
