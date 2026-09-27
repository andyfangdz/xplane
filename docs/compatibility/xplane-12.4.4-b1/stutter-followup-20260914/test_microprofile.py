import tempfile
import unittest
from pathlib import Path
from parse_microprofile import parse

class CaptureTests(unittest.TestCase):
    def test_scope_crosses_frame_boundary_and_truncated_scope_is_ignored(self):
        text='''var ThreadNames = ['Main Thread'];
TimerInfo[0] = MakeTimer(0, "Work", 0, '#000','#000', 0, 0, 0, 0, 0, 0, 0, 0, [],[],[]);
var tt0 = [[0,1]];
var ts0 = [MakeTimes(1e-4,[0,10000])];
var ti0 = [[0,0]];
Frames[0] = MakeFrame(0, 0, 2, 0, 0, ts0, tt0, ti0, []);
var tt1 = [[0]];
var ts1 = [MakeTimes(1e-4,[40000])];
var ti1 = [[0]];
Frames[1] = MakeFrame(0, 2, 5, 0, 0, ts1, tt1, ti1, []);
'''
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'capture.html';path.write_text(text)
            result=parse(path)
        self.assertEqual(result['scope_count'],1)
        self.assertEqual(result['longest_main_scopes'][0]['duration_ms'],3)
        self.assertEqual([f['main_scopes'][0]['overlap_ms'] for f in result['frames']],[1,2])

    def test_untrusted_capture_javascript_is_not_executed(self):
        text="var ThreadNames = [];\nvar Frames = Array(0);\nthrow new Error('must not execute');"
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'capture.html';path.write_text(text)
            self.assertEqual(parse(path)['scope_count'],0)

if __name__=='__main__':unittest.main()
