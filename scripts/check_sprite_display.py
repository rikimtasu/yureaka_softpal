"""Replay a selected save in an isolated resource root for sprite diagnostics."""
import argparse
from datetime import datetime
import os
from pathlib import Path
import shutil
import subprocess
import sys
from portable_config import load_config, game_path, font_options, engine_path

ROOT = Path(__file__).resolve().parents[1]


def verify_reported_scenes(game):
    from PIL import Image

    momiji = Image.open(game / "reported-momiji.png").convert("RGB")
    rgb = momiji.crop((1500, 0, 1900, 450)).tobytes()
    arena = list(zip(rgb[0::3], rgb[1::3], rgb[2::3]))
    black = sum(max(pixel) < 8 for pixel in arena) / len(arena)
    if black > 0.05:
        raise AssertionError(f"Arena background is missing: {black:.1%} of the sampled region is black")
    rgb = momiji.crop((100, 100, 400, 400)).tobytes()
    portrait = list(zip(rgb[0::3], rgb[1::3], rgb[2::3]))
    bright = sum(min(pixel) > 215 for pixel in portrait) / len(portrait)
    if bright > 0.05:
        raise AssertionError("Face portrait still appears over the upper-left background")
    shizuku_path = game / "reported-shizuku.png"
    if not shizuku_path.exists():
        # The first 13,300-frame baseline used its final frame for this scene.
        shizuku_path = game / "final.png"
    shizuku = Image.open(shizuku_path).convert("RGB")
    r, g, b = shizuku.getpixel((960, 5))
    if not (g > 150 and b > 150 and g > r):
        raise AssertionError("Standing sprite is still enlarged/clipped at the top edge")
    print("Screenshot checks: arena visible; no upper-left portrait; standing sprite clears top edge")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--slot", type=int, default=1, choices=range(12))
    parser.add_argument("--frames", type=int, default=6000)
    parser.add_argument("--interval", type=int, default=240)
    parser.add_argument("--window", action="store_true")
    parser.add_argument("--trace-sprites", action="store_true")
    parser.add_argument("--integer-scaling", action="store_true")
    parser.add_argument("--render-scale", type=int, choices=range(1, 5))
    parser.add_argument("--check-existing", type=Path)
    args = parser.parse_args()
    if args.check_existing:
        verify_reported_scenes(args.check_existing)
        return
    config = load_config()
    source = game_path(config)
    game = ROOT / "work" / ("sprite-display-check-" + datetime.now().strftime("%Y%m%d-%H%M%S-%f"))
    game.mkdir()
    for file in source.rglob("*"):
        if not file.is_file():
            continue
        relative = file.relative_to(source)
        target = game / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        if relative.parts[0].lower() == "save":
            shutil.copy2(file, target)
        else:
            os.link(file, target)
    x = 200 + 320 * (args.slot % 3)
    y = 200 + 129 * (args.slot // 3)
    options = [str(engine_path()), str(game),
               "--no-audio", "--trace-assets",
               "--diagnostic-device-size", "1920x1080", "--diagnostic-frames", str(args.frames),
               "--diagnostic-click", "3000:280:250",
               "--diagnostic-click", f"3500:{x}:{y}",
               "--diagnostic-click", "3700:550:380",
               "--diagnostic-png-at", f"3999:{game / 'loaded-scene.png'}",
               "--diagnostic-png-at", f"{args.frames - 1}:{game / 'final.png'}"]
    if args.window:
        options += ["--width", "1920", "--height", "1080", "--physical-window-size",
                    "--ignore-config-window-size", "--fixed-timestep-ms", "16"]
        options = ["--window-dump-frame-at" if item == "--diagnostic-png-at" else item for item in options]
    else:
        options += ["--headless"]
    options += font_options(config)
    if args.integer_scaling:
        options += ["--integer-scaling"]
    if args.render_scale:
        options += ["--render-scale", str(args.render_scale)]
    for index, frame in enumerate(range(4200, args.frames - args.interval, args.interval)):
        options += ["--diagnostic-click", f"{frame}:600:598"]
        if index % 10 == 0:
            options += ["--window-dump-frame-at" if args.window else "--diagnostic-png-at",
                        f"{frame + args.interval - 2}:{game / f'line-{index:03}.png'}"]
    for frame, name in [(4559, "reported-ruby"), (6778, "reported-momiji"), (13259, "reported-shizuku"),
                        (38699, "reported-novel-paragraphs"), (39719, "reported-novel-tree")]:
        if frame < args.frames:
            options += ["--window-dump-frame-at" if args.window else "--diagnostic-png-at",
                        f"{frame}:{game / (name + '.png')}"]
    log = game / "runtime.log"
    with log.open("wb") as stream:
        log_filter = "info,pal_vm::runtime=debug"
        if args.trace_sprites:
            log_filter += ",pal_vm::sprite=debug"
        subprocess.run(options, cwd=ROOT, env=dict(os.environ, RUST_LOG=log_filter),
                       stdout=stream, stderr=stream, check=True)
    subprocess.run([sys.executable, str(ROOT / "scripts/check_diagnostic.py"), str(log)], check=True)
    text = log.read_text(encoding="utf-8", errors="replace")
    if f"[trace-save] load slot={args.slot}" not in text:
        raise RuntimeError(f"Replay did not load slot {args.slot}; inspect {log}")
    if args.slot == 1 and args.interval == 60 and args.frames >= 13300 and not args.integer_scaling:
        verify_reported_scenes(game)
    print(game)


if __name__ == "__main__":
    main()
