"""Versioned TorqueSim setup adapter; all airborne guidance is native."""
from __future__ import annotations
import csv
import json
import math
from pathlib import Path
import shutil
import sys
import time
from .config import ROOT, atomic_json, native_text, sha
from .protocol import decode

sys.path.insert(0, str(ROOT / 'vendor'))
from flight_test.api import XPlaneApi
from flight_test.power_off_180 import PowerOff180Config, PowerOff180Runner
from flight_test.geometry import RunwayGeometry
from flight_test.short_field import quaternion

def wind_air_start(tas_mps, track_deg, wind_speed_kt, wind_from_deg):
    """Air-relative velocity plus meteorological wind, for a fixed ground track."""
    wind_mps=wind_speed_kt/1.94384449
    relative=math.radians(wind_from_deg-track_deg)
    heading=track_deg+math.degrees(math.asin(max(-.5,min(.5,wind_mps*math.sin(relative)/tas_mps))))
    air=math.radians(heading);wind=math.radians(wind_from_deg)
    return heading%360, tas_mps*math.sin(air)-wind_mps*math.sin(wind), 0., -tas_mps*math.cos(air)+wind_mps*math.cos(wind)

def full_fuel_stable(samples):
    if len(samples)<4:
        return False
    recent=samples[-4:];span=recent[-1]['sim_time']-recent[0]['sim_time']
    if span<2.5:
        return False
    return (all(27.5<=s['right_usg']<=28.1 and 27.25<=s['left_usg']<=28.1
                and 28.5<=s['right_volume_usg']<=29.5 for s in recent)
            and max(s['right_kg'] for s in recent)-min(s['right_kg'] for s in recent)<.01
            and max(s['left_kg'] for s in recent)-min(s['left_kg'] for s in recent)<.06)

