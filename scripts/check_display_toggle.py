"""Check F11 on a dedicated diagnostic window without sending global input."""
import ctypes
from ctypes import wintypes
from pathlib import Path
import subprocess
import time
import os

ROOT = Path(__file__).resolve().parents[1]
USER = ctypes.WinDLL("user32", use_last_error=True)
USER.FindWindowW.argtypes = [wintypes.LPCWSTR, wintypes.LPCWSTR]
USER.FindWindowW.restype = wintypes.HWND
USER.GetClientRect.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.RECT)]
USER.PostMessageW.argtypes = [wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM]


def wait_for(check):
    deadline = time.monotonic() + 12
    while time.monotonic() < deadline:
        result = check()
        if result:
            return result
        time.sleep(0.1)
    raise AssertionError("Display state did not reach the expected value")


def main():
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument("game", type=Path)
    parser.add_argument("--render-scale", type=int, default=2, choices=range(1, 5))
    args = parser.parse_args()
    game = args.game.resolve(strict=True)
    if ROOT / "work" not in game.parents or game.name == "game-copy":
        raise ValueError("Use an isolated diagnostic root, not the player resource copy")
    title = "Yureaka display option check"
    log = game / "display-toggle.log"
    with log.open("wb") as stream:
        process = subprocess.Popen([
            str(ROOT / "engine/target/release/sena.exe"), str(game),
            "--no-audio", "--title", title, "--width", "1280", "--height", "720",
            "--physical-window-size", "--ignore-config-window-size",
            "--borderless-fullscreen", "--render-scale", str(args.render_scale),
            "--trace-input",
        ], stdout=stream, stderr=stream, env=dict(os.environ, RUST_LOG="info"))
        window = None
        try:
            window = wait_for(lambda: USER.FindWindowW(None, title))

            def size():
                rect = wintypes.RECT()
                if not USER.GetClientRect(window, ctypes.byref(rect)):
                    raise ctypes.WinError(ctypes.get_last_error())
                return rect.right - rect.left, rect.bottom - rect.top

            fullscreen = wait_for(lambda: size() if size()[0] > 1280 and size()[1] >= 720 else None)
            print(f"Initial fullscreen: {fullscreen}", flush=True)

            def f11():
                # F11 scan code 0x57; target only this process's window.
                USER.PostMessageW(window, 0x0100, 0x7A, 0x00570001)
                USER.PostMessageW(window, 0x0101, 0x7A, 0xC0570001)

            f11()
            wait_for(lambda: size() == (1280, 720))
            print(f"Windowed after F11: {size()}", flush=True)
            f11()
            wait_for(lambda: size() == fullscreen)
            print(f"PASS: fullscreen {fullscreen[0]}x{fullscreen[1]} -> F11 windowed 1280x720 -> F11 fullscreen")
        finally:
            if window:
                USER.PostMessageW(window, 0x0010, 0, 0)
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                process.terminate()
                process.wait(timeout=5)


if __name__ == "__main__":
    main()
