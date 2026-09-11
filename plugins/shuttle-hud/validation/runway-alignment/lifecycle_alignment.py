from pathlib import Path
import sys,time,json,hashlib
sys.path.insert(0,str(Path(__file__).parent))
from capture_alignment import XPlaneApi,init,snap,advance,R

with XPlaneApi(port=8144,timeout=8) as a:
    a.refresh_catalogs()
    assert a.get_scalar('fsim_hud/version')==144
    a.command('fsim_hud/view',.2)
    a.set_dataref('sim/graphics/view/field_of_view_deg',65)
    a.set_dataref('sim/graphics/view/field_of_view_vertical_ratio',0)
    a.set_dataref('fsim_hud/declutter_mode',0)
    time.sleep(.3)
    for n,v in [('sim/graphics/view/pilots_head_psi',7),('sim/graphics/view/pilots_head_the',-2)]:a.set_dataref(n,v)
    time.sleep(.3);snap(a,'accepted-144-offaxis-cockpit')
    a.command('fsim_hud/view',.2)
    a.set_dataref('sim/graphics/view/field_of_view_vertical_ratio',0)
    result={'version':144,'checks':{}}
    for level in [0,1,2,3,0]:
        a.set_dataref('fsim_hud/declutter_mode',level)
        time.sleep(.2)
        snap(a,f'accepted-144-declutter-{level}')
        assert a.get_scalar('fsim_hud/declutter_level')==level
    result['checks']['declutter_readback']=True
    for name in ['fsim_hud/enabled','sim/cockpit2/switches/HUD_on']:
        a.set_dataref(name,0);time.sleep(.2)
        assert a.get_scalar('fsim_hud/runway_projection_valid')==0
        assert a.get_scalar('fsim_hud/active')==0
        a.set_dataref(name,1);time.sleep(.2)
        assert a.get_scalar('fsim_hud/runway_projection_valid')==1
        assert a.get_scalar('fsim_hud/active')==1
        result['checks'][name+'_off_on']=True
    before=a.get_batch(['sim/graphics/view/field_of_view_deg','sim/graphics/view/field_of_view_vertical_ratio'])
    a.command('fsim_hud/fullscreen',.2);time.sleep(.2)
    a.command('fsim_hud/view',.2);time.sleep(.2)
    after=a.get_batch(before)
    assert before==after,(before,after)
    result['checks']['view_restored']=True
    result['view_before']=before;result['view_after']=after
    # A short native glide verifies that the conformal outline updates in motion.
    result['motion']=[]
    for i in range(3):
        advance(a,1.0)
        snap(a,f'accepted-144-moving-{i}')
        state=a.get_batch(['sim/flightmodel/position/latitude','sim/flightmodel/position/longitude','sim/flightmodel/position/elevation','fsim_hud/runway_projection_valid','sim/operation/override/override_planepath','sim/operation/override/override_joystick'])
        assert state['fsim_hud/runway_projection_valid']==1
        assert not any(state['sim/operation/override/override_planepath'])
        assert state['sim/operation/override/override_joystick']==0
        result['motion'].append(state)
    result['checks']['native_glide']=True
    result['passed']=all(result['checks'].values())
    (R/'lifecycle-144.json').write_text(json.dumps(result,indent=2))
    print(json.dumps(result['checks']),flush=True)
