import json
import math
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock,patch
import urllib.request
import urllib.error
from xpt.config import resolve,ROOT,REPO,native_text,PARAMETERS
from xpt.status import Status
from xpt.protocol import FIELDS
from xpt.report import divergence
from xpt.analysis import assess
from xpt.adapter import NativeAdapter,full_fuel_stable,wind_air_start
from xpt.video import VideoCapture
from xpt.cli import source_hashes

class MigrationTests(unittest.TestCase):
    def test_recording_uses_explicit_simulator_root_after_relocation(self):
        capture=VideoCapture(None,Path('evidence'),Path('Z:/simulator'))
        self.assertEqual(capture.root,Path('Z:/simulator'))

    def test_frozen_sources_include_rust_and_capture_dependencies(self):
        hashes=source_hashes()
        for name in ['workspace/crates/poweroff180/parameters.csv','workspace/crates/poweroff180/snapshot.csv',
                     'workspace/crates/poweroff180/src/guidance.rs',
                     'workspace/crates/xplane-airports/src/local.rs',
                     'workspace/crates/xplane-hud/src/projection.rs',
                     'workspace/crates/xplane-hud/src/segment.rs',
                     'workspace/crates/xplane-plugin/src/opengl.rs',
                     'workspace/plugins/poweroff180-controller/src/runtime.rs','workspace/plugins/poweroff180-hud/src/graphics.rs',
                     'scripts/Capture-WasapiLoopback.py','requirements.lock']:
            self.assertIn(name,hashes)

class SetupTests(unittest.TestCase):
    def test_wind_relative_air_start_preserves_airspeed_and_ground_track(self):
        track=29.5;tas=53.0
        for speed,offset in [(0,0),(15,0),(5,180),(10,90),(10,-90)]:
            direction=track+offset
            heading,vx,vy,vz=wind_air_start(tas,track,speed,direction)
            wind=speed/1.94384449
            air_x=vx+wind*math.sin(math.radians(direction))
            air_z=vz-wind*math.cos(math.radians(direction))
            self.assertAlmostEqual(math.hypot(air_x,air_z),tas)
            self.assertAlmostEqual(math.degrees(math.atan2(vx,-vz)),track)
            self.assertAlmostEqual(math.degrees(math.atan2(air_x,-air_z)),heading)
            self.assertEqual(vy,0)

    def test_full_fuel_requires_quantity_and_mass_stability_with_advancing_time(self):
        samples=[{'sim_time':t,'right_usg':28,'left_usg':27.98-.003*t,'right_volume_usg':29.2,
                  'right_kg':78.024,'left_kg':77.97-.008*t} for t in range(4)]
        self.assertTrue(full_fuel_stable(samples))
        self.assertFalse(full_fuel_stable(samples[:3]))
        self.assertFalse(full_fuel_stable([dict(s,sim_time=1) for s in samples]))
        self.assertFalse(full_fuel_stable([dict(s,right_usg=25) for s in samples]))
        self.assertFalse(full_fuel_stable([dict(s,right_volume_usg=27) for s in samples]))
        self.assertFalse(full_fuel_stable([dict(s,right_kg=78.9-.03*s['sim_time']) for s in samples]))

    def test_measured_loading_keeps_payload_envelope_and_drift_checks(self):
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory);(path/'card.json').write_text('{"schema_version":1}')
            adapter=object.__new__(NativeAdapter);adapter.resolved=resolve(path/'card.json')
            adapter.card_directory=path;adapter.api=Mock()
            adapter.mass_setup={'pilot_kg':90.718474,'copilot_kg':90.718474,
                                'left_rear_kg':0,'right_rear_kg':0,'baggage_kg':0}
            adapter.api.get_scalar.return_value=2947/2.20462262185
            adapter._bind_measured_loading()
            self.assertAlmostEqual(adapter.resolved['parameters']['mass_target_lb'],2947)
            self.assertEqual(adapter.resolved['parameters']['mass_tolerance_lb'],3)
            self.assertEqual(json.loads((path/'effective-config.json').read_text())['parameters'],adapter.resolved['parameters'])
            adapter.api.get_scalar.return_value=2960/2.20462262185
            with self.assertRaisesRegex(RuntimeError,'envelope'):adapter._bind_measured_loading()
            adapter.api.get_scalar.return_value=2947/2.20462262185
            adapter.mass_setup['baggage_kg']=1
            with self.assertRaisesRegex(RuntimeError,'payload'):adapter._bind_measured_loading()

    def test_delayed_path_write_retries_with_a_bounded_failure(self):
        adapter=object.__new__(NativeAdapter);adapter.api=Mock();adapter._set_pause=Mock()
        adapter.api.get_raw.side_effect=[[0]*20,[0]*20,[1]+[0]*19]
        with patch('xpt.adapter.time.sleep'),patch('xpt.adapter.time.monotonic',side_effect=[0,0,1,2]):
            self.assertEqual(adapter._hold_setup_path(),3)
        self.assertEqual(adapter.api.set_dataref.call_count,3)
        adapter.api.get_raw.side_effect=None;adapter.api.get_raw.return_value=[0]*20
        with patch('xpt.adapter.time.sleep'),patch('xpt.adapter.time.monotonic',side_effect=[0,0,6]):
            with self.assertRaisesRegex(RuntimeError,'five seconds'):adapter._hold_setup_path()

