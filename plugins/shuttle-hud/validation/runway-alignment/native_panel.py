"""Measure the cockpit combiner and fullscreen overlay in one stationary desktop view per mode."""

import argparse
import base64
import csv
import hashlib
import json
import math
from pathlib import Path
import shutil
import sys
import time

import numpy as np
from PIL import Image

REPOSITORY = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(REPOSITORY / "tools/flight-test-harness/vendor"))
from flight_test.api import XPlaneApi, XPlaneApiError

HUD = "fsim_hud/"
POWER = "sim/cockpit2/switches/HUD_on"
LIGHT = "sim/cockpit2/electrical/HUD_brightness_ratio"
VIEW = "sim/graphics/view/view_type"
PAUSE = "sim/time/paused"
CASES = ("cockpit", "fullscreen", "poweroff180", "position", "xgs")


def prepare(args):
    """Add the renderer fixture only to a newly generated, disposable aircraft."""
    if not args.aircraft.is_relative_to(args.root / "Output/ShuttleHUD-install"):
        raise ValueError("The capture fixture belongs in a stage-only derivative")
    folder = args.aircraft / "plugins"
    fixture = folder / "xlua/scripts/xpt_visual_fixture"
    fixture.mkdir(exist_ok=False)
    with (REPOSITORY / "crates/poweroff180/snapshot.csv").open() as file:
        count = sum(1 for _ in csv.DictReader(file))
    (fixture / "xpt_visual_fixture.lua").write_text(
        f'XPT_snapshot = create_dataref("xpt/snapshot","array[{count}]",function() end)\n',
        encoding="ascii")
    config = folder / "XPTNativeGuidance"
    config.mkdir(exist_ok=False)
    with (REPOSITORY / "crates/poweroff180/parameters.csv").open() as file:
        (config / "effective-card.ini").write_text(
            "".join(f"{r['name']}={r['default']}\n" for r in csv.DictReader(file)), encoding="ascii")
    # Approach geometry from the installed KEDW 22L definition in runways.csv.
    with (folder / "ShuttleHUD/runways.csv").open() as file:
        row = next(r for r in csv.reader(file) if "KEDW" in r[0] and "22L" in r[0])
    lat, lon, end_lat, end_lon, _, displacement, field_elevation = map(float, row[1:])
    cos_lat = math.cos(math.radians((lat + end_lat) * .5))
    north, east = (end_lat-lat)*111120, (end_lon-lon)*111120*cos_lat
    length = math.hypot(north, east)
    start = -8500 + displacement
    flight = {
        "aircraft": {"path": args.relative_acf},
        "ai_aircraft": [],
        "lle_air_start": {"latitude": lat+start*north/length/111120, "longitude": lon+start*east/length/(111120*cos_lat),
                          "elevation_in_meters": field_elevation + 6500*.3048, "heading_true": math.degrees(math.atan2(east, north)) % 360,
                          "pitch_in_degrees": -12, "speed_in_meters_per_second": 173},
        "engine_status": {"all_engines": {"running": False}},
        "local_time": {"day_of_year": 252, "time_in_24_hours": 12},
        "weather": {"definition": {"latitude_in_degrees": lat, "longitude_in_degrees": lon,
                   "elevation_in_meters": field_elevation, "visibility_in_kilometers": 50,
                   "temperature_in_degrees_celsius": 15, "altimeter_setting_in_hpa": 1013.25,
                   "precipitation_ratio": 0, "clouds": [],
                   "wind": [{"altitude_in_feet_msl": h, "speed_in_knots": 0, "direction_in_degrees_true": 0,
                             "gust_increase_in_knots": 0, "shear_in_degrees": 0, "turbulence_ratio": 0}
                            for h in (0, 60000)]},
                    "terrain_state": "dry", "variation_across_region_percentage": 0,
                    "vertical_speed_in_thermal_in_feet_per_minute": 0, "wave_height_in_meters": 0,
                    "wave_direction_in_degrees": 0, "evolution_over_time_enum": "static"}
    }
    (args.out / "flight.json").write_text(json.dumps(flight, indent=2), encoding="utf-8")


