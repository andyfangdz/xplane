from pathlib import Path
import sys,time,json
R=Path(__file__).parent;sys.path.insert(0,str(R));from cards import XPlaneApi,snap
phase=sys.argv[1];p=R/'sdk-lifecycle-143.json';out=json.loads(p.read_text()) if p.exists() else {'method':'Actual X-Plane Plugin Admin checkbox through native UI','states':{}}
refs=['sim/graphics/view/field_of_view_deg','sim/graphics/view/field_of_view_vertical_ratio','sim/aircraft/specialcontrols/acf_chute_area']
with XPlaneApi(port=8144,timeout=5) as a:
 a.refresh_catalogs();a.command('sim/operation/pause_on',.2)
 if phase=='before':
  a.set_dataref('fsim_hud/enabled',1);a.set_dataref('sim/cockpit2/switches/HUD_on',1);a.set_dataref('sim/cockpit2/switches/HUD_brightness_ratio',1)
  a.command('fsim_hud/view',.2);time.sleep(.2);out['view_before']=a.get_batch(refs)
  a.command('fsim_hud/fullscreen',.2);time.sleep(.3);out['fullscreen_before']=a.get_batch(refs)
  s=snap(a,'sdk-143-before','Rust trial before actual SDK disable');assert s['fsim_hud/plugin_enabled']==1 and s['fsim_hud/active']==1
  a.command('sim/developer/toggle_plugin_admin',.2)
 elif phase=='disabled':
  s=snap(a,'sdk-143-disabled','Rust trial after actual SDK disable');assert s['fsim_hud/plugin_enabled']==0 and s['fsim_hud/active']==0 and s['fsim_hud/cockpit_active']==0
  out['view_disabled']=a.get_batch(refs);assert out['view_disabled']==out['view_before']
 elif phase=='enabled':
  a.command('fsim_hud/view',.2);time.sleep(.3);s=snap(a,'sdk-143-enabled','Rust trial after actual SDK re-enable');assert s['fsim_hud/plugin_enabled']==1 and s['fsim_hud/active']==1 and s['fsim_hud/cockpit_active']==1
  out['passed']=True
 else:raise ValueError(phase)
 out['states'][phase]=s;p.write_text(json.dumps(out,indent=2));print(phase,'verified',flush=True)
