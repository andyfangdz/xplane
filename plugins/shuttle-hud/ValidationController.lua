-- Validation pilot for the shared Shuttle HUD guidance. Native controls only.
-- This script belongs in a separate test aircraft, never in the released HUD.
function writable() end
armed=create_dataref('shuttle_demo/armed','number',writable)
heartbeat=create_dataref('shuttle_demo/heartbeat','number',writable)
trial=create_dataref('shuttle_demo/trial','number',writable)
chute_enabled=create_dataref('shuttle_demo/chute_enabled','number',writable)
phase=create_dataref('shuttle_demo/phase','number')
along_out=create_dataref('shuttle_demo/along_m','number')
cross_out=create_dataref('shuttle_demo/cross_m','number')
touch_vy=create_dataref('shuttle_demo/touchdown_vy','number')
touch_ias=create_dataref('shuttle_demo/touchdown_ias','number')
touch_eas=create_dataref('shuttle_demo/touchdown_eas','number')
touch_along=create_dataref('shuttle_demo/touchdown_along','number')
touch_cross=create_dataref('shuttle_demo/touchdown_cross','number')
gphase=find_dataref('fsim_hud/guidance_phase')
gheight=find_dataref('fsim_hud/height_command_m')
ggamma=find_dataref('fsim_hud/gamma_command_deg')
gbrake=find_dataref('fsim_hud/speedbrake_command_ratio')
ggear=find_dataref('fsim_hud/gear_command')
chute_ratio=find_dataref('fsim_hud/chute_area_ratio')
along=find_dataref('fsim_hud/along_m')
cross=find_dataref('fsim_hud/cross_m')
height=find_dataref('fsim_hud/main_wheel_height_ft')
radar_height=find_dataref('fsim_hud/radar_height_ft')
eas=find_dataref('fsim_hud/equivalent_airspeed_kt')
hudenabled=find_dataref('fsim_hud/plugin_enabled')
version=find_dataref('fsim_hud/version')
vx=find_dataref('sim/flightmodel/position/local_vx')
vy=find_dataref('fsim_hud/vertical_velocity_mps')
local_vy=find_dataref('sim/flightmodel/position/local_vy')
groundspeed=find_dataref('fsim_hud/groundspeed_mps')
vz=find_dataref('sim/flightmodel/position/local_vz')
pitch=find_dataref('sim/flightmodel/position/theta')
bank=find_dataref('sim/flightmodel/position/phi')
heading=find_dataref('sim/flightmodel/position/psi')
prate=find_dataref('sim/flightmodel/position/P')
qrate=find_dataref('sim/flightmodel/position/Q')
rrate=find_dataref('sim/flightmodel/position/R')
ias=find_dataref('sim/flightmodel/position/indicated_airspeed')
aoa=find_dataref('sim/flightmodel/position/alpha')
elev=find_dataref('sim/flightmodel/position/elevation')
agl=find_dataref('sim/flightmodel/position/y_agl')
mass=find_dataref('sim/flightmodel/weight/m_total')
nz=find_dataref('sim/flightmodel/forces/g_nrml')
simtime=find_dataref('sim/time/total_flight_time_sec')
realtime=find_dataref('sim/time/total_running_time_sec')
paused=find_dataref('sim/time/paused')
replay=find_dataref('sim/time/is_in_replay')
onground=find_dataref('sim/flightmodel2/gear/on_ground')
deploy=find_dataref('sim/flightmodel2/gear/deploy_ratio')
over_g=find_dataref('sim/flightmodel/failures/over_g')
override=find_dataref('sim/operation/override/override_joystick')
ap_mode=find_dataref('sim/cockpit/autopilot/autopilot_mode')
yoke=find_dataref('sim/joystick/yoke_pitch_ratio')
roll=find_dataref('sim/joystick/yoke_roll_ratio')
rudder=find_dataref('sim/joystick/yoke_heading_ratio')
gear=find_dataref('sim/cockpit2/controls/gear_handle_down')
speedbrake=find_dataref('sim/cockpit2/controls/speedbrake_ratio')
actual_speedbrake=find_dataref('sim/flightmodel2/controls/speedbrake_ratio')
brake=find_dataref('sim/cockpit2/controls/parking_brake_ratio')
throttle=find_dataref('sim/cockpit2/engine/actuators/throttle_ratio_all')
chute=find_dataref('sim/cockpit/switches/parachute_on')
pause_cmd=find_command('sim/operation/pause_on')
chute_cmd=find_command('sim/flight_controls/deploy_parachute')
logpath='D:/X-Plane 12/Output/shuttle-flare-20260910/'
active=false;logfile=nil
function clamp(x,a,b) return math.max(a,math.min(b,x)) end
function wrap(x) return (x+180)%360-180 end
function release()
 override=0;yoke=0;roll=0;rudder=0;armed=0;active=false
 if logfile then logfile:flush();logfile:close();logfile=nil end
