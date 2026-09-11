"""Read-only loopback status, atomic snapshots, and structured event history."""
import json
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from .config import atomic_json

PAGE = '''<!doctype html><html lang="en"><meta charset="utf-8"><title>X-Plane test status</title>
<style>body{font:17px system-ui;max-width:950px;margin:50px auto;padding:0 24px;background:#f7f7f3;color:#182d35}h1{font-size:32px}pre{white-space:pre-wrap;background:white;border:1px solid #cbd4d7;padding:24px;border-radius:8px}#phase{font-size:24px}small{color:#526870}</style>
<h1>X-Plane test session</h1><p id="phase">Connecting…</p><small id="age"></small><pre id="data"></pre>
<script>async function refresh(){try{const r=await fetch('/status',{cache:'no-store'}),s=await r.json();document.getElementById('phase').textContent=s.phase+' · '+(s.card||'session');document.getElementById('age').textContent='Updated '+s.age_seconds.toFixed(1)+' seconds ago'+(s.stale?' — status is stale':'');document.getElementById('data').textContent=JSON.stringify(s,null,2)}catch(e){document.getElementById('age').textContent='Connection closed. Final results are saved in the run directory.'}}refresh();setInterval(refresh,1000)</script></html>'''

class Status:
    def __init__(self, directory):
        self.directory = directory
        self.lock = threading.Lock()
        self.value = {'phase': 'starting', 'updated_epoch': time.time(), 'revision': 0}
        self.events = (directory / 'events.jsonl').open('a', encoding='utf-8', buffering=1)
        self.server = None

    def update(self, phase=None, **values):
        with self.lock:
            previous = (self.value.get('phase'), self.value.get('card'))
            self.value.update(values)
            if phase:
                self.value['phase'] = phase
            self.value.update(updated_epoch=time.time(), revision=self.value['revision'] + 1)
            atomic_json(self.directory / 'status.json', self.value)
            if previous != (self.value.get('phase'), self.value.get('card')) or values.get('event'):
                self.events.write(json.dumps(self.value, allow_nan=False) + '\n')

    def snapshot(self):
        with self.lock:
            value = dict(self.value)
        value['age_seconds'] = max(0, time.time() - value['updated_epoch'])
        value['stale'] = value['age_seconds'] > 5 and value['phase'] in ('downwind','delay','turn_to_base','base','turn_to_final','final','rollout')
        return value

    def start_server(self):
        owner = self
        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *_):
                pass
            def do_GET(self):
                if self.path == '/':
                    data = PAGE.encode(); mime = 'text/html; charset=utf-8'
                elif self.path == '/status':
                    data = json.dumps(owner.snapshot()).encode(); mime = 'application/json'
                elif self.path == '/events':
                    data = (owner.directory / 'events.jsonl').read_bytes(); mime = 'application/x-ndjson'
                else:
                    self.send_error(404); return
                self.send_response(200); self.send_header('Content-Type', mime)
                self.send_header('Cache-Control', 'no-store'); self.send_header('Content-Length', str(len(data)))
                self.end_headers(); self.wfile.write(data)
        self.server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        self.server.daemon_threads = True
        threading.Thread(target=self.server.serve_forever, daemon=True).start()
        url = f'http://127.0.0.1:{self.server.server_port}'
        self.update(status_url=url)
        return url

    def close(self):
        if self.server:
            self.server.shutdown(); self.server.server_close()
        self.events.close()