class NativeAdapter(PowerOff180Runner):
    def __init__(self, api, resolved, card_directory, plugin_directory, status):
        self.resolved = resolved
        self.card_directory = card_directory
        self.plugin_directory = plugin_directory
        self.status = status
        p = resolved['parameters']; setup = resolved['setup']
        runway = RunwayGeometry('KCDW', '22', p['threshold_lat'], p['threshold_lon'], p['end_lat'], p['end_lon'], setup['runway_elevation_ft'])
        cfg = PowerOff180Config(aircraft_path=resolved['aircraft_path'], require_custom_mod=0,
            use_torquesim_mass=1, use_torquesim_flaps=1, use_torquesim_engine=1,
            downwind_cross_ft=p['entry_cross_ft'], downwind_altitude_agl_ft=p['entry_agl_ft'],
            entry_kias=p['entry_kias'], start_along_ft=setup['start_along_ft'],
            initial_pitch_deg=p['initial_pitch_deg'], initial_throttle=p['initial_throttle'],
            downwind_level_pitch_deg=p['level_pitch_deg'], target_mass_kg=p['mass_target_lb']/2.20462262185)
        super().__init__(api, cfg, card_directory/'unused-legacy-result.json', runway=runway)

    def _prepare_flight(self):
        result=super()._prepare_flight()
        self._normalize_air_start()
        return result

    def _normalize_air_start(self):
        # Seed once while paused. Held-path IAS/TAS are stale; the sustained
        # native entry gate is the authority for achieved airborne airspeed.
        p=self.resolved['parameters']
        tas=p['entry_kias']*1.022/1.94384449
        self._hold_setup_path()
        self.api.set_dataref('sr20g6/test_controller/armed',0)
        heading,vx,vy,vz=wind_air_start(tas,self.runway.heading_true_deg+180,
            p['wind_speed_kt'],self.runway.heading_true_deg+p['wind_offset_deg'])
        self.api.set_dataref('sim/flightmodel/position/q',quaternion(heading,p['initial_pitch_deg'],0))
        expected={'local_vx':vx,'local_vy':vy,'local_vz':vz,'P':0,'Q':0,'R':0}
        for name,value in expected.items():
            self.api.set_dataref('sim/flightmodel/position/'+name,value)
        actual={name:float(self.api.get_scalar('sim/flightmodel/position/'+name)) for name in expected}
        if any(abs(actual[k]-v)>.1 for k,v in expected.items()):
            raise RuntimeError('Wind-relative velocity initialization readback failed')
        wind=self.api.get_batch(['sim/weather/aircraft/wind_now_speed_msc','sim/weather/aircraft/wind_now_direction_degt'])
        atomic_json(self.card_directory/'air-start-seed.json',{
            'setup_only':True,'seeded_while_paused':True,'wind_compensated':True,
            'nominal_ias_kias':p['entry_kias'],'seed_air_tas_mps':tas,'heading_true_deg':heading,
            'expected_velocity':expected,'readback_velocity':actual,'weather_readback':wind,
            'measurement_authority':'Sustained native airborne entry gate; held-path IAS/TAS are not used.'})
        self._controller_setup()
        self.api.set_dataref('sim/operation/override/override_planepath',[0]*20)
        if int(self.api.get_raw('sim/operation/override/override_planepath')[0])!=0:
            raise RuntimeError('Air-start path override did not release')
    def _flight_payload(self):
        payload, *rest = super()._flight_payload()
        p = self.resolved['parameters']
        for layer in payload['data']['weather']['definition']['wind']:
            layer['speed_in_knots'] = p['wind_speed_kt']
            layer['direction_in_degrees_true'] = (self.runway.heading_true_deg+p['wind_offset_deg']) % 360
        atomic_json(self.card_directory/'flight-request.json', payload)
        return payload, *rest

    def _wait_for_catalog(self, timeout=180):
        deadline = time.monotonic()+timeout
        while time.monotonic()<deadline:
            try:
                self.api.refresh_catalogs()
                if 'sim/operation/pause_on' in self.api.commands:
                    self.api.command('sim/operation/pause_on', .2)
                    if int(self.api.get_scalar('sim/time/paused'))==1:
                        return
            except Exception:
                pass
            time.sleep(.3)
        raise RuntimeError('Unable to establish immediate post-load pause')

    def _set_pause(self, paused):
        deadline = time.monotonic()+90
        while True:
            try:
                return super()._set_pause(paused)
            except RuntimeError:
                if paused or time.monotonic()>=deadline:
                    raise
                time.sleep(.5)

    def _hold_setup_path(self):
        deadline=time.monotonic()+5
        attempts=0
        while time.monotonic()<deadline:
            self._set_pause(True)
            self.api.set_dataref('sim/operation/override/override_planepath',[1]+[0]*19)
            time.sleep(.15)
            attempts+=1
            if int(self.api.get_raw('sim/operation/override/override_planepath')[0])==1:
                return attempts
        raise RuntimeError('Unable to establish held setup path for aircraft activation within five seconds')

    def _before_native_mass_setup(self):
        deadline=time.monotonic()+120
        evidence=[];consecutive=0
        while time.monotonic()<deadline:
            if (self.card_directory.parents[1]/'cancel.request').exists():
                raise RuntimeError('Cancelled during aircraft activation')
            hold_attempts=self._hold_setup_path()
            self.status.update('waiting_for_aircraft_activation')
            self._set_pause(False)
            time.sleep(.5)
            self._set_pause(True)
            native=float(self.api.get_scalar('afm/sr/mass/total_kg'))
            if native>1000:
                # X-Plane recomputes m_total only with its flight model released.
                # This is setup only; the inherited setup reseeds the air start.
                self.api.set_dataref('sim/operation/override/override_planepath',[0]*20)
                self._set_pause(False)
                time.sleep(.5)
                self._set_pause(True)
                hold_attempts+=self._hold_setup_path()
            simulator=float(self.api.get_scalar('sim/flightmodel/weight/m_total'))
            evidence.append({'epoch':time.time(),'native_mass_kg':native,'simulator_mass_kg':simulator,
                             'path_hold_attempts':hold_attempts})
            consecutive=consecutive+1 if native>1000 and abs(simulator-native)<5 else 0
            if consecutive>=2:
                atomic_json(self.card_directory/'activation-readback.json',evidence)
                self.status.update('preparing')
                return
        atomic_json(self.card_directory/'activation-readback.json',evidence)
        raise RuntimeError('TorqueSim physics did not activate and synchronize mass within 120 seconds')

    def _after_native_fuel_setup(self):
        # Verify the add-on's saturated full-tank request while setup is held.
        # Gauge readings vary with attitude; use both physical tank volume and
        # mass stability. The inherited setup later reseeds attitude/velocity.
        evidence=[];deadline=time.monotonic()+120
        refs={'sim_time':'sim/time/total_flight_time_sec','left_kg':'afm/sr/fuel/massL_kg',
              'right_kg':'afm/sr/fuel/massR_kg','left_usg':'afm/sr/fuel/sensL_USG',
              'right_usg':'afm/sr/fuel/sensR_USG','native_total_kg':'afm/sr/mass/total_kg',
              'right_volume_usg':'afm/sr/fuel/volumes/tank_r_usg',
              'pitch_deg':'sim/flightmodel/position/theta','bank_deg':'sim/flightmodel/position/phi'}
        while time.monotonic()<deadline:
            if (self.card_directory.parents[1]/'cancel.request').exists():
                raise RuntimeError('Cancelled during full-fuel stabilization')
            self._hold_setup_path()
            self.status.update('settling_full_fuel')
            self._set_pause(False)
            time.sleep(1)
            self._set_pause(True)
            values=self.api.get_batch(list(refs.values()))
            evidence.append({key:float(values[name]) for key,name in refs.items()})
            stable=full_fuel_stable(evidence)
            atomic_json(self.card_directory/'fuel-stabilization.json',{'stable':stable,'samples':evidence})
            if stable:
                self.status.update('preparing')
                return
        raise RuntimeError('Full-tank quantity and mass did not stabilize within 120 seconds')

    def _bind_measured_loading(self):
        setup=self.resolved['setup']
        stations={'pilot_kg':setup['pilot_kg'],'copilot_kg':setup['copilot_kg'],
                  'left_rear_kg':0,'right_rear_kg':0,'baggage_kg':0}
        if any(abs(self.mass_setup[name]-expected)>.01 for name,expected in stations.items()):
            raise RuntimeError('Measured payload does not match 400 lb in the front seats')
        observed_lb=float(self.api.get_scalar('sim/flightmodel/weight/m_total'))*2.20462262185
        if not setup['loaded_mass_min_lb']<=observed_lb<=setup['loaded_mass_max_lb']:
            raise RuntimeError(f'Full-fuel loading outside the adapter envelope: {observed_lb:.3f} lb')
        nominal=self.resolved['parameters']['mass_target_lb']
        self.resolved['parameters']['mass_target_lb']=observed_lb
        atomic_json(self.card_directory/'loading-mass-reference.json',{
            'mode':'measured full-fuel request and verified 400 lb front-seat payload',
            'nominal_mass_lb':nominal,'measured_mass_lb':observed_lb,
            'drift_limit_lb':self.resolved['parameters']['mass_tolerance_lb'],
            'allowed_loading_envelope_lb':[setup['loaded_mass_min_lb'],setup['loaded_mass_max_lb']]})
        atomic_json(self.card_directory/'effective-config.json',self.resolved)

    def execute(self):
        token = self.resolved['parameters']['run_token']
        self.status.update('preparing', native=None)
        polls = []; probe = None
        terminal = None
        media = None
        try:
            self._prepare_flight()
            self.api.refresh_catalogs()
            vr_enabled=int(self.api.get_scalar('sim/graphics/VR/enabled'))
            atomic_json(self.card_directory/'rendering-readback.json',{'profile':'2d','vr_enabled':vr_enabled})
            if vr_enabled!=0:
                raise RuntimeError('Automated 2D rendering profile was not established')
            if self.resolved.get('record_video'):
                from .video import VideoCapture
                media=VideoCapture(self.api,self.card_directory,self.resolved['xplane_root'])
                media.prepare()
            self.api.require_datarefs(['xpt/snapshot','xpt/heartbeat','xpt/configuration_error','xpt/rust_implementation'])
            if self.api.get_scalar('xpt/rust_implementation')!=1:
                raise RuntimeError('Rust guidance implementation missing')
            if self.api.get_scalar('sr20g6/test_controller/rust_implementation')!=1:
                raise RuntimeError('Rust attitude implementation missing')
            loading_refs = ['sim/aircraft/weight/acf_m_fuel_tot', 'sim/flightmodel/weight/m_total',
                'sim/cockpit2/fuel/fuel_temp_at_fuel_tank', 'afm/sr/fuel/massL_kg', 'afm/sr/fuel/massR_kg',
                'afm/sr/fuel/volumes', 'afm/sr/fuel/sensL_USG', 'afm/sr/fuel/sensR_USG',
                'afm/sr/mass/empty_kg', 'afm/sr/mass/oil_kg', 'afm/sr/mass/total_kg']
            atomic_json(self.card_directory/'loading-diagnostics.json',
                self.api.get_batch([name for name in loading_refs if name in self.api.datarefs]))
            atomic_json(self.card_directory/'setup-readback.json', {'loading': self.mass_setup, 'engine': self.engine_setup,
                'fuel_kg': self.api.get_raw('sim/flightmodel/weight/m_fuel'), 'paused': self.api.get_scalar('sim/time/paused'),
                'path_override': self.api.get_raw('sim/operation/override/override_planepath'),
                'guidance_rust_implementation':self.api.get_scalar('xpt/rust_implementation'),
                'attitude_rust_implementation':self.api.get_scalar('sr20g6/test_controller/rust_implementation'),
                'axis_controller_version_minor': self.api.get_scalar('sr20g6/test_controller/version_minor')})
            self._bind_measured_loading()
            staged = native_text(self.resolved['parameters'])
            (self.plugin_directory/'active-card.ini').write_text(staged, encoding='ascii', newline='\n')
            self.api.command('xpt/configure', .2)
            effective_path = self.plugin_directory/'effective-card.ini'
            deadline = time.monotonic()+5
            while time.monotonic()<deadline:
                if int(self.api.get_scalar('xpt/configuration_error')):
                    raise RuntimeError('Native configuration rejected')
                if effective_path.exists():
                    effective = {k:float(v) for k,v in (line.split('=') for line in effective_path.read_text().splitlines())}
                    if effective == self.resolved['parameters']:
                        break
                time.sleep(.1)
            else:
                raise RuntimeError('Native effective configuration does not match the resolved card')
            shutil.copy2(effective_path,self.card_directory/'native-effective.ini')
            self.api.set_dataref('xpt/heartbeat',1)
            self.api.command('xpt/start', .2)
            time.sleep(.1)
            initial = decode(self.api.get_raw('xpt/snapshot'))
            if initial['phase']!='downwind' or initial['config_token']!=token:
                raise RuntimeError(f'Native start not acknowledged: {initial["phase"]}')
            self._set_pause(False)
            start = time.monotonic(); heartbeat=1
            wall_limit=self.resolved['supervision']['wall_timeout_seconds']
            while time.monotonic()-start < wall_limit:
                if (self.card_directory.parents[1]/'cancel.request').exists():
                    self.api.command('xpt/abort', .2)
                heartbeat+=1;self.api.set_dataref('xpt/heartbeat',heartbeat)
                sample=decode(self.api.get_raw('xpt/snapshot'))
                sample['observed_epoch']=time.time();polls.append(sample)
                if media:media.observe(sample)
                self.status.update(sample['phase'], native={k:sample[k] for k in ['sim_time','agl_ft','ias_kias','runway_along_ft','runway_cross_ft','pitch_deg','native_steps','control_dt_s','heartbeat_age_s','reason']})
                if sample['phase'] in ('complete','aborted'):
                    terminal=sample
                    atomic_json(self.card_directory/'native-terminal-safety.json',{
                        'observed_before_supervisor_cleanup':True,
                        'phase':sample['phase'],'reason':sample['reason'],
                        'readback':self.api.get_batch(['sim/time/paused','sr20g6/test_controller/armed',
                            'sr20g6/test_controller/active','sr20g6/test_controller/release_reason',
                            'sim/operation/override/override_joystick_roll',
                            'sim/operation/override/override_joystick_pitch',
                            'sim/operation/override/override_joystick_heading',
                            'sim/cockpit2/engine/actuators/throttle_ratio_all',
                            'sim/operation/override/override_planepath'])})
                    break
                disconnect=self.resolved['supervision']['disconnect_probe_seconds']
                if disconnect and probe is None and sample['cut_sim_time']>=0:
                    probe={'before_steps':sample['native_steps'],'requested_wall_seconds':disconnect,'before_sim_time':sample['sim_time']}
                    self.status.update(event='supervision_gap_probe', gap_seconds=disconnect)
                    time.sleep(disconnect)
                    after=decode(self.api.get_raw('xpt/snapshot'))
                    probe.update(after_steps=after['native_steps'],after_sim_time=after['sim_time'],after_phase=after['phase'],after_reason=after['reason'])
                time.sleep(self.resolved['supervision']['poll_seconds'])
            if terminal is None:
                raise TimeoutError('Supervisor wall-time limit reached')
        finally:
            if terminal is None or terminal.get('phase') not in ('complete','aborted'):
                try:self.api.command('xpt/abort', .2)
                except Exception:pass
            for name,value in [('sr20g6/test_controller/armed',0),('sim/cockpit2/engine/actuators/throttle_ratio_all',0),
                               ('sim/operation/override/override_planepath',[0]*20),('sim/operation/override/override_toe_brakes',0),
                               ('sim/cockpit2/controls/left_brake_ratio',0),('sim/cockpit2/controls/right_brake_ratio',0)]:
                self._safe_set(name,value)
            try:self._set_pause(True)
            except Exception:pass
            if media:media.stop()
            native_trace=self.plugin_directory/f'trace-{token}.csv'
            if native_trace.exists():shutil.copy2(native_trace,self.card_directory/'trace.csv')
            atomic_json(self.card_directory/'supervision.json',{'polls':polls,'gap_probe':probe,'terminal':terminal})
        with (self.card_directory/'trace.csv').open(encoding='utf-8') as stream:
            rows=[{k:float(v) for k,v in row.items()} for row in csv.DictReader(stream)]
        if not rows:
            raise RuntimeError('Native trace is empty')
        return {'schema_version':1,'effective_config':self.resolved,'effective_config_sha256':sha(self.resolved),
                'terminal':terminal,'native_trace_samples':len(rows),'gap_probe':probe}, rows
