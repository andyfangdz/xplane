from pathlib import Path
import sys,time,json,traceback
R=Path(__file__).parent;sys.path.insert(0,str(R));import fly_trial as f
from cards import snap,config
trial=int(sys.argv[1]);photos='--photos' in sys.argv
with f.XPlaneApi(port=8144,timeout=8) as a:
 config('disable')
 try:f.prepare(a,trial)
 finally:config('restore')
 try:
  a.set_dataref('fsim_hud/declutter_mode',-1);a.set_dataref('fsim_hud/att_ref_caged',0)
  a.set_dataref('shuttle_demo/armed',1);a.set_dataref('shuttle_demo/heartbeat',int(time.monotonic()));a.command('sim/operation/pause_off',.2)
  start=time.monotonic();seen=set();samples=[];next_notice=0;startup_pauses=0
  names=['shuttle_demo/phase','shuttle_demo/armed','sim/time/paused','sim/time/total_flight_time_sec','sim/flightmodel/weight/m_total','sim/flightmodel/position/theta','sim/flightmodel/position/phi','sim/operation/override/override_planepath']+[n for n in a.datarefs if n.startswith('fsim_hud/')]
  while time.monotonic()-start<240:
   a.set_dataref('shuttle_demo/heartbeat',int(time.monotonic()));s=a.get_batch(names);s['wall_elapsed']=time.monotonic()-start;samples.append(s)
   h=s['fsim_hud/main_wheel_height_ft'];phase=s['shuttle_demo/phase']
   if time.monotonic()>next_notice:print('FLIGHT',trial,'phase',phase,'height',round(h),'EAS',round(s['fsim_hud/equivalent_airspeed_kt'],1),'WOW',s['fsim_hud/display_main_wow'],s['fsim_hud/display_nose_wow'],flush=True);next_notice=time.monotonic()+10
   if phase<0:raise RuntimeError('Validation pilot rejected flight: '+str(phase))
   if phase==5:break
   if s['sim/time/paused']:
    if s['sim/time/total_flight_time_sec']<2 and startup_pauses<3:
     a.command('sim/operation/pause_off',.2);startup_pauses+=1
    else:raise RuntimeError('Unexpected pause during flown regression')
   if s['wall_elapsed']>4:
    target=float(sys.argv[sys.argv.index('--mass-lb')+1]);assert abs(s['sim/flightmodel/weight/m_total']/.45359237-target)<2
    assert not any(s['sim/operation/override/override_planepath'])
   if photos and s['wall_elapsed']>4 and s['fsim_hud/equivalent_airspeed_kt']>100:
    tag=''
    if s['fsim_hud/display_nose_wow']:tag='nose-contact'
    elif s['fsim_hud/display_main_wow']:tag='main-contact'
    elif s['fsim_hud/display_phase']==6:tag='css-final-flare'
    elif h<170:tag='inner'
    elif h<1950:tag='preflare'
    elif h<3450:tag='flare-preview'
    elif h<6500:tag='ogs'
    if tag and tag not in seen:
     seen.add(tag);a.command('sim/operation/pause_on',.2);time.sleep(.2)
     snap(a,f'flight-{trial}-{tag}','observed during native flown regression; paused for capture')
     a.set_dataref('shuttle_demo/heartbeat',int(time.monotonic()));a.command('sim/operation/pause_off',.2)
   time.sleep(.15)
  else:raise RuntimeError('Regression flight deadline exceeded')
  a.command('sim/operation/pause_on',.2)
  result=a.get_batch(['shuttle_demo/touchdown_vy','shuttle_demo/touchdown_ias','shuttle_demo/touchdown_eas','shuttle_demo/touchdown_along','shuttle_demo/touchdown_cross','shuttle_demo/phase','shuttle_demo/armed','sim/operation/override/override_joystick','sim/operation/override/override_planepath','sim/time/paused'])
  (R/f'result-{trial}.json').write_text(json.dumps(result,indent=2));(R/f'hud-flight-{trial}.json').write_text(json.dumps({'startup_pauses':startup_pauses,'photos':sorted(seen),'samples':samples},indent=2))
  if photos:snap(a,f'flight-{trial}-stopped','observed stopped state after native flown regression')
  print('RESULT',json.dumps(result),flush=True)
 except Exception:
  (R/f'error-{trial}.txt').write_text(traceback.format_exc());raise
 finally:
  a.command('sim/operation/pause_on',.2);a.set_dataref('shuttle_demo/armed',0);a.set_dataref('sim/operation/override/override_planepath',[0]*20);a.set_dataref('sim/operation/override/override_joystick',0)
