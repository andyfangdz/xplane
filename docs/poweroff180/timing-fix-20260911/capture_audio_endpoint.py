"""Read the dedicated simulator's Windows audio endpoints during capture.

Requires pycaw 20251023, comtypes 1.4.16 and psutil 7.2.2. --deps can point
at a separate pip --target directory without changing the harness environment.
This script reads Core Audio sessions; it never changes devices or volume.
"""
import argparse
from datetime import datetime
import json
from pathlib import Path
import sys
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("run", type=Path)
parser.add_argument("--deps", type=Path)
args = parser.parse_args()
if args.deps:
    sys.path.insert(0, str(args.deps))
import psutil
from pycaw.api.audiopolicy import IAudioSessionControl2
from pycaw.utils import AudioUtilities

session = json.loads((args.run / "session.json").read_text(encoding="utf-8-sig"))
assert session["record_video"]
identity = session["simulator"]
process = psutil.Process(identity["pid"])
assert Path(process.exe()).resolve() == Path(identity["executable"]).resolve()
assert abs(process.create_time() - datetime.fromisoformat(identity["start_time"]).timestamp()) < .1
status = json.loads((args.run / "status.json").read_text())
card = args.run / "cards" / status["card"]
assert status["phase"] in ("downwind", "delay", "turn_to_base", "base", "turn_to_final", "final", "rollout")
capture = json.loads((card / "capture.json").read_text())
endpoint = capture["audio_ready"]["loopback"]
matches = []
for device in AudioUtilities.GetAllDevices(data_flow=0, device_state=1):
    sessions = device.AudioSessionManager.GetSessionEnumerator()
    for index in range(sessions.GetCount()):
        control = sessions.GetSession(index).QueryInterface(IAudioSessionControl2)
        if control.GetProcessId() == identity["pid"]:
            matches.append({"device": device.FriendlyName, "state": control.GetState()})
result = {"observed_epoch": time.time(), "simulator_identity": identity,
          "card": status["card"], "phase": status["phase"], "recorder_loopback": endpoint,
          "simulator_audio_sessions": matches,
          "active_endpoint_matches_recorder": any(m["device"] == endpoint and m["state"] == 1 for m in matches),
          "method": "Read-only Windows Core Audio session enumeration, matched to verified simulator PID, path and start time."}
output = card / "audio-endpoint-validation.json"
history = json.loads(output.read_text()) if output.exists() else []
history.append(result)
output.write_text(json.dumps(history, indent=2) + "\n")
print(json.dumps(result, indent=2))
assert result["active_endpoint_matches_recorder"]
