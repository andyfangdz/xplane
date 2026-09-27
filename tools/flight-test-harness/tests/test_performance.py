import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock
from xpt.perf_config import resolve,segments
from xpt.perf_report import stats,present_windows,assess,build,angular_span
from xpt.readiness import check_startup,SimulatorStartupError,startup_menu_visible
from xpt.perf_matrix import build as build_matrix

class PerformanceTests(unittest.TestCase):
    def config(self,value):
        with tempfile.TemporaryDirectory() as d:
            p=Path(d)/'config.json';p.write_text(json.dumps(value));return resolve(p)
    def test_counterbalanced_order_preserves_repetition_identity(self):
        s=list(segments(self.config({})))
        self.assertEqual([(v['repeat'],v['record']) for v in s],[(1,False),(1,True),(2,True),(2,False),(3,False),(3,True)])
    def test_rejects_measurement_ambiguity(self):
        for value in [{'repeats':True},{'wpr':'false'},{'duration_seconds':5},{'duration_seconds':float('nan')},
                      {'typo':1},{'variants':[{'name':'same','record':False},{'name':'same','record':True}]}]:
            with self.assertRaises(ValueError,msg=str(value)):self.config(value)
    def test_integer_qpc_boundaries_and_swapchains(self):
        lo=2**54
        rows=[dict(ProcessID='42',CPUStartQPC=str(lo+i),SwapChainAddress='main') for i in [-1,0,1,2,5]]
        rows+=[dict(ProcessID='7',CPUStartQPC=str(lo+1),SwapChainAddress='main'),
               dict(ProcessID='42',CPUStartQPC=str(lo+1),SwapChainAddress='menu')]
        window,chains=present_windows(rows,{'measure_start_qpc':lo,'end_qpc':lo+5},42)
        self.assertEqual([t-lo for t,_ in window],[0,1,2]);self.assertEqual(chains,{'main':3,'menu':1})
    def test_tail_statistics_do_not_hide_spike_in_average(self):
        s=stats([10]*99+[100]);self.assertEqual(s['over_50ms'],1);self.assertEqual(s['max_ms'],100)
        self.assertAlmostEqual(s['mean_ms'],10.9);self.assertEqual(stats([0,float('nan')]),None)
    def test_wrap_at_north_is_not_a_full_camera_sweep(self):
        self.assertAlmostEqual(angular_span([359.9,0,.1]),.2)
        self.assertEqual(angular_span([300,330,0,30,60]),120)
    def test_rejects_truncated_or_event_loss_trace(self):
        m=dict(state=3,invalid_flags=0,overflow=False,frames_seen=2000,frames_stored=0,
               record=False,measure_start_qpc=1000,end_qpc=21000,qpc_frequency=1000,epoch_qpc=1000)
        rows=[dict(ProcessID='42',CPUStartQPC=str(t)) for t in range(1000,21000,10)]
        s={'duration_seconds':20}
        self.assertTrue(assess(s,m,[],rows,42,True,{'status':'Dropped event: 0'},True)['valid'])
        for status in ['Dropped event: 1','unavailable']:
            self.assertFalse(assess(s,m,[],rows,42,True,{'status':status},True)['valid'])
        self.assertFalse(assess(s,m,[],rows[:100],42,False,None,False)['valid'])
    def test_failed_attempt_survives_report(self):
        with tempfile.TemporaryDirectory() as d:
            p=Path(d);(p/'segments/01_buffered').mkdir(parents=True)
            (p/'requested-config.json').write_text('{"repeats":1,"variants":[{"name":"buffered"}]}')
            (p/'segments/01_buffered/error.json').write_text('{"error":"focus lost"}')
            r=build(p);self.assertFalse(r['complete']);self.assertEqual(r['valid_segments'],0)
            self.assertIn('focus lost',(p/'PERFORMANCE.md').read_text())
    def test_matrix_excludes_focus_wait_and_preserves_failed_attempt(self):
        with tempfile.TemporaryDirectory() as d:
            root=Path(d);matrix=root/'matrix';matrix.mkdir();attempts=[]
            for variant in (0,1):
                run=root/f'run{variant}';run.mkdir()
                attempts.append(dict(run=run.name,repeat=1,variant=variant,exit_code=0))
                (run/'performance-report.json').write_text(json.dumps({'complete':True,'variants':{'buffered':{'median_mean_ms':10+variant}}}))
                (run/'load-timing.json').write_text(json.dumps({'seconds_to_paused_plugin_ready':30+variant,'seconds_to_gated_scene':100+50*variant,'focus_wait_seconds':70+49*variant}))
            (matrix/'matrix.json').write_text(json.dumps({'compare':'Flag','expected_runs':2,'attempts':attempts}))
            result=build_matrix(matrix)
            self.assertTrue(result['complete']);self.assertEqual(result['paired_differences'][0]['load_seconds_B_minus_A'],1)
            attempts[1]['exit_code']=1
            (matrix/'matrix.json').write_text(json.dumps({'compare':'Flag','expected_runs':2,'attempts':attempts}))
            result=build_matrix(matrix)
            self.assertFalse(result['complete']);self.assertEqual(len(result['attempts']),2);self.assertEqual(result['paired_differences'],[])
    def test_fatal_preview_error_stops_startup_without_ui_toggle(self):
        with tempfile.TemporaryDirectory() as d:
            (Path(d)/'Log.txt').write_text('E/SYS: MACIBM_alert: Error generating path for aircraft preview')
            with self.assertRaises(SimulatorStartupError):check_startup(d)
        api=Mock();api.datarefs={};self.assertFalse(startup_menu_visible(api));api.command.assert_not_called()
    def test_safe_mode_cannot_be_counted_as_requested_profile(self):
        with tempfile.TemporaryDirectory() as d:
            (Path(d)/'Log.txt').write_text('0:07:53.962 I/SYS: ** SAFE MODE **\n0:07:53.962 I/SYS: ** Limiting: Graphics, Aircraft, Scenery, Plugins, Art Controls **')
            with self.assertRaisesRegex(SimulatorStartupError,'SAFE MODE'):check_startup(d)

if __name__=='__main__':unittest.main()
