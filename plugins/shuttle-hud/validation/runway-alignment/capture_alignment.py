from pathlib import Path
import sys, json, time, math, shutil, subprocess
R=Path(__file__).parent
ROOT=R.parents[1]
sys.path.insert(0,str(ROOT/'Support/flight-test-harness/vendor'))
from flight_test.api import XPlaneApi, XPlaneConnectionError
from flight_test.geometry import quaternion
sys.path.insert(0,str(ROOT/'Output/shuttle-hud-20260909/cockpit'))
import check_native as capture
capture.out=R

def advance(a,seconds):
    start=a.get_scalar('sim/time/total_flight_time_sec')
    end=time.monotonic()+25
    a.command('sim/operation/pause_off',.2)
    try:
        while time.monotonic()<end:
            now=a.get_scalar('sim/time/total_flight_time_sec')
            if now<start:start=now
            if now-start>=seconds:return
            if a.get_scalar('sim/time/paused'):a.command('sim/operation/pause_off',.2)
            time.sleep(.08)
        raise RuntimeError('Flight clock failed to advance')
    finally:a.command('sim/operation/pause_on',.2)

def snap(a,name):
    a.refresh_catalogs()
    names=[n for n in a.datarefs if n.startswith('fsim_hud/') or n.startswith('sim/graphics/view/') or n.startswith('sim/flightmodel/position/')]
    if a.get_scalar('fsim_hud/version')<144:
        names=[n for n in names if n!='fsim_hud/runway_projection_valid']
    names += ['sim/time/paused','sim/operation/override/override_planepath','sim/operation/override/override_joystick']
    state=a.get_batch(names)
    (R/(name+'.json')).write_text(json.dumps(state,indent=2))
    capture.photo(a,name)
    print(name,'version',state['fsim_hud/version'],'active',state['fsim_hud/active'],flush=True)

def init(a,name,airport='KEDW',reverse=False,along=-6500,height=6500,cross=0,bank=0,aircraft_path='Aircraft/Testing/Shuttle Runway Trial/Orbiter_Glider.acf'):
    rows=[r.split(',') for r in (ROOT/'Aircraft/Testing/Shuttle Runway Trial/plugins/ShuttleHUD/runways.csv').read_text().splitlines()]
    row=rows[(0 if reverse else 1) if airport=='KEDW' else (3 if reverse else 2)]
    lat,lon,endlat,endlon,width,displaced,elev=map(float,row[1:])
    n=(endlat-lat)*111120;e=(endlon-lon)*111120*math.cos(math.radians((lat+endlat)/2));length=math.hypot(n,e)
    un=n/length;ue=e/length;heading=math.degrees(math.atan2(e,n))%360
    alt=(694.69 if airport=='KEDW' else 2.44)+height*.3048
    latitude=lat+((along+displaced)*un-cross*ue)/111120
    longitude=lon+((along+displaced)*ue+cross*un)/(111120*math.cos(math.radians(lat)))
    p=json.loads((ROOT/'Output/shuttle-hud-20260909/release-flight-request.json').read_text())
    p['data']['aircraft']['path']=aircraft_path
    p['data']['lle_air_start'].update(latitude=latitude,longitude=longitude,elevation_in_meters=alt,heading_true=heading,pitch_in_degrees=-10,speed_in_meters_per_second=170)
    p['data']['weather']['definition'].update(latitude_in_degrees=latitude,longitude_in_degrees=longitude,elevation_in_meters=alt)
    (R/(name+'-request.json')).write_text(json.dumps(p,indent=2))
    subprocess.run([sys.executable,str(R/'scenery.py'),'disable'],check=True)
    try:
        try:a.load_flight(p)
        except XPlaneConnectionError as exc:print('Flight load connection:',str(exc)[:100],flush=True)
        end=time.monotonic()+240
        while time.monotonic()<end:
            try:
                a.refresh_catalogs();a.command('sim/operation/pause_on',.2)
                if a.get_scalar('sim/time/paused')==1 and abs(a.get_scalar('sim/flightmodel/position/elevation')-alt)<50:break
            except XPlaneConnectionError:pass
            time.sleep(.5)
        else:raise RuntimeError('Paused air start did not load')
        advance(a,.65)
        a.set_dataref('sim/operation/override/override_planepath',[1]+[0]*19)
        a.set_dataref('sim/flightmodel/position/q',quaternion(heading,-10,bank))
        h=math.radians(heading);g=math.radians(-20)
        for n,v in dict(local_vx=170*math.cos(g)*math.sin(h),local_vy=170*math.sin(g),local_vz=-170*math.cos(g)*math.cos(h),P=0,Q=0,R=0).items():a.set_dataref('sim/flightmodel/position/'+n,v)
        for n,v in {'sim/cockpit/autopilot/autopilot_mode':0,'sim/cockpit2/switches/avionics_power_on':1,'sim/cockpit2/switches/HUD_on':1,'sim/cockpit2/switches/HUD_brightness_ratio':1,'fsim_hud/declutter_mode':0,'fsim_hud/att_ref_caged':0}.items():a.set_dataref(n,v)
        a.set_dataref('sim/cockpit2/electrical/battery_on',[1]*len(a.get_raw('sim/cockpit2/electrical/battery_on')))
        a.set_dataref('sim/operation/override/override_planepath',[0]*20)
        advance(a,.15)
        a.command('fsim_hud/view',.2);a.set_dataref('sim/graphics/view/field_of_view_deg',65)
        time.sleep(.3);snap(a,name+'-cockpit')
        a.command('fsim_hud/fullscreen',.2);time.sleep(.3);snap(a,name+'-fullscreen')
    finally:
        subprocess.run([sys.executable,str(R/'scenery.py'),'restore'],check=True)
        a.command('sim/operation/pause_on',.2)
        a.set_dataref('sim/operation/override/override_planepath',[0]*20)