def capture_poweroff(api, args, evidence):
    with (REPOSITORY / "crates/poweroff180/snapshot.csv").open() as file:
        fields = [r["field"] for r in csv.DictReader(file)]
    data = [0.] * len(fields)
    if len(api.get_raw("xpt/snapshot")) != len(data):
        raise RuntimeError("Test snapshot does not match the production HUD protocol")
    screenshot(api, args.root, args.out / "poweroff180-off.png")
    for name, value in {"configured":1, "run_token":1, "sim_time":36, "phase_id":2, "agl_ft":1000,
                        "ias_kias":100, "groundspeed_kt":99, "runway_along_ft":2000}.items():
        data[fields.index(name)] = value
    try:
        api.set_dataref("xpt/snapshot", data)
        try:
            wait_for(lambda: api.get_scalar("xpt/video_hud/draw_frames") > 4, 10, "power-off 180 frames")
        finally:
            screenshot(api, args.root, args.out / "poweroff180-on.png")
        evidence["captures"]["poweroff180"] = {
            "input": "synthetic configured snapshot on disposable Shuttle",
            "state": api.get_batch(["xpt/video_hud/font_ready", "xpt/video_hud/draw_frames"])
        }
        if not evidence["captures"]["poweroff180"]["state"]["xpt/video_hud/font_ready"]:
            raise RuntimeError("The power-off HUD font did not become ready")
        compare_frames(args, evidence, "poweroff180", "poweroff180-off", "poweroff180-on")
        orientation = poweroff_title_scores(args.out / "poweroff180-on.png")
        evidence["captures"]["poweroff180"].update(orientation)
        if orientation["upright_title_match"] < .65 or orientation["upright_title_match"] < orientation["mirrored_title_match"] + .15:
            raise RuntimeError(f"Power-off HUD title missing or vertically mirrored: {orientation}")
    finally:
        data[fields.index("configured")] = 0
        api.set_dataref("xpt/snapshot", data)


def capture_position(api, args, evidence):
    screenshot(api, args.root, args.out / "position-hidden.png")
    api.command("PositionAircraftNative/toggle_window")
    try:
        time.sleep(.6)
        screenshot(api, args.root, args.out / "position-visible.png")
    finally:
        api.command("PositionAircraftNative/toggle_window")
    compare_frames(args, evidence, "position", "position-hidden", "position-visible")


def compare_frames(args, evidence, name, hidden, shown):
    with Image.open(args.out / f"{hidden}.png") as a, Image.open(args.out / f"{shown}.png") as b:
        delta = np.abs(np.asarray(a.convert("RGB"), dtype=np.int16) - np.asarray(b.convert("RGB"), dtype=np.int16))
    count = int((delta.max(axis=2) > 45).sum())
    evidence["captures"].setdefault(name, {})["changed_pixels"] = count
    if count < 200:
        raise RuntimeError(f"{name} produced only {count} changed desktop pixels")


def poweroff_title_scores(shown):
    """Compare letter shapes to the real SR20 HUD title captured before migration."""
    def title_mask(path):
        with Image.open(path) as frame:
            design = frame.convert("L").resize((1920, 1080), Image.Resampling.LANCZOS)
            mask = np.asarray(design.crop((279, 6, 701, 55))) > 150
        y, x = np.nonzero(mask)
        if not x.size:
            raise RuntimeError(f"Could not find the HUD title at its design coordinates in {path}")
        return mask[y.min():y.max()+1, x.min():x.max()+1]

    reference = title_mask(REPOSITORY / "docs/compatibility/xplane-12.4.4-b1/ui/sr20-hud.png")
    live = Image.fromarray(title_mask(shown).astype("uint8") * 255)
    live = np.asarray(live.resize((reference.shape[1], reference.shape[0]), Image.Resampling.BILINEAR)) > 127
    def overlap(mask):
        return float(2 * np.count_nonzero(reference & mask) / (reference.sum() + mask.sum()))
    return {"upright_title_match": overlap(live), "mirrored_title_match": overlap(np.flipud(live))}


def capture_xgs(api, args, evidence):
    screenshot(api, args.root, args.out / "xgs-hidden.png")
    (args.out / "xgs-ready").write_text("Plugins > Landing Speed Rust > Preview Overlay\n", encoding="ascii")
    # The preview is deliberately a real menu action. The caller signals when
    # selection is complete; drawing the sample in fixture code would bypass XGS.
    wait_for(lambda: (args.out / "xgs-selected").exists(), 180, "XGS preview selected")
    # The user's menu action may take longer than X-Plane's HTTP keepalive.
    # Obtain fresh IDs and a fresh connection before this non-idempotent POST.
    with XPlaneApi(port=args.port, timeout=5) as fresh:
        fresh.refresh_catalogs()
        screenshot(fresh, args.root, args.out / "xgs-preview.png")
    with Image.open(args.out / "xgs-hidden.png") as other, Image.open(args.out / "xgs-preview.png") as preview:
        background = np.asarray(other.convert("RGB"), dtype=np.int16)
        foreground = np.asarray(preview.convert("RGB"), dtype=np.int16)
    # In the same paused view, the translucent black backing darkens the
    # scene and the sample labels add white strokes inside its bounds.
    darkened = (background - foreground).mean(axis=2) > 30
    letters = (foreground.min(axis=2) > 180) & ((foreground - background).mean(axis=2) > 30)
    evidence["captures"]["xgs"] = {"darkened_pixels": int(darkened.sum()), "lighter_text_pixels": int(letters.sum())}
    if darkened.sum() < 200 or letters.sum() < 20:
        raise RuntimeError(f"XGS preview absent or incomplete: {evidence['captures']['xgs']}")


