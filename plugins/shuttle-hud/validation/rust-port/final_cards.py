from pathlib import Path
import sys,time,json
R=Path(__file__).parent;sys.path.insert(0,str(R));from cards import XPlaneApi,init,snap,advance
with XPlaneApi(port=8144,timeout=8) as a:
 a.refresh_catalogs();out={}
 try:
  assert a.get_scalar('shuttle_demo/phase')==5
  assert a.get_scalar('fsim_hud/display_nose_wow')==1
  cycle=[]
  for _ in range(3):
   a.command('fsim_hud/declutter_cycle',.2);time.sleep(.2)
   s=snap(a,'verified-143-ground-cycle-'+str(len(cycle)+1),'observed stopped state; manual declutter command')
   cycle.append(s['fsim_hud/declutter_level'])
   if cycle[-1]==3:assert s['fsim_hud/display_flags']==1
  assert cycle==[1,3,0],cycle;out['ground_cycle']=cycle
  init(a,'verified-143-fade-fixed',height=18100,along=-18000,pitch=-5,gamma=-10)
  assert a.get_scalar('fsim_hud/velocity_vector_blend')==0
  a.set_dataref('sim/operation/override/override_planepath',[1]+[0]*19)
  a.set_dataref('sim/flightmodel/position/local_y',a.get_scalar('sim/flightmodel/position/local_y')-200*.3048)
  a.set_dataref('sim/operation/override/override_planepath',[0]*20);advance(a,.15)
  s=snap(a,'verified-143-fade-start');assert s['fsim_hud/display_phase']==2 and s['fsim_hud/velocity_vector_blend']<.2
  out['fade_start']=s
  a.set_dataref('sim/operation/override/override_planepath',[1]+[0]*19)
  advance(a,2.4);out['fade_mid']=snap(a,'verified-143-fade-mid')
  advance(a,2.8);s=snap(a,'verified-143-fade-complete');assert s['fsim_hud/velocity_vector_blend']==1;out['fade_end']=s
  a.set_dataref('sim/operation/override/override_planepath',[0]*20)
  # A real replay transition must reset presentation time/phase safely.
  replay=[n for n in a.commands if 'replay' in n];out['replay_commands']=replay
  a.command('sim/replay/replay_toggle',.2);time.sleep(.4)
  assert a.get_scalar('sim/time/is_in_replay')==1
  out['replay']=snap(a,'verified-143-replay')
  a.command('sim/replay/replay_toggle',.2);a.command('sim/operation/pause_on',.2);time.sleep(.4)
  assert a.get_scalar('sim/time/is_in_replay')==0
  out['replay_exit']=snap(a,'verified-143-replay-exit')
  init(a,'verified-143-optical-center',height=3300,along=-5050,pitch=-10)
  refs=['sim/graphics/view/field_of_view_deg','sim/graphics/view/field_of_view_vertical_ratio']
  out['view_before']=a.get_batch(refs)
  a.command('fsim_hud/fullscreen',.2);time.sleep(.3);out['fullscreen']=snap(a,'verified-143-fullscreen')
  a.set_dataref('sim/cockpit2/switches/HUD_on',0);time.sleep(.3);out['fullscreen_poweroff']=snap(a,'verified-143-fullscreen-poweroff')
  a.set_dataref('sim/cockpit2/switches/HUD_on',1);a.command('fsim_hud/view',.2);time.sleep(.3)
  out['view_after']=a.get_batch(refs);assert out['view_before']==out['view_after'],out
  a.set_dataref('fsim_hud/brightness',.2);time.sleep(.3);out['dim']=snap(a,'verified-143-native-dim')
  a.set_dataref('fsim_hud/brightness',1);a.set_dataref('sim/cockpit2/switches/HUD_on',0);time.sleep(.3)
  s=snap(a,'verified-143-native-poweroff');assert s['fsim_hud/active']==0;out['native_poweroff']=s
  a.set_dataref('sim/cockpit2/switches/HUD_on',1)
  head='sim/graphics/view/pilots_head_x';x=a.get_scalar(head)
  a.set_dataref(head,x+.06);time.sleep(.3);out['offaxis']=snap(a,'verified-143-native-offaxis');a.set_dataref(head,x)
  time.sleep(.2);out['restored']=snap(a,'verified-143-optical-restored')
 finally:
  a.command('sim/operation/pause_on',.2);a.set_dataref('sim/operation/override/override_planepath',[0]*20);a.set_dataref('sim/operation/override/override_joystick',0)
  a.set_dataref('sim/cockpit2/switches/HUD_on',1);a.set_dataref('fsim_hud/brightness',1)
  (R/'final-cards-143.json').write_text(json.dumps(out,indent=2))
 print('Final native cards passed.',flush=True)