end
function stop(code) phase=code;pause_cmd:once();release() end
function flight_start() release();phase=0;chute_enabled=1 end
function aircraft_unload() release() end
function flight_crash() stop(-3) end
function after_physics()
 if armed<.5 then if active then stop(-4) end;return end
 if replay>0 or hudenabled<1 or version<122 then stop(-6);return end
 if not active then
  active=true;elapsed=0;integral=0;gamma_integral=0;last_sample=-1;touch_time=-1;derotate_time=-1;jettisoned=false;last_vy=vy;last_gs=groundspeed;alpha_rate=0;last_gamma=math.deg(math.atan(vy/math.max(10,last_gs)));last_gcmd=ggamma;gamma_rate=0;command_rate=0
  last_beat=heartbeat;last_beat_time=realtime
  touch_vy=0;touch_ias=0;touch_eas=0;touch_along=0;touch_cross=0
  logfile=io.open(logpath..'trace-'..math.floor(trial)..'.csv','w')
  if logfile then logfile:write('time,phase,along_m,cross_m,elevation_m,agl_m,height_ft,ias,eas,groundspeed_kt,pitch,bank,alpha,vy,gamma,gamma_cmd,h_cmd,yoke,roll,rudder,speedbrake,speedbrake_actual,Q,mass,nz,main_wow,nose_wow,gear_cmd,gear0,gear1,gear2,chute,brakes,derotating,local_vy,radar_height_ft\n') end
 end
 if heartbeat~=last_beat then last_beat=heartbeat;last_beat_time=realtime end
 if realtime-last_beat_time>8 then stop(-5);return end
 if paused>0 then return end
 local dt=clamp(SIM_PERIOD,.001,.1);elapsed=elapsed+dt
 override=1;ap_mode=0;throttle=0
 along_out=along;cross_out=cross
 local gs=groundspeed
 local gamma=math.deg(math.atan(vy/math.max(10,gs)))
 -- Pitch rate includes the changing angle of attack as the glider decelerates.
 -- Filter that measured term so a constant flightpath does not slowly sink away.
 local adot=clamp(-2*aoa*(gs-last_gs)/(dt*math.max(gs,35)),-.6,.6);last_gs=gs
 alpha_rate=alpha_rate+(adot-alpha_rate)*dt/(1+dt)
 local gd=clamp((gamma-last_gamma)/dt,-4,4);last_gamma=gamma
 local cd=clamp((ggamma-last_gcmd)/dt,-2,2);last_gcmd=ggamma
 gamma_rate=gamma_rate+(gd-gamma_rate)*dt/(.5+dt)
 command_rate=command_rate+(cd-command_rate)*dt/(.5+dt)
 local rawq=1.4*(ggamma-gamma)+gamma_integral+command_rate-1.8*(gamma_rate-command_rate)+alpha_rate
 if math.abs(rawq)<3.5 then gamma_integral=clamp(gamma_integral+.2*(ggamma-gamma)*dt,-.7,.7) end
 local qcmd=clamp(rawq,-3.5,3.5)
 if pitch>12 and height<115 then qcmd=math.min(qcmd,.7*(12-pitch)) end
 integral=clamp(integral+.05*(qcmd-qrate)*dt,-.7,.7)
 yoke=clamp(.12*(qcmd-qrate)+integral,-1,1)
 speedbrake=speedbrake+clamp(gbrake-speedbrake,-dt*.5,dt*.5)
 gear=ggear;phase=gphase;brake=0
 local desired_heading=238.115746+math.deg(math.atan(clamp(-cross/1200,-.15,.15)))
 local bcmd=clamp(wrap(desired_heading-heading)*1.8,-15,15)
 if height<100 then bcmd=clamp(bcmd,-3,3) end
 roll=clamp(.05*(bcmd-bank)-.06*prate,-.6,.6)
 rudder=clamp(-.07*rrate,-.25,.25)
 local mainwow=0;if onground[1]>0 or onground[2]>0 then mainwow=1 end
 if mainwow==1 and touch_time<0 then
  touch_time=elapsed;touch_pitch=pitch;touch_vy=last_vy;touch_ias=ias;touch_eas=eas;touch_along=along;touch_cross=cross
  if chute_enabled>.5 then chute_cmd:once() end
 end
 if touch_time>=0 then
  phase=4;gear=1;speedbrake=1
  local since=elapsed-touch_time
  -- JSC-23266 section 5.3.6.7: start at 185 KEAS, build rate over 1--2 s.
  if derotate_time<0 and eas<=185 then derotate_time=elapsed end
  local ptarget=touch_pitch
  local downrate=0
  if derotate_time>=0 then
   local t=elapsed-derotate_time;local u=clamp(t/1.5,0,1)
   downrate=2*u*u*(3-2*u)
   local angle=t<1.5 and 3*(u*u*u-.5*u*u*u*u) or 2*t-1.5
   ptarget=math.max(0,touch_pitch-angle)
  end
  local rq=clamp(.8*(ptarget-pitch)-downrate,-3,2)
  integral=clamp(integral+.12*(rq-qrate)*dt,-.9,.9)
  local chute_trim=chute_enabled>.5 and not jettisoned and (6/18)*chute_ratio or 0
  yoke=clamp(.18*(rq-qrate)+integral-chute_trim,-1,1)
  roll=clamp(-.06*bank-.06*prate,-.4,.4)
  local ground_heading=238.115746+math.deg(math.atan(clamp(-cross/250,-.12,.12)))
  rudder=clamp(wrap(ground_heading-heading)*.06-rrate*.20,-.5,.5)
  if onground[0]>0 or pitch<2.5 then brake=clamp((since-2)*.07,0,.8) end
  if gs*1.943844492<=60 and not jettisoned then if chute_enabled>.5 then chute_cmd:once() end;jettisoned=true end
  if gs<.3 then brake=1;stop(5);return end
  if math.abs(cross)>35 or along>4400 then stop(-2);return end
 end
 if logfile and elapsed-last_sample>=.05 then
  local row={elapsed,phase,along,cross,elev,agl,height,ias,eas,gs*1.943844492,pitch,bank,aoa,vy,gamma,ggamma,gheight,yoke,roll,rudder,speedbrake,actual_speedbrake,qrate,mass,nz,mainwow,onground[0],gear,deploy[0],deploy[1],deploy[2],chute,brake,derotate_time>=0 and 1 or 0,local_vy,radar_height}
  for i=1,#row do row[i]=string.format('%.6f',row[i]) end
  logfile:write(table.concat(row,',')..'\n');last_sample=elapsed
  if math.floor(elapsed)%2==0 then logfile:flush() end
 end
 last_vy=vy
 if elapsed>240 or math.abs(bank)>65 or over_g>0 or (elapsed>2 and eas<130 and height>60) then stop(-1) end
end
