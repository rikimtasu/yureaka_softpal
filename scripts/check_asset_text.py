"""Replay the copied save with the selected font without touching player saves."""
import os
from pathlib import Path
import shutil
import subprocess
import sys
from datetime import datetime
from portable_config import load_config, game_path, font_options, engine_path

ROOT = Path(__file__).resolve().parents[1]


def main():
    config = load_config()
    source = game_path(config)
    game = ROOT / "work" / ("asset-text-check-" + datetime.now().strftime("%Y%m%d-%H%M%S"))
    game.mkdir()
    for file in source.rglob("*"):
        if not file.is_file():
            continue
        rel = file.relative_to(source)
        target = game / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        if rel.parts[0].lower() == "save":
            shutil.copy2(file, target)
        else:
            os.link(file, target)
    options = [str(engine_path()), str(game), *font_options(config),
               "--headless", "--no-audio",
               "--diagnostic-device-size", "1920x1080", "--diagnostic-frames", "7600",
               "--diagnostic-click", "3000:280:250",
               "--diagnostic-png-at", f"3300:{game / 'load-menu.png'}",
               "--diagnostic-click", "3500:200:200",
               "--diagnostic-click", "3700:550:380",
               "--diagnostic-png-at", f"3999:{game / 'loaded-scene.png'}",
               "--diagnostic-png-at", f"7599:{game / 'final.png'}"]
    for index, frame in enumerate(range(4200, 7400, 240)):
        options += ["--diagnostic-click", f"{frame}:600:598",
                    "--diagnostic-png-at", f"{frame + 200}:{game / f'line-{index:02}.png'}"]
    log = game / "runtime.log"
    with log.open("wb") as stream:
        subprocess.run(options, cwd=ROOT, env=dict(os.environ, RUST_LOG="info,pal_vm::runtime=debug"),
                       stdout=stream, stderr=stream, check=True)
    subprocess.run([sys.executable, str(ROOT / "scripts/check_diagnostic.py"), str(log)], check=True)
    text = log.read_text(encoding="utf-8", errors="replace")
    if "[trace-save] load slot=0" not in text:
        raise RuntimeError("Replay did not load the copied save")
    print(game)


if __name__ == "__main__":
    main()