class ConfigurationTests(unittest.TestCase):
    def config(self,value):
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'card.json';path.write_text(json.dumps(value))
            return resolve(path)
    def test_reject_unknown_and_string_numbers(self):
        for value in [{'schema_version':1,'parameters':{'final_kais':80}},
                      {'schema_version':1,'parameters':{'final_kias':'80'}},
                      {'schema_version':1,'parameters':{'capture_blend_full_deg':30}},
                      {'schema_version':1,'repeats':True}]:
            with self.assertRaises(ValueError):self.config(value)
    def test_complete_native_roundtrip(self):
        resolved=self.config({'schema_version':1})
        readback={k:float(v) for k,v in (line.split('=') for line in native_text(resolved['parameters']).splitlines())}
        self.assertEqual(readback,resolved['parameters'])
        self.assertNotIn('configuration',resolved)
    def test_protocol_preserves_v1_wire_layout(self):
        header=(REPO/'crates/poweroff180/tests/fixtures/protocol-v1-header.csv').read_text().strip()
        self.assertEqual(header.split(','),FIELDS)
        self.assertEqual(len(FIELDS),75)

class StatusTests(unittest.TestCase):
    def test_loopback_snapshot_and_no_path_traversal(self):
        with tempfile.TemporaryDirectory() as directory:
            status=Status(Path(directory));url=status.start_server()
            try:
                status.update('final',card='calm-01',native={'ias_kias':80})
                data=json.load(urllib.request.urlopen(url+'/status'))
                self.assertEqual(data['phase'],'final');self.assertFalse(data['stale'])
                with self.assertRaises(urllib.error.HTTPError):urllib.request.urlopen(url+'/../session.json')
                self.assertEqual(json.loads((Path(directory)/'status.json').read_text())['card'],'calm-01')
            finally:status.close()

class ComparisonTests(unittest.TestCase):
    def test_interpolated_repeat_divergence(self):
        a=[{'elapsed':t,'ias_kias':80,'pitch_deg':2,'runway_cross_ft':0,'physical_sink_fpm':100} for t in range(4)]
        b=[dict(row,ias_kias=85) for row in a]
        result=divergence([a,b]);self.assertEqual(result['ias_kias']['maximum_spread'],5)
        self.assertEqual(len(result['flags']),1)

    def test_aborted_flight_is_not_a_valid_landing_measurement(self):
        # An abort after a valid power cut still cannot qualify as a landing trial.
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'card.json';path.write_text('{"schema_version":1}')
            config=resolve(path)
        row={field:0.0 for field in FIELDS}
        row.update(sim_time=2,cut_sim_time=1,entry_gate_s=3,phase_id=10,
                   control_dt_s=.03,mass_kg=config['parameters']['mass_target_lb']/2.20462262185)
        result=assess({'effective_config':config,'terminal':{'phase':'aborted','reason':'supervisor_lost','native_steps':10}},[row])
        self.assertFalse(result['measurement_valid']);self.assertFalse(result['passed'])
        self.assertIn('Native abort: supervisor_lost',result['reasons'])


    def test_touchdown_speed_and_roundout_height_have_upper_limits(self):
        from xpt.config import ACCEPTANCE
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'card.json';path.write_text('{"schema_version":1}')
            config=resolve(path)
        base={field:0.0 for field in FIELDS}
        base.update(cut_sim_time=1,entry_gate_s=3,control_dt_s=.03,
                    mass_kg=config['parameters']['mass_target_lb']/2.20462262185)
        cut=dict(base,sim_time=1,phase_id=3,roundout_sim_time=-1)
        flare=dict(base,sim_time=2,phase_id=7,roundout_sim_time=2,agl_ft=35,runway_along_ft=400)
        contact=dict(base,sim_time=5,phase_id=8,roundout_sim_time=2,contact_latched=1,
                     first_sim_time=5,last_airborne_sim_time=4.97,first_along_ft=1100,
                     last_airborne_along_ft=1097,first_physical_fpm=-100,first_kias=65)
        terminal={'phase':'complete','reason':'none','native_steps':100,
                  'post_contact_max_agl_ft':1,'post_contact_max_g':1.3}
        def result(speed,height=35):
            return assess({'effective_config':config,'terminal':terminal},
                          [cut,dict(flare,agl_ft=height),dict(contact,first_kias=speed)])
        for speed in [63,65,67]:self.assertTrue(result(speed)['passed'])
        for speed in [62.9,67.1]:self.assertIn('Touchdown speed',result(speed)['reasons'])
        self.assertIn('Roundout height',result(65,35.1)['reasons'])

if __name__=='__main__':unittest.main()
