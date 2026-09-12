"""Launch the real native binary on a virtual display and capture its window.

Run under xvfb-run. Requires the locally available Pillow and xwininfo tools;
neither is a dependency of the shipped Rust application.
"""

import argparse
import os
import re
from pathlib import Path
import shutil
import subprocess
import tempfile
import time

from PIL import ImageGrab


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--screenshot", type=Path, default=Path("screenshots/native-desktop.png"))
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    binary = args.binary.resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix="codex-ui-smoke-") as temporary:
        home = Path(temporary) / "codex-home"
        shutil.copytree(root / "tests/fixtures/codex-home", home)
        environment = dict(os.environ, CODEX_HOME=str(home))
        with tempfile.TemporaryFile() as log:
            process = subprocess.Popen([str(binary)], env=environment, stdout=log, stderr=log)
            try:
                deadline = time.monotonic() + 20
                while time.monotonic() < deadline:
                    if process.poll() is not None:
                        raise RuntimeError(f"Native application exited early: {process.returncode}")
                    windows = subprocess.run(
                        ["xwininfo", "-root", "-tree"],
                        capture_output=True, text=True, check=True, timeout=3,
                    ).stdout
                    window = re.search(r'^\s*(0x[0-9a-f]+)\s+"Codex 配置助手"', windows, re.MULTILINE)
                    if window:
                        break
                    time.sleep(0.2)
                else:
                    raise RuntimeError("Native application did not map a window within 20 seconds")
                time.sleep(4.5)
                if process.poll() is not None:
                    raise RuntimeError("Native application exited before capture")
                geometry = subprocess.run(
                    ["xwininfo", "-id", window.group(1)],
                    capture_output=True, text=True, check=True, timeout=3,
                ).stdout
                def dimension(label):
                    match = re.search(rf"{label}:\s+(-?\d+)", geometry)
                    if not match:
                        raise RuntimeError(f"Missing native window geometry: {label}")
                    return int(match.group(1))
                x, y = dimension("Absolute upper-left X"), dimension("Absolute upper-left Y")
                width, height = dimension("Width"), dimension("Height")
                image = ImageGrab.grab(
                    bbox=(x, y, x + width, y + height), xdisplay=environment["DISPLAY"],
                )
                args.screenshot.parent.mkdir(parents=True, exist_ok=True)
                image.save(args.screenshot)
                print(f"PASS: real native window mapped and remained running; screenshot: {args.screenshot}")
            finally:
                if process.poll() is None:
                    process.terminate()
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait(timeout=5)
                log.seek(0)
                output = log.read().decode(errors="replace")
                if output:
                    print(output)


if __name__ == "__main__":
    main()
