"""Regenerate the checked-in 64-bit bindings using bindgen-cli 0.72.1."""
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent
HEADERS = ROOT / "SDK/CHeaders"
VERSIONS = (200, 210, 300, 301, 302, 303, 400, 410, 420, 430, 440)


def main():
    with tempfile.TemporaryDirectory() as directory:
        wrapper = Path(directory) / "sdk.h"
        wrapper.write_text("".join(
            f'#include "{p.name}"\n'
            for group in ("XPLM", "Widgets")
            for p in sorted((HEADERS / group).glob("*.h"))
        ), encoding="ascii")
        bindings = Path(directory) / "bindings.rs"
        # LIN selects plain C declarations. The platform macros only control
        # import/export attributes; the 64-bit SDK data layouts are shared.
        subprocess.run([
            "bindgen", str(wrapper), "--allowlist-file", r".*[/\\](XPLM|Widgets)[/\\].*\.h",
            "--no-doc-comments", "--rust-target", "1.85", "--rust-edition", "2024",
            "--output", str(bindings), "--",
            "--target=x86_64-pc-windows-msvc", "-ffreestanding", "-DLIN=1",
            *(f"-DXPLM{version}=1" for version in VERSIONS),
            f"-I{HEADERS / 'XPLM'}", f"-I{HEADERS / 'Widgets'}",
        ], check=True)
        (ROOT / "src/bindings.rs").write_bytes(bindings.read_bytes())


if __name__ == "__main__":
    main()
