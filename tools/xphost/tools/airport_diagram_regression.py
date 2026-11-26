"""Exercise the submitted map, labels and projected markers in the built G1000 example."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
SCENARIO = ROOT / "tests/airport-diagram"


def require(ok, message):
    if not ok:
        raise ValueError(message)


def mfd(directory, name):
    frame = json.loads((directory / f"{name}.json").read_text())
    require(not frame["errors"], f"{name}: {frame['errors']}")
    return next(surface["commands"] for surface in frame["surfaces"]
                if surface.get("device") == "7" and surface.get("phase") == "after")


def check_map(commands):
    require(commands and commands[0]["op"] == "map", "expected native map before symbol overlay")
    view = commands[0]["args"]
    require(view["layers"] == 296 and view["orientation"] == 0 and view["range_nm"] == 1.25
            and view["rect"] == [190, 672, 1004, 88] and view["center"] == [597, 380]
            and abs(view["datum"][0] - 47.53) < 0.00001 and abs(view["datum"][1] + 122.3) < 0.00001,
            "incorrect airport, layers, orientation, range or viewport")
    labels = [c["args"]["text"] for c in commands if c["op"] == "text"]
    require("AIRPORT DIAGRAM  KBFI" in labels and "NORTH UP   1.25 NM" in labels,
            "nearest-airport identification or scale missing")
    # A white ring at the airport, distinct from the cyan ownship dot NW of it.
    ring = [xy for c in commands if c["op"] == "quadstrip" for xy, color in c["args"]["points"]
            if color == 0xffffffff]
    require(ring and all(6 < ((x - 597)**2 + (y - 380)**2)**0.5 < 13 for x, y in ring),
            "white airport marker missed the datum")
    vertices = [xy for c in commands if c["op"] == "atlas_mesh" and c["args"]["color"] == 0xffffff00
                for xy, uv in c["args"]["geometry"]]
    require(vertices, "missing cyan ownship marker")
    center = [(min(axis) + max(axis))/2 for axis in zip(*vertices)]
    require(abs(center[0] - 578.1) < 0.5 and abs(center[1] - 394.0) < 0.5,
            f"unexpected ownship position {center}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plugin", required=True, type=Path, help="compiled airport_diagram.dll")
    parser.add_argument("--font", required=True, type=Path, help="installed ProFontWindows.ttf")
    parser.add_argument("--out", required=True, type=Path, help="new evidence directory")
    args = parser.parse_args()
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    package = out / "package"
    plugin = package / "plugins/airport-diagram/64/win.xpl"
    plugin.parent.mkdir(parents=True)
    shutil.copyfile(args.plugin, plugin)
    shutil.copyfile(ROOT / "tests/fixture/fixture.acf", package / "fixture.acf")
    capture = out / "host"
    fonts = capture / "xplane/Resources/fonts"
    fonts.mkdir(parents=True)
    shutil.copyfile(args.font, fonts / "ProFontWindows.ttf")
    catalog = out / "datarefs.txt"
    catalog.write_text("sim/cockpit/electrical/avionics_on\tint\ty\n", encoding="ascii")
    command = [str(ROOT / "build/xphost.exe"), "--pkg", str(package), "--acf", "fixture.acf",
               "--xpl", str(plugin), "--clock", "fast", "--lat", "47.531", "--lon", "-122.302",
               "--datarefs", str(catalog), "--navdat", str(SCENARIO / "nav.dat"),
               "--script", str(SCENARIO / "overlay.xps"), "--out", str(capture)]
    with (out / "run.log").open("wb") as log:
        subprocess.run(command, check=True, stdout=log, stderr=subprocess.STDOUT,
                       timeout=30, creationflags=subprocess.CREATE_NO_WINDOW)
    for name in ("stock", "dark", "closed"):
        require(not mfd(capture, name), f"{name}: overlay remained on the MFD")
    require(all(c["op"] != "map" for c in mfd(capture, "loading")), "new map should start loading")
    for name in ("active", "restored"):
        check_map(mfd(capture, name))
    digest = hashlib.sha256(plugin.read_bytes()).hexdigest()
    (out / "result.json").write_text(json.dumps({"passed": True, "plugin_sha256": digest}, indent=2))
    print(f"Airport diagram passed for {digest}. Captures: {capture}")


if __name__ == "__main__":
    main()
