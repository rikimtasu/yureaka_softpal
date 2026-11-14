"""Create a private, writable game root without linking writable game data."""
import argparse
import json
from pathlib import Path
import shutil
import stat

WORKSPACE = Path(__file__).resolve().parents[1]


def prepare(source: Path, destination: Path) -> None:
    source = source.resolve(strict=True)
    destination = destination.resolve()
    if not (source / "data.pac").is_file() or not (source / "yureaka.exe").is_file():
        raise ValueError("Select the Yureaka installation containing data.pac and yureaka.exe")
    if destination == source or source in destination.parents or destination in source.parents:
        raise ValueError("The isolated root must be separate from the installation")
    if destination.exists():
        raise FileExistsError(f"Refusing to overwrite {destination}; choose a new destination")
    destination.mkdir(parents=True)
    # Copy rather than hardlink: even accidental resource writes cannot touch the source.
    files = list(source.glob("*.pac")) + [source / "yureaka.exe", source / "dll" / "Pal.dll"]
    for directory in ("movie", "fonts"):
        base = source / directory
        if base.is_dir():
            files.extend(p for p in base.rglob("*") if p.is_file())
    for file in files:
        if file.is_symlink() or getattr(file.lstat(), "st_file_attributes", 0) & stat.FILE_ATTRIBUTE_REPARSE_POINT:
            raise ValueError(f"Refusing linked input: {file}")
        relative = file.relative_to(source)
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        print(f"Copying {relative}", flush=True)
        shutil.copyfile(file, target)
    (destination / "save").mkdir()
    print(f"Isolated game ready: {destination}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("game", type=Path)
    parser.add_argument("--destination", type=Path, default=WORKSPACE / "work" / "game")
    args = parser.parse_args()
    prepare(args.game, args.destination)
    config_path = WORKSPACE / "yureaka.local.json"
    if not config_path.exists():
        config_path.write_text(json.dumps({"game_root": str(args.destination.resolve()),
                                          "font_file": "C:/Windows/Fonts/msgothic.ttc",
                                          "width": 1920, "height": 1080,
                                          "borderless_fullscreen": True,
                                          "integer_scaling": True,
                                          "render_scale": 2}, indent=2), encoding="utf-8")


if __name__ == "__main__":
    main()
