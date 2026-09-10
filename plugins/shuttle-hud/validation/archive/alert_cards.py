from pathlib import Path
import sys,time,json
R=Path(__file__).parent;sys.path.insert(0,str(R));from cards import XPlaneApi,init,snap,advance
with XPlaneApi(port=8144,timeout=8) as a:
 a.refresh_catalogs();out=[]
 try:
  init(a,'verified-142-alert-base',height=150,along=-700,pitch=20,gamma=-1.5,eas=250,gear=0)
  a.set_dataref('fsim_hud/declutter_mode',2)
  a.set_dataref('sim/cockpit2/controls/speedbrake_ratio',1)
  a.set_dataref('sim/operation/override/override_planepath',[1]+[0]*19);advance(a,.2)
  assert a.get_scalar('fsim_hud/velocity_limited')==1
  for k in range(8):
   s=snap(a,f'verified-142-alert-{k}');assert s['fsim_hud/gear_cue']==1;out.append(s);time.sleep(.11)
 finally:
  a.command('sim/operation/pause_on',.2);a.set_dataref('sim/operation/override/override_planepath',[0]*20)
  (R/'alert-cards-142.json').write_text(json.dumps(out,indent=2))