def wait_for(operation, seconds, description):
    deadline = time.monotonic() + seconds
    last = None
    while time.monotonic() < deadline:
        try:
            last = operation()
            if last:
                return last
        except XPlaneApiError as error:
            last = error
        time.sleep(.25)
    raise RuntimeError(f"Timed out waiting for {description}: {last}")


def screenshot(api, root, destination):
    folder = root / "Output/screenshots"
    before = set(folder.glob("*.png"))
    api.command("sim/operation/screenshot")

    def copy_complete():
        fresh = sorted(set(folder.glob("*.png")) - before)
        if len(fresh) > 1:
            raise RuntimeError(f"Ambiguous screenshot: {fresh}")
        if not fresh:
            return False
        try:
            with Image.open(fresh[0]) as image:
                image.load()
        except (OSError, SyntaxError):
            return False
        shutil.copyfile(fresh[0], destination)
        return True

    wait_for(copy_complete, 20, str(destination))


def lit_pixels(off, on):
    """Count newly green HUD strokes. Reject ambient green, exposure changes and identical frames."""
    with Image.open(off) as image:
        dark = np.asarray(image.convert("RGB"), dtype=np.int16)
    with Image.open(on) as image:
        lit = np.asarray(image.convert("RGB"), dtype=np.int16)
    if lit.shape != dark.shape:
        raise ValueError("Viewport changed between captures")
    # Fullscreen uses pale green (historical captures have a green excess
    # of 25). On sunlit pavement the stroke can be darker than the scenery.
    # Require a color change AND new green chroma, not just brighter pixels.
    def chroma(rgb):
        return rgb[:, :, 1] - np.maximum(rgb[:, :, 0], rgb[:, :, 2])
    mask = (chroma(lit) > 12) & (chroma(lit) - chroma(dark) > 12) & (np.abs(lit-dark).max(axis=2) > 18)
    ys, xs = np.where(mask)
    return {
        "viewport": [lit.shape[1], lit.shape[0]],
        "lit_green_pixels": int(len(xs)),
        "bounds": [int(xs.min()), int(ys.min()), int(xs.max()), int(ys.max())] if xs.size else None,
    }


def capture_shuttle(api, args, evidence, mode, view, command, counter):
    if not api.get_scalar(HUD + "aircraft_match"):
        raise RuntimeError("Shuttle HUD does not match the loaded aircraft")
    for ref, value in [(HUD+"enabled", 1), (HUD+"brightness", 1), (HUD+"declutter_mode", 0)]:
        api.set_dataref(ref, value)
    knob = "sim/cockpit2/switches/HUD_brightness_ratio"
    master = "sim/cockpit2/switches/avionics_power_on"
    time_ref = "sim/time/total_flight_time_sec"
    batteries = "sim/cockpit2/electrical/battery_on"
    volts = "sim/cockpit2/electrical/bus_volts"
    start_time = api.get_scalar(time_ref)
    trace = []
    evidence.setdefault("electrical_setup", {})[mode] = trace

    def energized():
        state = api.get_batch([time_ref, PAUSE, batteries, master, volts, knob, POWER, LIGHT])
        trace.append(state)
        if state[PAUSE]:
            api.command("sim/operation/pause_off")
        if not all(state[batteries][:3]):
            # The Orbiter has three batteries in its ACF.
            api.set_dataref(batteries, [1, 1, 1, *state[batteries][3:]])
        if not state[master]:
            api.command("sim/systems/avionics_on")
        if state[knob] < .99:
            api.set_dataref(knob, 1)
        if not state[POWER]:
            api.set_dataref(POWER, 1)
        # Flight startup can publish the aircraft path before its first
        # physics tick. Read back a live bus and actual HUD brightness after
        # that tick before freezing the pose for the optical comparison.
        return (state[time_ref] >= start_time + .5 and state[batteries][0]
                and state[master] and state[volts][0] > 1 and state[LIGHT] > 0)

    # Use actual aircraft switches. Reading HUD_brightness_ratio in the
    # electrical namespace gives the resulting brightness after bus/failures.
    api.command(HUD + command)
    api.command("sim/operation/pause_off")
    try:
        wait_for(energized, 60, "energized aircraft and powered HUD")
    finally:
        api.command("sim/operation/pause_on")
    wait_for(lambda: api.get_scalar(PAUSE), 5, "paused flight")
    api.command(HUD + command)
    wait_for(lambda: api.get_scalar(VIEW) == view, 10, mode)
    api.set_dataref(POWER, 0)
    wait_for(lambda: api.get_scalar(POWER) == 0, 4, "HUD switch off")
    screenshot(api, args.root, args.out / f"{mode}-off.png")
    api.set_dataref(POWER, 1)
    start = api.get_scalar(HUD + counter)
    try:
        wait_for(lambda: api.get_scalar(HUD + counter) >= start + 4, 10, mode + " drawing")
    except RuntimeError:
        # Preserve the rendered image even if the callback failed to run.
        pass
    screenshot(api, args.root, args.out / f"{mode}-on.png")
    state = api.get_batch([HUD + counter, HUD+"active", HUD+"runway_projection_valid", VIEW, PAUSE, POWER, LIGHT])
    pixels = lit_pixels(args.out / f"{mode}-off.png", args.out / f"{mode}-on.png")
    evidence["captures"][mode] = {**pixels, "state": state, "frames_advanced": state[HUD + counter] - start}
    if pixels["lit_green_pixels"] < 30 or state[HUD+"runway_projection_valid"] != 1 or state[VIEW] != view or state[PAUSE] != 1 or state[HUD + counter] < start + 4:
        raise RuntimeError(f"{mode} HUD did not pass: {evidence['captures'][mode]}")


