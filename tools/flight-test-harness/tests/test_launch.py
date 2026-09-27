import os
from pathlib import Path
import tempfile
import threading
import time
import unittest
from xpt.config import atomic_json
from xpt.launch import wait_for_launch


class LaunchTests(unittest.TestCase):
    @unittest.skipUnless(os.name == 'nt', 'Windows file-sharing regression')
    def test_record_replacement_waits_for_reader(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'status.json'
            atomic_json(path, {'revision': 1})
            reader = path.open('rb')
            timer = threading.Timer(.15, reader.close)
            timer.start()
            started = time.monotonic()
            try:
                atomic_json(path, {'revision': 2})
                self.assertGreaterEqual(time.monotonic() - started, .1)
                self.assertIn('"revision": 2', path.read_text())
                self.assertEqual(list(path.parent.glob('*.tmp')), [])
            finally:
                timer.join()
                reader.close()

    @unittest.skipUnless(os.name == 'nt', 'Windows file-sharing regression')
    def test_blocked_record_preserves_previous_value(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'status.json'
            atomic_json(path, {'revision': 1})
            with path.open('rb'):
                with self.assertRaises(PermissionError):
                    atomic_json(path, {'revision': 2})
                self.assertIn('"revision": 1', path.read_text())
                self.assertEqual(list(path.parent.glob('*.tmp')), [])

    def test_waits_for_recoverable_worker(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp)
            session = {'simulator': {'pid': 10}, 'worker': None}
            atomic_json(directory / 'session.json', session)
            expected = dict(session, worker={'pid': 20})
            timer = threading.Timer(.1, atomic_json, (directory / 'session.json', expected))
            timer.start()
            try:
                self.assertEqual(wait_for_launch(directory, 2), expected)
            finally:
                timer.join()

    def test_unpublished_worker_times_out(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp)
            atomic_json(directory / 'session.json', {'simulator': {'pid': 10}, 'worker': None})
            with self.assertRaisesRegex(TimeoutError, 'API was not opened'):
                wait_for_launch(directory, .05)

    def test_restored_run_cannot_restart_worker(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp)
            atomic_json(directory / 'session.json', {'simulator': {'pid': 10}, 'worker': {'pid': 20}})
            atomic_json(directory / 'restoration.json', {'restored': True})
            with self.assertRaisesRegex(RuntimeError, 'cancelled'):
                wait_for_launch(directory)
