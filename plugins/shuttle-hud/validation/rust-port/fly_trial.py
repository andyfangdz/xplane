from pathlib import Path
import sys, json, time, math, base64, traceback
ROOT=Path(r'D:\X-Plane 12')
RUN=Path(__file__).parent
sys.path.insert(0,str(ROOT/'Support'/'flight-test-harness'/'vendor'))
from flight_test.api import XPlaneApi, XPlaneConnectionError
from flight_test.geometry import quaternion

def save(name,value):
    p=RUN/name
    p.write_text(json.dumps(value,indent=2),encoding='utf-8')

def prepare(api,trial):
    ready_deadline=time.monotonic()+120
    while time.monotonic()<ready_deadline:
        try:
            api.request('GET','/datarefs?limit=1')
            break
        except XPlaneConnectionError:
            time.sleep(.5)
    else: raise RuntimeError('Simulator Web API did not start')
    # Coordinates from installed KEDW apt.dat, 542 m displaced threshold on 22L.
    un=-0.528204999290666;ue=-0.8491168816625587
    along=-8500
    north=(along+542)*un;east=(along+542)*ue
    latitude=34.91886525+north/111120
    longitude=-117.85736822+east/(111120*math.cos(math.radians(34.9067)))
    height=(-7500*.3048-along)*math.tan(math.radians(20))
    # Seed 300 KEAS approximately at local ISA density; achieved EAS is measured.
    altitude=2310*.3048+height
    sigma=(1-2.25577e-5*altitude)**4.25588
    tas=300*.514444/math.sqrt(sigma)
    payload={'data':{'aircraft':{'path':'Aircraft/Testing/Shuttle Rust Trial/Orbiter_Glider.acf'},
        'lle_air_start':{'latitude':latitude,'longitude':longitude,'elevation_in_meters':altitude,
                        'heading_true':238.115746099,'speed_in_meters_per_second':tas,'pitch_in_degrees':-12},
        'engine_status':{'all_engines':{'running':False}},
        'local_time':{'day_of_year':252,'time_in_24_hours':12.0},
        'weather':{'definition':{'latitude_in_degrees':34.91886525,'longitude_in_degrees':-117.85736822,
            'elevation_in_meters':2310*.3048,'visibility_in_kilometers':50,'temperature_in_degrees_celsius':15,
            'altimeter_setting_in_hpa':1013.25,'precipitation_ratio':0,
            'wind':[{'altitude_in_feet_msl':h,'speed_in_knots':0,'direction_in_degrees_true':0,'turbulence_ratio':0} for h in [0,16000]]},
            'vertical_speed_in_thermal_in_feet_per_minute':0,'wave_height_in_meters':0,'wave_direction_in_degrees':0,
            'terrain_state':'dry','variation_across_region_percentage':0,'evolution_over_time_enum':'static'}}}
    save(f'flight-request-{trial}.json',payload)
    try: api.load_flight(payload)
    except XPlaneConnectionError as e: print('Load connection ended; checking flight state:',e,flush=True)
    deadline=time.monotonic()+240
    next_notice=0
    while time.monotonic()<deadline:
        try:
            api.refresh_catalogs()
            if 'sim/operation/pause_on' in api.commands:
                api.command('sim/operation/pause_on',.2)
                if (int(api.get_scalar('sim/time/paused'))==1 and api.datarefs.get('shuttle_demo/armed',{}).get('is_writable') and abs(api.get_scalar('sim/flightmodel/position/elevation')-altitude)<50 and api.get_scalar('sim/flightmodel/weight/m_total')>80000):
                    break
        except Exception as e:
            if time.monotonic()>next_notice:
                print('Waiting for paused shuttle load:',str(e)[:150],flush=True);next_notice=time.monotonic()+20
        time.sleep(.5)
    else: raise RuntimeError('Could not establish paused shuttle after load')
    print('Shuttle loaded and paused.',flush=True)
    time.sleep(2)
    api.refresh_catalogs()
    api.set_dataref('shuttle_demo/armed',0)
    api.set_dataref('sim/operation/override/override_planepath',[1]+[0]*19)
    api.set_dataref('sim/flightmodel/position/q',quaternion(238.115746099,-12,0))
    gamma=math.radians(-20);heading=math.radians(238.115746099)
    for name,value in {'local_vx':tas*math.cos(gamma)*math.sin(heading),'local_vy':tas*math.sin(gamma),
                       'local_vz':-tas*math.cos(gamma)*math.cos(heading),'P':0,'Q':0,'R':0}.items():
        api.set_dataref('sim/flightmodel/position/'+name,value)
    settings={'sim/cockpit/autopilot/autopilot_mode':0,'sim/cockpit2/controls/gear_handle_down':0,
              'sim/aircraft/parts/acf_gear_deploy':[0]*10,'sim/cockpit2/controls/speedbrake_ratio':0,
              'sim/cockpit2/controls/parking_brake_ratio':0,'sim/flightmodel/controls/elv_trim':0,
              'sim/flightmodel/weight/m_fixed':0,'sim/cockpit2/engine/actuators/throttle_ratio_all':0,
              'sim/cockpit/switches/parachute_on':0,'sim/cockpit/switches/puffers_on':0,
              'sim/operation/override/override_joystick':1,
              'sim/cockpit2/electrical/battery_on':[1]*8,'sim/cockpit2/switches/avionics_power_on':1}
    if '--mass-lb' in sys.argv:
        target_lb=float(sys.argv[sys.argv.index('--mass-lb')+1]);assert 184000<=target_lb<=226040
        # This port has no payload stations and ignores m_fixed writes.
        # Weight-comparison probe changes native mass at the existing CG while
        # paused; it does not purport to reproduce the mission's CG/inertia.
        settings['sim/aircraft/weight/acf_m_empty']=target_lb*.45359237
    applied={}
    for name,value in settings.items():
        if name not in api.datarefs: continue
        if isinstance(value,list):
            value=[value[0]]*len(api.get_raw(name))
        api.set_dataref(name,value);applied[name]=api.get_raw(name)
        if name=='sim/aircraft/weight/acf_m_empty':assert abs(applied[name]-value)<.1, 'Native mass setting rejected'
    api.command('fsim_hud/view',.2)
    # A stale track during paused loading must never select the reciprocal runway.
    assert api.get_scalar('fsim_hud/runway_index')==1, 'Expected Edwards 22L guidance'
    api.set_dataref('sim/graphics/view/field_of_view_deg',65)
    api.set_dataref('sim/cockpit2/switches/HUD_on',1)
    api.set_dataref('sim/cockpit2/switches/HUD_brightness_ratio',1)
    api.command('sim/operation/screenshot',.2)
    names=['sim/time/paused','sim/flightmodel/position/elevation','sim/flightmodel/position/y_agl',
           'sim/flightmodel/position/theta','sim/flightmodel/position/phi','sim/flightmodel/weight/m_total',
           'sim/flightmodel/position/local_vx','sim/flightmodel/position/local_vy','sim/flightmodel/position/local_vz',
           'sim/aircraft/view/acf_relative_path','sim/graphics/VR/enabled',
           'fsim_hud/runway_index','fsim_hud/vertical_velocity_mps','fsim_hud/groundspeed_mps']
    values=api.get_batch([n for n in names if n in api.datarefs])
    save(f'setup-{trial}.json',{'readback':values,'applied':applied,'tas_seed':tas,'initial_height_m':height})
    assert values['sim/time/paused']==1 and abs(values['sim/flightmodel/position/elevation']-altitude)<2, 'Flight placement changed during initialization'
    assert abs(values['sim/flightmodel/position/phi'])<1 and values['fsim_hud/runway_index']==1
    api.set_dataref('shuttle_demo/trial',trial)
    api.set_dataref('shuttle_demo/heartbeat',int(time.monotonic()))
    api.set_dataref('sim/operation/override/override_planepath',[0]*20)
    if api.get_raw('sim/operation/override/override_planepath')[0]!=0: raise RuntimeError('Path override failed to release')
    print('Setup verified:',values,flush=True)

