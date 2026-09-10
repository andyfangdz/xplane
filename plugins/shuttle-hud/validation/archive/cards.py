from pathlib import Path
import sys,math,time,json,shutil,subprocess
R=Path(__file__).parent;ROOT=R.parents[1]
sys.path.insert(0,str(R));from fly_trial import XPlaneApi,XPlaneConnectionError,quaternion
sys.path.insert(0,str(ROOT/'Output/shuttle-hud-20260909/cockpit'));import check_native as capture
capture.out=R
HEAD=238.115746099;FT=3.280839895
def config(mode):subprocess.run([sys.executable,str(R/'scenery.py'),mode],check=True,capture_output=True)
def advance(a,seconds):
 start=a.get_scalar('sim/time/total_flight_time_sec');a.command('sim/operation/pause_off',.2)
 end=time.monotonic()+25
 try:
  while time.monotonic()<end:
   now=a.get_scalar('sim/time/total_flight_time_sec')
   if now<start:start=now
   if now-start>=seconds:return
   # The port schedules a one-time pause toggle at 0.3 s after flight load.
   if a.get_scalar('sim/time/paused'):a.command('sim/operation/pause_off',.2)
   time.sleep(.06)
  raise RuntimeError('Simulator did not advance the requested actual flight time')
 finally:a.command('sim/operation/pause_on',.2)
def snap(a,name,kind='initialized display card'):
 names=[n for n in a.datarefs if n.startswith('fsim_hud/')]+[
  'sim/time/paused','sim/time/total_flight_time_sec','sim/flightmodel/position/theta','sim/flightmodel/position/phi',
  'sim/flightmodel/position/elevation','sim/flightmodel2/gear/deploy_ratio','sim/flightmodel2/gear/on_ground',
  'sim/cockpit/autopilot/autopilot_mode','sim/cockpit2/autopilot/servos_on','sim/operation/override/override_planepath']
 data={'kind':kind,'state':a.get_batch(names)}
 capture.photo(a,name);(R/(name+'.json')).write_text(json.dumps(data,indent=2));print(name,data['state']['fsim_hud/display_phase'],data['state']['fsim_hud/display_flags'],flush=True)
 return data['state']
