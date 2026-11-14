"""Run independent font and letterbox checks against the private resource copy."""
import os
from pathlib import Path
import subprocess
import sys
from portable_config import load_config, game_path, resolve_path, engine_path

ROOT = Path(__file__).resolve().parents[1]


def main():
    config = load_config()
    source = game_path(config)
    for tag, font, size, extra in [
        ("mincho", "%WINDIR%/Fonts/msmincho.ttc", "1920x1080", []),
        ("letterbox", "%WINDIR%/Fonts/msgothic.ttc", "1920x1200", []),
        ("backlog", "%WINDIR%/Fonts/msgothic.ttc", "1920x1080", ["--diagnostic-wheel", "9100:1"]),
        ("menu", "%WINDIR%/Fonts/msgothic.ttc", "1920x1080", ["--diagnostic-click", "9100:1250:700"]),
    ]:
        if len(sys.argv) > 1 and tag not in sys.argv[1:]:
            continue
        font = resolve_path(font)
        if not font.is_file():
            raise FileNotFoundError(f"Install a Japanese font before running this font comparison: {font}")
        game = ROOT / "work" / f"smoke-{tag}"
        suffix = 2
        while game.exists():
            game = ROOT / "work" / f"smoke-{tag}-{suffix}"
            suffix += 1
        game.mkdir()  # Preserve earlier runs and their saves.
        for file in source.rglob("*"):
            rel = file.relative_to(source)
            if not file.is_file() or rel.parts[0].lower() == "save":
                continue
            dest = game / rel
            dest.parent.mkdir(parents=True, exist_ok=True)
            os.link(file, dest)  # Both endpoints are workspace-owned copies.
        (game / "save").mkdir()
        options = [str(engine_path()), str(game),
                   "--headless", "--no-audio", "--font-file", str(font),
                   "--diagnostic-device-size", size, "--diagnostic-frames", "9200",
                   "--diagnostic-click", "3000:280:184",
                   "--diagnostic-png-at", f"8999:work/diag/{tag}-adv.png",
                   "--diagnostic-png-at", f"9199:work/diag/{tag}-final.png"]
        # Stop advancing before the backlog opens.
        for frame in range(3500, 9000, 90):
            options += ["--diagnostic-click", f"{frame}:640:640"]
        options += extra
        env = dict(os.environ, RUST_LOG="info")
        log = ROOT / "work" / f"smoke-{tag}.log"
        with log.open("wb") as stream:
            subprocess.run(options, cwd=ROOT, env=env, stdout=stream, stderr=stream, check=True)
        subprocess.run([sys.executable, str(ROOT / "scripts/check_diagnostic.py"), str(log)], check=True)
        print(f"{tag}: checked, images under work/diag", flush=True)


if __name__ == "__main__":
    main()
