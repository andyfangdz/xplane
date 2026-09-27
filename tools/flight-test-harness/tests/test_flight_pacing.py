import unittest
import tempfile
from pathlib import Path
from xpt.config import atomic_json
from xpt.flight_pacing import assess_flight,interval_summary,build_matrix


def flight(spikes=None,pause_at=None):
    spikes=spikes or {};qpc=1000000;rows=[]
    for i in range(16004):
        dt=spikes.get(i,.01);qpc+=round(dt*1000000)
        rows.append({'qpc':str(qpc),'wall_frame_dt_s':str(dt),
                     'paused':int(i<2 or i>=16002 or i==pause_at),'vr':0,'foreground':1})
    m={'state':3,'invalid_flags':0,'overflow':False,'sweep':False,'record':True,
       'qpc_frequency':1000000,'frames_seen':len(rows),'frames_stored':len(rows)}
    return m,rows


class FlightPacingTests(unittest.TestCase):
    def test_activation_is_retained_separately_from_midflight_hitch(self):
        m,rows=flight({100:.2,10200:.08})
        r=assess_flight(m,rows,{'measurement_valid':True,'passed':False})
        self.assertTrue(r['valid'],r['reasons'])
        self.assertFalse(r['landing_passed'])
        self.assertEqual(r['full_flight']['over_50ms'],2)
        self.assertEqual(r['activation_first_5_seconds']['over_50ms'],1)
        self.assertEqual(r['primary']['over_50ms'],1)
        self.assertGreater(r['primary']['exposure_seconds'],150)
        self.assertEqual(r['primary']['local_spikes'],1)

    def test_pause_focus_and_missing_tail_invalidate_flight(self):
        m,rows=flight(pause_at=8000)
        self.assertFalse(assess_flight(m,rows,{'measurement_valid':True})['valid'])
        m,rows=flight();rows[7000]['foreground']=0
        self.assertFalse(assess_flight(m,rows,{'measurement_valid':True})['valid'])
        m,rows=flight();rows[-1]['paused']=0
        self.assertFalse(assess_flight(m,rows,{'measurement_valid':True})['valid'])

    def test_corrupt_delta_or_truncated_buffer_cannot_pass(self):
        m,rows=flight();rows[600]['wall_frame_dt_s']='.02'
        self.assertFalse(assess_flight(m,rows,{'measurement_valid':True})['valid'])
        m,rows=flight();m['frames_seen']+=1
        self.assertFalse(assess_flight(m,rows,{'measurement_valid':True})['valid'])

    def test_rates_use_exposure_and_smaller_spikes_are_visible(self):
        rows=[{'interval_ms':10} for _ in range(99)]+[{'interval_ms':30}]
        r=interval_summary(rows)
        self.assertEqual(r['over_25ms'],1);self.assertEqual(r['over_50ms'],0)
        self.assertAlmostEqual(r['over_25ms_per_minute'],60/1.02)
        self.assertGreater(r['p999_ms'],r['p99_ms'])

    def test_matrix_requires_reproduction_across_processes_and_same_sources(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp);matrix=root/'matrix';matrix.mkdir();attempts=[]
            for pair in (1,2):
                for flag in (False,True):
                    run=root/f'p{pair}_{flag}';run.mkdir()
                    attempts.append({'run':run.name,'pair':pair,'flag':flag,'exit_code':0})
                    s=interval_summary([{'interval_ms':10}]*99+[{'interval_ms':80 if not flag else 10}])
                    atomic_json(run/'flight-pacing.json',{'complete':True,'settings':{'no_defragment':flag,'texture_cache_gb':10},'flights':[{'card':'calm-01','valid':True,'primary':s}]})
                    atomic_json(run/'source-manifest.json',{'harness_sha256':{'a':'same'}})
            atomic_json(matrix/'matrix.json',{'expected_runs':4,'attempts':attempts})
            r=build_matrix(matrix)
            self.assertTrue(r['complete']);self.assertTrue(r['baseline_reproduced'])
            self.assertLess(r['paired_differences'][0]['over_50ms_per_minute_on_minus_off'],0)
            atomic_json(run/'source-manifest.json',{'harness_sha256':{'a':'changed'}})
            self.assertFalse(build_matrix(matrix)['complete'])
