import tempfile
import unittest
from pathlib import Path

from prepare import prepare


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


if __name__ == "__main__":
    unittest.main()