def init(a,name,height=8500,along=-9500,pitch=-10,bank=0,heading=HEAD,gamma=-20,eas=300,gear=0,aircraft_path='Aircraft/Testing/Shuttle Symbology Trial/Orbiter_Glider.acf'):
 config('disable')
 try:
  p=json.loads((ROOT/'Output/shuttle-hud-20260909/release-flight-request.json').read_text())
  north=(along+542)*(-.528204999290666);east=(along+542)*(-.8491168816625587)
  gy=a.get_raw('sim/aircraft/parts/acf_gear_ynodef')[1]-a.get_raw('sim/aircraft/parts/acf_gear_leglen')[1]-a.get_raw('sim/flightmodel2/gear/tire_radius_mtrs')[1]
  gz=a.get_raw('sim/aircraft/parts/acf_gear_znodef')[1]
  offset=gy*math.cos(math.radians(pitch))*math.cos(math.radians(bank))-gz*math.sin(math.radians(pitch))
  alt=694.691+height/FT-offset
  sigma=(1-2.25577e-5*alt)**4.25588;tas=eas*.514444/math.sqrt(sigma)
  p['data']['aircraft']['path']=aircraft_path
  p['data']['lle_air_start'].update(latitude=34.91886525+north/111120,longitude=-117.85736822+east/(111120*math.cos(math.radians(34.91886525))),elevation_in_meters=alt,heading_true=heading,pitch_in_degrees=pitch,speed_in_meters_per_second=tas)
  (R/(name+'-request.json')).write_text(json.dumps(p,indent=2))
  try:a.load_flight(p)
  except XPlaneConnectionError:pass
  end=time.monotonic()+120
  while time.monotonic()<end:
   try:
    a.refresh_catalogs();a.command('sim/operation/pause_on',.2)
    if a.get_scalar('sim/time/paused')==1 and abs(a.get_scalar('sim/flightmodel/position/elevation')-alt)<30:break
   except XPlaneConnectionError:pass
   time.sleep(.3)
  else:raise RuntimeError('Failed to initialize paused card')
  if '/Testing/' in aircraft_path:a.set_dataref('shuttle_demo/armed',0)
  a.set_dataref('sim/operation/override/override_planepath',[0]*20)
  advance(a,.65)
  rho=a.get_scalar('sim/weather/rho');assert rho>.1,rho
  tas=eas*.514444/math.sqrt(rho/1.225)
  a.set_dataref('sim/operation/override/override_planepath',[1]+[0]*19)
  a.set_dataref('sim/flightmodel/position/q',quaternion(heading,pitch,bank))
  h=math.radians(heading);g=math.radians(gamma)
  for n,v in dict(local_vx=tas*math.cos(g)*math.sin(h),local_vy=tas*math.sin(g),local_vz=-tas*math.cos(g)*math.cos(h),P=0,Q=0,R=0).items():a.set_dataref('sim/flightmodel/position/'+n,v)
  for n,v in {'sim/cockpit/autopilot/autopilot_mode':0,'sim/cockpit2/controls/gear_handle_down':gear,'sim/cockpit2/controls/speedbrake_ratio':.4,'sim/cockpit2/controls/parking_brake_ratio':0,'sim/cockpit2/switches/avionics_power_on':1,'sim/cockpit2/switches/HUD_on':1,'sim/cockpit2/switches/HUD_brightness_ratio':1,'fsim_hud/declutter_mode':0,'fsim_hud/att_ref_caged':0}.items():a.set_dataref(n,v)
  a.set_dataref('sim/aircraft/parts/acf_gear_deploy',[gear]*len(a.get_raw('sim/aircraft/parts/acf_gear_deploy')))
  a.set_dataref('sim/cockpit2/electrical/battery_on',[1]*len(a.get_raw('sim/cockpit2/electrical/battery_on')))
  a.command('fsim_hud/view',.2);a.set_dataref('sim/graphics/view/field_of_view_deg',65)
  a.set_dataref('sim/operation/override/override_planepath',[0]*20);advance(a,.15);time.sleep(.2)
  assert abs(a.get_scalar('sim/flightmodel/position/phi')-bank)<2,'Bank initialization rejected'
  assert a.get_scalar('fsim_hud/equivalent_airspeed_kt')>eas-8,'Airspeed not ready'
  assert a.get_scalar('fsim_hud/active')==1,'Native renderer not active'
  return snap(a,name)
 finally:
  config('restore')
  try:a.command('sim/operation/pause_on',.2);a.set_dataref('sim/operation/override/override_planepath',[0]*20);a.set_dataref('sim/operation/override/override_joystick',0)
  except Exception:pass
if __name__=='__main__':
 with XPlaneApi(port=8144,timeout=8) as a:
  a.refresh_catalogs()
  if sys.argv[1]=='first':init(a,'ogs-first-138',height=7000,along=-8150)
  elif sys.argv[1]=='matrix':
   specs=[('hac',dict(height=24000,along=-20000,pitch=-5,bank=45,heading=180,gamma=-10,eas=300)),
    ('prefinal',dict(height=14000,along=-15000,pitch=-10,gamma=-17)),
    ('capture',dict(height=8500,along=-8800,pitch=-10,gamma=-17)),
    ('flare-preview',dict(height=3300,along=-5050,pitch=-10)),
    ('preflare',dict(height=1700,along=-3700,pitch=-5)),
    ('inner',dict(height=180,along=-1000,pitch=12,gamma=-1.5,eas=270,gear=1)),
    ('final-css',dict(height=20,along=300,pitch=11,gamma=-.5,eas=240,gear=1)),
    ('gear-up',dict(height=150,along=-700,pitch=12,gamma=-1.5,eas=250))]
   for name,values in specs:init(a,'verified-140-'+name,**values)
