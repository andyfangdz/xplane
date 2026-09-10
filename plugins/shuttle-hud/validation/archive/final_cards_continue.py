from pathlib import Path
import sys,time,json
R=Path(__file__).parent;sys.path.insert(0,str(R));from cards import XPlaneApi,init,snap,advance
with XPlaneApi(port=8144,timeout=8) as a:
 a.refresh_catalogs();out=json.loads((R/'final-cards-140.json').read_text())
 try:
  refs=['sim/graphics/view/field_of_view_deg','sim/graphics/view/field_of_view_vertical_ratio']
  out['view_before']=a.get_batch(refs)
  a.command('fsim_hud/fullscreen',.2);time.sleep(.3);out['fullscreen']=snap(a,'verified-140-fullscreen')
  a.set_dataref('sim/cockpit2/switches/HUD_on',0);time.sleep(.3);out['fullscreen_poweroff']=snap(a,'verified-140-fullscreen-poweroff')
  a.set_dataref('sim/cockpit2/switches/HUD_on',1);a.command('fsim_hud/view',.2);time.sleep(.3)
  out['view_after']=a.get_batch(refs);assert out['view_before']==out['view_after'],out
  a.set_dataref('fsim_hud/brightness',.2);time.sleep(.3);out['dim']=snap(a,'verified-140-native-dim')
  a.set_dataref('fsim_hud/brightness',1);a.set_dataref('sim/cockpit2/switches/HUD_on',0);time.sleep(.3)
  s=snap(a,'verified-140-native-poweroff');assert s['fsim_hud/active']==0;out['native_poweroff']=s
  a.set_dataref('sim/cockpit2/switches/HUD_on',1)
  head='sim/graphics/view/pilots_head_x';x=a.get_scalar(head)
  a.set_dataref(head,x+.06);time.sleep(.3);out['offaxis']=snap(a,'verified-140-native-offaxis');a.set_dataref(head,x)
  time.sleep(.2);out['restored']=snap(a,'verified-140-optical-restored')
 finally:
  a.command('sim/operation/pause_on',.2);a.set_dataref('sim/operation/override/override_planepath',[0]*20);a.set_dataref('sim/operation/override/override_joystick',0)
  a.set_dataref('sim/cockpit2/switches/HUD_on',1);a.set_dataref('fsim_hud/brightness',1)
  (R/'final-cards-140.json').write_text(json.dumps(out,indent=2))
 print('Final native cards passed.',flush=True)
