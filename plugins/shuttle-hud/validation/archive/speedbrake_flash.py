from pathlib import Path
import sys,time,json
R=Path(__file__).parent;sys.path.insert(0,str(R));from cards import XPlaneApi,init,snap,advance
with XPlaneApi(port=8144,timeout=8) as a:
 a.refresh_catalogs();out=[]
 try:
  init(a,'release-142-speedbrake-base',height=6000,along=-8200,pitch=-10,gamma=-20,eas=300,gear=0,aircraft_path='Aircraft/OrgForum/Space Shuttle F-SIM HUD/Orbiter_Glider.acf')
  a.set_dataref('sim/cockpit2/controls/speedbrake_ratio',1);advance(a,2)
  actual=a.get_scalar('sim/flightmodel2/controls/speedbrake_ratio');command=a.get_scalar('fsim_hud/speedbrake_command_ratio')
  assert abs(actual-command)>20/98.6,(actual,command)
  a.set_dataref('sim/operation/override/override_planepath',[1]+[0]*19)
  for k in range(4):
   advance(a,.27);s=snap(a,f'release-142-speedbrake-flash-{k}','initialized native speedbrake discrepancy; simulation time advanced between captures')
   out.append({'state':s,'actual':a.get_scalar('sim/flightmodel2/controls/speedbrake_ratio'),'flash_clock':a.get_scalar('sim/time/total_running_time_sec')})
 finally:
  a.command('sim/operation/pause_on',.2);a.set_dataref('sim/operation/override/override_planepath',[0]*20);a.set_dataref('sim/operation/override/override_joystick',0)
  (R/'speedbrake-flash-142.json').write_text(json.dumps(out,indent=2))
