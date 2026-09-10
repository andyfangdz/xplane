from pathlib import Path
import sys,time,json
R=Path(__file__).parent;sys.path.insert(0,str(R));from cards import XPlaneApi,init,snap,advance
with XPlaneApi(port=8144,timeout=8) as a:
 a.refresh_catalogs();out=[]
 try:
  init(a,'release-142-alert-base',height=150,along=-700,pitch=20,gamma=-1.5,eas=250,gear=0,aircraft_path='Aircraft/OrgForum/Space Shuttle F-SIM HUD/Orbiter_Glider.acf')
  a.set_dataref('fsim_hud/declutter_mode',2);a.set_dataref('sim/cockpit2/controls/speedbrake_ratio',1)
  a.set_dataref('sim/operation/override/override_planepath',[1]+[0]*19)
  for k in range(6):
   advance(a,.27)
   s=snap(a,f'release-142-flash-{k}','initialized alert card; actual simulation time advanced between paused captures')
   assert s['fsim_hud/gear_cue']==1 and s['fsim_hud/velocity_limited']==1
   out.append({'state':s,'flash_clock':a.get_scalar('sim/time/total_running_time_sec'),'speedbrake_actual':a.get_scalar('sim/flightmodel2/controls/speedbrake_ratio')})
 finally:
  a.command('sim/operation/pause_on',.2);a.set_dataref('sim/operation/override/override_planepath',[0]*20);a.set_dataref('sim/operation/override/override_joystick',0)
  (R/'release-flash-142.json').write_text(json.dumps(out,indent=2))
