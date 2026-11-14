"""Resolve local settings from the port folder, including old absolute workspace paths."""
import json
import hashlib
import os
from pathlib import Path
import re
import shutil

ROOT = Path(__file__).resolve().parents[1]


def resolve_path(value: str, root: Path = ROOT) -> Path:
    value = re.sub(r"%([^%]+)%", lambda match: os.environ.get(match[1], match[0]), value)
    path = Path(value)
    return (path if path.is_absolute() else root / path).resolve()


def load_config(root: Path = ROOT) -> dict:
    config = {"game_root": "work/game", "font_file": "%WINDIR%/Fonts/msgothic.ttc",
              "width": 1920, "height": 1080, "borderless_fullscreen": True,
              "integer_scaling": True, "render_scale": 2, "debug_mode": False}
    path = root / "yureaka.local.json"
    if path.exists():
        config.update(json.loads(path.read_text(encoding="utf-8-sig")))
    if not resolve_path(config["game_root"], root).exists():
        old = config["game_root"].replace("\\", "/")
        marker = old.lower().rfind("/work/")
        if marker >= 0 and resolve_path(old[marker + 1:], root).exists():
            config["game_root"] = old[marker + 1:]
    font = config.get("font_file", "")
    if font and not resolve_path(font, root).is_file():
        config["font_file"] = "%WINDIR%/Fonts/msgothic.ttc" if resolve_path("%WINDIR%/Fonts/msgothic.ttc", root).is_file() else ""
    return config


def game_path(config: dict, root: Path = ROOT) -> Path:
    game = resolve_path(config["game_root"], root)
    if (root / "work").resolve() not in game.parents:
        raise ValueError("Validation must use private resources inside the port/work folder")
    return game.resolve(strict=True)


def font_options(config: dict, root: Path = ROOT) -> list[str]:
    return ["--font-file", str(resolve_path(config["font_file"], root))] if config.get("font_file") else []


def save_config(config: dict, root: Path = ROOT) -> None:
    config = dict(config)
    config["game_root"] = game_path(config, root).relative_to(root.resolve()).as_posix()
    font = config.get("font_file", "")
    if font:
        path = resolve_path(font, root)
        bases = [(root, ""), *[(Path(os.environ[name]) / relative, f"%{name}%/{relative}/")
                               for name, relative in (("WINDIR", "Fonts"), ("LOCALAPPDATA", "Microsoft/Windows/Fonts"))
                               if os.environ.get(name)]]
        for base, token in bases:
            try:
                config["font_file"] = token + path.relative_to(base.resolve()).as_posix()
                break
            except ValueError:
                continue
        else:
            directory = root / "fonts"
            directory.mkdir(exist_ok=True)
            digest = hashlib.sha256(path.read_bytes()).hexdigest()
            target = directory / (digest + path.suffix.lower())
            if not target.exists():
                shutil.copyfile(path, target)
            config["font_file"] = target.relative_to(root).as_posix()
    (root / "yureaka.local.json").write_text(json.dumps(config, indent=2), encoding="utf-8")


def engine_path(root: Path = ROOT) -> Path:
    for relative in ("bin/sena.exe", "engine/target/release/sena.exe"):
        candidate = root / relative
        if candidate.is_file():
            return candidate
    raise FileNotFoundError("Engine missing: use the portable ZIP or run scripts/build.ps1")
