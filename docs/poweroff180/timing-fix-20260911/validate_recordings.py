"""Check the recording-enabled test profile and retain event frames."""
import argparse
import csv
import json
import subprocess
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("run", type=Path)
parser.add_argument("--ffmpeg-bin", type=Path, required=True)
args = parser.parse_args()
run, bin_dir = args.run, args.ffmpeg_bin
ffmpeg, ffprobe = bin_dir / "ffmpeg.exe", bin_dir / "ffprobe.exe"
results = []
fmod_lines = [line for line in (run / "Log.txt").read_text(errors="replace").splitlines() if " I/FMOD:" in line]
assert any("Master bank Aircraft/X-Aviation/TorqueSim SR20/fmod/" in line for line in fmod_lines)
for card in sorted((run / "cards").iterdir()):
    assessment = json.loads((card / "assessment.json").read_text())
    capture = json.loads((card / "capture.json").read_text())
    audio = json.loads((card / "audio.metadata.json").read_text())
    endpoints = json.loads((card / "audio-endpoint-validation.json").read_text())
    assert endpoints and all(e["active_endpoint_matches_recorder"] and e["recorder_loopback"] == audio["loopback"] for e in endpoints)
    assert assessment["measurement_valid"] and assessment["passed"]
    assert capture["errors"] == [] and capture["audio_exit_code"] == 0
    assert audio["peak_linear"] > .001 and audio["rms_linear"] > .0001
    assert audio["speaker"] == audio["loopback"]
    assert audio["started_epoch"] <= capture["movie_start_epoch"]
    assert audio["stopped_epoch"] >= capture["movie_stop_epoch"]
    wav = card / "audio.wav"
    audio_info = json.loads(subprocess.check_output([str(ffprobe), "-v", "error", "-show_format", "-show_streams", "-of", "json", str(wav)], text=True))
    audio_stream = next(s for s in audio_info["streams"] if s["codec_type"] == "audio")
    assert int(audio_stream["sample_rate"]) == audio["sample_rate"] and audio_stream["channels"] == audio["channels"]
    assert abs(float(audio_info["format"]["duration"]) - audio["duration_seconds"]) < .001
    audio_decode = subprocess.run([str(ffmpeg), "-nostdin", "-v", "error", "-i", str(wav), "-f", "null", "-"], capture_output=True, text=True)
    assert audio_decode.returncode == 0 and not audio_decode.stderr.strip(), audio_decode.stderr
    for name, value in capture["sound_readback"].items():
        assert abs(value - (.6 if name.endswith("master_volume_ratio") else 1)) < .02
    movies = []
    for path in map(Path, capture["native_movies"]):
        info = json.loads(subprocess.check_output([str(ffprobe), "-v", "error", "-show_format", "-show_streams", "-of", "json", str(path)], text=True))
        video = next(s for s in info["streams"] if s["codec_type"] == "video")
        duration = float(info["format"]["duration"])
        assert duration > 0 and video["width"] >= 1000
        decoded = subprocess.run([str(ffmpeg), "-nostdin", "-v", "error", "-i", str(path), "-f", "null", "-"], capture_output=True, text=True)
        assert decoded.returncode == 0 and not decoded.stderr.strip(), decoded.stderr
        movies.append({"path": str(path), "duration_seconds": duration, "width": video["width"], "height": video["height"], "codec": video["codec_name"], "full_decode_passed": True})
    assert movies
    total = sum(m["duration_seconds"] for m in movies)
    sim_span = capture["movie_stop_sim_time"] - capture["movie_start_sim_time"]
    assert sim_span > 0
    with (card / "trace.csv").open() as stream:
        contact = next(float(r["first_sim_time"]) for r in csv.DictReader(stream) if float(r["contact_latched"]) == 1)
    frames = []
    for label, offset in [("before", -1), ("contact", 0), ("after", 1)]:
        sim_time = contact + offset
        video_time = (sim_time - capture["movie_start_sim_time"]) / sim_span * total
        assert 0 < video_time < total
        cursor = video_time
        for movie in movies:
            if cursor < movie["duration_seconds"]:
                output = card / ("event-" + label + ".jpg")
                subprocess.run([str(ffmpeg), "-nostdin", "-v", "error", "-y", "-ss", str(cursor), "-i", movie["path"], "-frames:v", "1", "-q:v", "2", "-update", "1", str(output)], check=True)
                frames.append({"file": output.name, "estimated_sim_time": sim_time, "video_time": video_time})
                break
            cursor -= movie["duration_seconds"]
    result = {"card": card.name, "landing_passed": True, "movies": movies, "audio": audio,
              "audio_endpoint_verified": True, "fmod_log_lines": fmod_lines,
              "raw_audio_full_decode_passed": True,
              "audio_covers_movie": True, "sound_readback_passed": True, "event_frames": frames,
              "event_mapping": "Linear map of native movie duration to recorded simulation start/stop times; inspect visible event state separately."}
    (card / "recording-validation.json").write_text(json.dumps(result, indent=2) + "\n")
    results.append(result)
(run / "recording-validation.json").write_text(json.dumps(results, indent=2) + "\n")
print(json.dumps(results, indent=2))