def fly(api,trial):
    api.set_dataref('shuttle_demo/armed',1)
    api.set_dataref('shuttle_demo/heartbeat',int(time.monotonic()))
    api.command('sim/operation/pause_off',.2)
    start=time.monotonic();next_notice=0
    samples=[]
    names=['shuttle_demo/phase','shuttle_demo/along_m','shuttle_demo/cross_m',
           'sim/flightmodel/position/indicated_airspeed','sim/flightmodel/position/y_agl',
           'sim/flightmodel/position/theta','sim/flightmodel/position/phi','sim/flightmodel/position/local_vy',
           'sim/time/paused','sim/flightmodel/weight/m_total']
    while time.monotonic()-start<600:
        api.set_dataref('shuttle_demo/heartbeat',int(time.monotonic()))
        state=api.get_batch(names);state['wall_elapsed']=time.monotonic()-start
        samples.append(state)
        save('live-status.json',state)
        phase=state['shuttle_demo/phase']
        target_lb=float(sys.argv[sys.argv.index('--mass-lb')+1]) if '--mass-lb' in sys.argv else 184000
        if state['wall_elapsed']>3 and phase>0 and abs(state['sim/flightmodel/weight/m_total']/.45359237-target_lb)>2:
            api.command('sim/operation/pause_on',.2);api.set_dataref('shuttle_demo/armed',0);api.set_dataref('sim/operation/override/override_joystick',0)
            raise RuntimeError('Flight mass differs from requested test condition')
        if time.monotonic()>next_notice:
            print('FLIGHT',json.dumps(state),flush=True);next_notice=time.monotonic()+10
        if phase==5 or phase<0:
            break
        time.sleep(.4)
    else: raise RuntimeError('Flight supervision deadline exceeded')
    api.command('sim/operation/pause_on',.2)
    touchdown=api.get_batch(['shuttle_demo/touchdown_vy','shuttle_demo/touchdown_ias','shuttle_demo/touchdown_eas','shuttle_demo/touchdown_along','shuttle_demo/touchdown_cross',
                            'shuttle_demo/phase','shuttle_demo/armed','sim/operation/override/override_joystick',
                            'sim/operation/override/override_planepath','sim/flightmodel/failures/over_g','sim/time/paused'])
    save(f'supervision-{trial}.json',samples)
    save(f'result-{trial}.json',touchdown)
    api.command('sim/operation/screenshot',.2)
    print('RESULT',json.dumps(touchdown),flush=True)

if __name__=='__main__':
    trial=int(sys.argv[1]) if len(sys.argv)>1 else 1
    with XPlaneApi(port=8144,timeout=5) as api:
        try:
            prepare(api,trial)
            if '--prepare-only' not in sys.argv: fly(api,trial)
        except Exception:
            detail=traceback.format_exc();print(detail,flush=True);(RUN/f'error-{trial}.txt').write_text(detail)
            try:
                api.refresh_catalogs();api.command('sim/operation/pause_on',.2)
                if 'shuttle_demo/armed' in api.datarefs:api.set_dataref('shuttle_demo/armed',0)
            except Exception: pass
            raise
