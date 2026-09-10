from pathlib import Path
import sys,time,json
R=Path(__file__).parent;sys.path.insert(0,str(R));from cards import XPlaneApi,init,snap,advance
with XPlaneApi(port=8144,timeout=8) as a:
 a.refresh_catalogs();init(a,'verified-140-css-digital-base',height=20,along=300,pitch=11,gamma=-.5,eas=240,gear=1)
 results={}
 try:
  for mode in [0,1,2,3]:
   a.set_dataref('fsim_hud/declutter_mode',mode);time.sleep(.2)
   s=snap(a,f'verified-140-air-declutter-{mode}');assert s['fsim_hud/declutter_level']==mode
   if mode==3:assert s['fsim_hud/display_flags']==1
   results['declutter'+str(mode)]=s
  a.set_dataref('fsim_hud/declutter_mode',0);cycle=[]
  for _ in range(4):a.command('fsim_hud/declutter_cycle',.2);time.sleep(.15);cycle.append(a.get_scalar('fsim_hud/declutter_level'))
  assert cycle==[1,2,3,0],cycle;results['manual_cycle']=cycle
  a.set_dataref('fsim_hud/declutter_mode',2);time.sleep(.2)
  a.set_dataref('sim/cockpit/autopilot/autopilot_mode',2);time.sleep(.3)
  s=snap(a,'verified-140-auto-final-flare');assert s['fsim_hud/control_auto']==1 and s['fsim_hud/display_flags']&4;results['auto']=s
  a.set_dataref('sim/cockpit/autopilot/autopilot_mode',0);a.command('fsim_hud/att_ref',.2);time.sleep(.2)
  s=snap(a,'verified-140-att-ref-caged');assert s['fsim_hud/att_ref_caged']==1;results['cage']=s
  a.command('fsim_hud/att_ref',.2)
  a.set_dataref('sim/operation/override/override_planepath',[1]+[0]*19);advance(a,5.2)
  s=snap(a,'verified-140-gear-down-expired');assert s['fsim_hud/gear_cue']==0;results['gear_timer']=s
  # Show the two phases of the real flashing warning while the card is paused.
  a.set_dataref('sim/aircraft/parts/acf_gear_deploy',[0]*len(a.get_raw('sim/aircraft/parts/acf_gear_deploy')))
  a.set_dataref('sim/cockpit2/controls/gear_handle_down',0);advance(a,.1)
  for name,target in [('on',.12),('off',.68)]:
   end=time.monotonic()+3
   while time.monotonic()<end:
    t=a.get_scalar('sim/time/total_running_time_sec')%1
    if abs(t-target)<.06:break
    time.sleep(.025)
   s=snap(a,'verified-140-gear-warning-'+name);assert s['fsim_hud/gear_cue']==1
  a.command('fsim_hud/declutter_auto',.2);time.sleep(.2);assert a.get_scalar('fsim_hud/declutter_level')==2
  # Current state and the pause timer must remain fixed while paused.
  before=a.get_batch(['fsim_hud/velocity_vector_blend','fsim_hud/display_phase','fsim_hud/display_altitude_ft','sim/time/total_flight_time_sec']);time.sleep(.4)
  after=a.get_batch(before);assert before==after;results['paused_steady']=after
 finally:
  a.command('sim/operation/pause_on',.2);a.set_dataref('sim/cockpit/autopilot/autopilot_mode',0)
  a.set_dataref('fsim_hud/att_ref_caged',0);a.set_dataref('sim/operation/override/override_planepath',[0]*20);a.set_dataref('sim/operation/override/override_joystick',0)
  (R/'extended-140.json').write_text(json.dumps(results,indent=2))
 print('Extended native display checks passed.',flush=True)
