"""Create a private, writable game root without linking writable game data."""
import argparse
from pathlib import Path
import shutil
import stat
from portable_config import load_config, resolve_path, save_config

WORKSPACE = Path(__file__).resolve().parents[1]


def prepare(source: Path, destination: Path) -> None:
    for path in (source.absolute(), destination.absolute().parent):
        for ancestor in [path, *path.parents]:
            if ancestor.exists() and (ancestor.is_symlink() or getattr(ancestor.lstat(), "st_file_attributes", 0) & stat.FILE_ATTRIBUTE_REPARSE_POINT):
                raise ValueError(f"Refusing linked directory: {ancestor}")
    source = source.resolve(strict=True)
    destination = destination.resolve()
    if not all((source / name).is_file() for name in ("data.pac", "yureaka.exe", "dll/Pal.dll")):
        raise ValueError("Select the Yureaka installation containing data.pac, yureaka.exe and dll/Pal.dll")
    if destination == source or source in destination.parents or destination in source.parents:
        raise ValueError("The isolated root must be separate from the installation")
    if destination.exists():
        raise FileExistsError(f"Refusing to overwrite {destination}; choose a new destination")
    # Copy rather than hardlink: even accidental resource writes cannot touch the source.
    files = list(source.glob("*.pac")) + [source / "yureaka.exe", source / "dll" / "Pal.dll"]
    for directory in ("dll", "movie", "fonts"):
        base = source / directory
        if base.is_dir():
            for path in [base, *base.rglob("*")]:
                if path.is_symlink() or getattr(path.lstat(), "st_file_attributes", 0) & stat.FILE_ATTRIBUTE_REPARSE_POINT:
                    raise ValueError(f"Refusing linked input: {path}")
                if path.is_file() and directory != "dll":
                    files.append(path)
    for file in files:
        if file.is_symlink() or getattr(file.lstat(), "st_file_attributes", 0) & stat.FILE_ATTRIBUTE_REPARSE_POINT:
            raise ValueError(f"Refusing linked input: {file}")
    destination.mkdir(parents=True)
    for file in files:
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
    # Destination paths are always port-relative, even when invoked from elsewhere.
    destination = resolve_path(str(args.destination), WORKSPACE)
    if WORKSPACE / "work" not in destination.parents:
        parser.error("Destination must be inside the port/work folder")
    prepare(args.game, destination)
    config = load_config(WORKSPACE)
    config["game_root"] = destination.relative_to(WORKSPACE).as_posix()
    save_config(config, WORKSPACE)


if __name__ == "__main__":
    main()