if __name__=='__main__':
    with XPlaneApi(port=8144,timeout=8) as a:
        end=time.monotonic()+120
        while True:
            try:a.refresh_catalogs();break
            except XPlaneConnectionError:
                if time.monotonic()>end:raise
                time.sleep(.5)
        if sys.argv[1]=='baseline':init(a,'baseline-143')
        elif sys.argv[1]=='first':init(a,'corrected-144')
        elif sys.argv[1]=='reload-final':
            init(a,'swap-release-143',aircraft_path='Aircraft/OrgForum/Space Shuttle F-SIM HUD/Orbiter_Glider.acf')
            assert a.get_scalar('fsim_hud/version')==143
            binary=ROOT/'Aircraft/Testing/Shuttle Runway Trial/plugins/ShuttleHUD/64/win.xpl'
            shutil.copy2(binary,R/'candidate-144-a.xpl')
            shutil.copy2(Path('V:/src/xplane/target/release/shuttle_hud.dll'),binary)
            init(a,'accepted-144-edwards')
        elif sys.argv[1]=='accepted-matrix':
            for name,args in [('edwards-close',dict(along=-3500,height=3200)),('edwards-offset',dict(along=-6500,height=6500,cross=300,bank=10)),('edwards-reciprocal',dict(reverse=True)),('kennedy-15',dict(airport='KTTS')),('kennedy-33',dict(airport='KTTS',reverse=True))]:
                init(a,'accepted-144-'+name,**args)
        elif sys.argv[1]=='installed':
            init(a,'installed-144-edwards',along=-8500,height=6500,aircraft_path='Aircraft/OrgForum/Space Shuttle F-SIM HUD/Orbiter_Glider.acf')
            assert a.get_scalar('fsim_hud/version')==144
            assert a.get_scalar('fsim_hud/runway_projection_valid')==1
            a.set_dataref('sim/graphics/view/field_of_view_deg',25)
            time.sleep(.4);snap(a,'installed-144-threshold-zoom-on')
            a.set_dataref('fsim_hud/enabled',0)
            a.set_dataref('sim/graphics/view/field_of_view_deg',25)
            a.set_dataref('sim/graphics/view/field_of_view_vertical_ratio',-.5)
            time.sleep(.4);snap(a,'installed-144-threshold-zoom-off')
            a.set_dataref('fsim_hud/enabled',1)
            a.command('sim/view/forward_with_hud',.2)
            a.set_dataref('sim/graphics/view/field_of_view_deg',65)
            a.set_dataref('sim/graphics/view/field_of_view_vertical_ratio',0)
            time.sleep(.3);snap(a,'installed-144-shift-w')
        elif sys.argv[1]=='matrix':
            for name,args in [('edwards-close',dict(along=-3500,height=3200)),('edwards-offset',dict(along=-6500,height=6500,cross=300,bank=10)),('edwards-reciprocal',dict(reverse=True)),('kennedy-15',dict(airport='KTTS')),('kennedy-33',dict(airport='KTTS',reverse=True))]:
                init(a,'corrected-144-'+name,**args)
        elif sys.argv[1]=='detail':
            init(a,'detail-144-edwards',along=-8500,height=6500)
            a.set_dataref('sim/graphics/view/field_of_view_deg',25)
            time.sleep(.4);snap(a,'detail-144-edwards-zoom-on')
            a.set_dataref('fsim_hud/enabled',0)
            # Disabling restores the user's FOV. Reapply only for this paired
            # paused evidence image; the next HUD-view command restores defaults.
            a.set_dataref('sim/graphics/view/field_of_view_deg',25)
            a.set_dataref('sim/graphics/view/field_of_view_vertical_ratio',-.5)
            time.sleep(.4);snap(a,'detail-144-edwards-zoom-off')
            a.set_dataref('fsim_hud/enabled',1)
        elif sys.argv[1]=='snap':snap(a,sys.argv[2])
