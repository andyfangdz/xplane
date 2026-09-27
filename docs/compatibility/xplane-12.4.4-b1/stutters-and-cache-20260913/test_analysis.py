"""Check event grouping, exposure, boundaries and sequence diagnostics on known data."""
import csv
import json
import tempfile
import unittest
from pathlib import Path
import analyze

class TimingAnalysisTests(unittest.TestCase):
    def fixture(self,path):
        fields=['wall_elapsed_s','wall_frame_dt_s','attitude_frame_dt_s','flight_loop_counter','native_steps']
        intervals=[0,.01,.05,.06,.12,.02,.08]
        elapsed=0
        with (path/'trace.csv').open('w',newline='') as f:
            writer=csv.DictWriter(f,fieldnames=fields);writer.writeheader()
            for i,dt in enumerate(intervals):
                elapsed+=dt
                writer.writerow(dict(zip(fields,[elapsed,dt,min(dt,.05025),100+i*2,i])))
        (path/'assessment.json').write_text(json.dumps({'passed':True,'measurement_valid':True,'reasons':[]}))

    def test_known_pause_counts_exposure_and_dispatch_counter(self):
        with tempfile.TemporaryDirectory() as name:
            path=Path(name);self.fixture(path);result=analyze.analyze(path)
        self.assertEqual(result['frames'],6)
        self.assertEqual(result['zero_dt_samples'],1)
        self.assertAlmostEqual(result['observed_wall_seconds'],.34)
        self.assertEqual(result['over_50_ms'],3)
        self.assertEqual(result['events_over_50_ms'],2)
        self.assertEqual(result['over_100_ms'],1)
        self.assertAlmostEqual(result['average_fps'],6/.34)
        self.assertAlmostEqual(result['excess_seconds_above_50_ms'],.11)
        self.assertEqual(result['sdk_counter_increments'],{2:6})
        self.assertEqual(result['native_step_gaps'],0)
        self.assertTrue(result['dt_matches_wall_timestamp'])

    def test_detects_missing_native_sample_and_inconsistent_timestamp(self):
        with tempfile.TemporaryDirectory() as name:
            path=Path(name);self.fixture(path)
            with (path/'trace.csv').open(newline='') as f:rows=list(csv.DictReader(f))
            rows.pop(3)
            with (path/'trace.csv').open('w',newline='') as f:
                writer=csv.DictWriter(f,fieldnames=list(rows[0]));writer.writeheader();writer.writerows(rows)
            result=analyze.analyze(path)
        self.assertEqual(result['native_step_gaps'],1)
        self.assertFalse(result['dt_matches_wall_timestamp'])

if __name__=='__main__':unittest.main()