def capture(args):
    binary = args.aircraft / "plugins/ShuttleHUD/64/win.xpl"
    expected = hashlib.sha256(args.binary.read_bytes()).hexdigest()
    if hashlib.sha256(binary.read_bytes()).hexdigest() != expected:
        raise ValueError("Staged aircraft plug-in differs from candidate")
    relative = (args.aircraft / "Orbiter_Glider.acf").relative_to(args.root).as_posix()
    evidence = {"aircraft": relative, "plugin_sha256": expected, "requested": args.cases, "captures": {}, "failures": {}}
    receipt = args.out / "shuttle-panel.json"
    try:
        with XPlaneApi(port=args.port, timeout=5) as api:
            def ready():
                api.refresh_catalogs()
                loaded = base64.b64decode(api.get_raw("sim/aircraft/view/acf_relative_path")).split(b"\0")[0]
                path = loaded.decode().replace("\\", "/")
                if not path:
                    return False
                if path.casefold() == relative.casefold():
                    return path
                # Aircraft-local HUD cases require the exact derivative. X-Plane may
                # explicitly reject it and substitute its default aircraft. Global
                # window captures are still valid on that managed, paused fallback.
                if set(args.cases) <= {"position", "xgs"}:
                    return path
                lines = (args.root / "Log.txt").read_text(encoding="utf-8", errors="replace").splitlines()
                rejected = any("Failed to open the following aircraft" in line and
                               relative.casefold() in lines[i+1].replace("\\", "/").casefold()
                               for i, line in enumerate(lines[:-1]))
                return path if rejected else False
            loaded_path = wait_for(ready, 240, "staged Shuttle or a confirmed aircraft fallback")
            evidence["loaded_aircraft"] = loaded_path
            # Pause the dedicated approach as soon as the loaded aircraft is known.
            api.command("sim/operation/pause_on")
            wait_for(lambda: api.get_scalar(PAUSE), 5, "paused flight")
            if api.get_scalar("sim/graphics/VR/enabled"):
                raise RuntimeError("This check captures the desktop, not VR")
            for case in args.cases:
                try:
                    if case not in ("position", "xgs") and loaded_path.casefold() != relative.casefold():
                        raise RuntimeError(f"Expected staged Shuttle; X-Plane loaded {loaded_path}")
                    api.refresh_catalogs()
                    if case == "cockpit":
                        capture_shuttle(api, args, evidence, case, 1026, "view", "cockpit_draw_frames")
                    elif case == "fullscreen":
                        capture_shuttle(api, args, evidence, case, 1024, "fullscreen", "draw_frames")
                    else:
                        api.set_dataref(POWER, 0)
                        api.command("sim/view/forward_with_nothing")
                        wait_for(lambda: api.get_scalar(VIEW) == 1024, 8, "stable desktop view")
                        {"poweroff180":capture_poweroff, "position":capture_position, "xgs":capture_xgs}[case](api, args, evidence)
                except Exception as error:
                    evidence["failures"][case] = f"{type(error).__name__}: {error}"
                receipt.write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")
            if evidence["failures"]:
                raise RuntimeError(str(evidence["failures"]))
            evidence["passed"] = True
    except Exception as error:
        evidence["session_error"] = f"{type(error).__name__}: {error}"
        raise
    finally:
        receipt.write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("root", "aircraft", "binary", "out"):
        parser.add_argument("--" + name, type=lambda v: Path(v).resolve(strict=True), required=True)
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--prepare", action="store_true")
    parser.add_argument("--relative-acf", help="Path X-Plane will load after staging")
    parser.add_argument("--cases", nargs="+", choices=CASES, default=list(CASES))
    options = parser.parse_args()
    if options.prepare:
        prepare(options)
    else:
        capture(options)
