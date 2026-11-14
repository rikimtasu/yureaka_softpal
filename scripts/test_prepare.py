import tempfile
import unittest
import json
import os
import shutil
from unittest.mock import patch
from pathlib import Path

from prepare import prepare
from portable_config import load_config, save_config, game_path, font_options, engine_path


class PrepareTests(unittest.TestCase):
    def test_resources_are_independent_and_original_saves_are_not_imported(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            source, target = root / "original", root / "isolated"
            (source / "dll").mkdir(parents=True)
            (source / "save").mkdir()
            for name in ("data.pac", "csv.pac", "yureaka.exe", "dll/Pal.dll", "save/system.dat"):
                (source / name).write_bytes(b"original")
            prepare(source, target)
            (target / "data.pac").write_bytes(b"changed")
            self.assertEqual((source / "data.pac").read_bytes(), b"original")
            self.assertEqual(list((target / "save").iterdir()), [])
            with self.assertRaises(FileExistsError):
                prepare(source, target)
            with self.assertRaises(ValueError):
                prepare(source, source / "nested")

    def test_missing_installation_is_rejected_without_creating_output(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            with self.assertRaises(ValueError):
                prepare(root, root / "output")
            self.assertFalse((root / "output").exists())

    def test_relative_settings_survive_a_move_and_an_unrelated_cwd(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            before, after = root / "before", root / "moved port"
            (before / "work/game").mkdir(parents=True)
            (before / "fonts").mkdir()
            (before / "fonts/custom.ttf").write_bytes(b"font")
            (before / "bin").mkdir()
            (before / "bin/sena.exe").write_bytes(b"engine")
            (before / "yureaka.local.json").write_text(json.dumps({
                "game_root": "work/game", "font_file": "fonts/custom.ttf"}), encoding="utf-8")
            shutil.move(before, after)
            config = load_config(after)
            self.assertEqual(game_path(config, after), after / "work/game")
            self.assertEqual(font_options(config, after), ["--font-file", str(after / "fonts/custom.ttf")])
            self.assertEqual(engine_path(after), after / "bin/sena.exe")

    def test_legacy_paths_and_missing_fonts_recover_without_writing_settings(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "work/game-copy").mkdir(parents=True)
            (root / "windows/Fonts").mkdir(parents=True)
            (root / "windows/Fonts/msgothic.ttc").write_bytes(b"font")
            settings = json.dumps({"game_root": "Z:/old/port/work/game-copy", "font_file": "missing.ttf"})
            (root / "yureaka.local.json").write_text(settings, encoding="utf-8")
            with patch.dict(os.environ, WINDIR=str(root / "windows")):
                config = load_config(root)
                self.assertEqual(game_path(config, root), root / "work/game-copy")
                self.assertEqual(font_options(config, root)[1], str(root / "windows/Fonts/msgothic.ttc"))
            self.assertEqual((root / "yureaka.local.json").read_text(encoding="utf-8"), settings)

    def test_saving_settings_normalizes_paths_and_keeps_preferences(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            root = base / "port"
            (root / "work/game").mkdir(parents=True)
            font = base / "external.ttf"
            font.write_bytes(b"custom font")
            config = load_config(root)
            config.update(game_root=str(root / "work/game"), font_file=str(font), width=1600)
            save_config(config, root)
            stored = load_config(root)
            self.assertEqual(stored["game_root"], "work/game")
            self.assertTrue(stored["font_file"].startswith("fonts/"))
            self.assertEqual(stored["width"], 1600)
            self.assertEqual(Path(font_options(stored, root)[1]).read_bytes(), font.read_bytes())


if __name__ == "__main__":
    unittest.main()
