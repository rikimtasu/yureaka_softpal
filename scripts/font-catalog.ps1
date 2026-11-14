# Read the display name of the same first face that the engine selects. Reading
# metadata avoids installing or loading native font code just to populate the UI.
if (-not ('Yureaka.FontNames' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.IO;
using System.Text;

namespace Yureaka {
    public static class FontNames {
        static int U16(byte[] b, int i) {
            if (i < 0 || i > b.Length - 2) throw new InvalidDataException();
            return (b[i] << 8) | b[i + 1];
        }
        static uint U32(byte[] b, int i) {
            return ((uint)U16(b, i) << 16) | (uint)U16(b, i + 2);
        }
        static byte[] ReadAt(FileStream f, long position, int count) {
            if (position < 0 || count < 0 || position > f.Length - count) throw new InvalidDataException();
            f.Position = position;
            byte[] b = new byte[count];
            for (int n = 0; n < count;) {
                int read = f.Read(b, n, count - n);
                if (read == 0) throw new EndOfStreamException();
                n += read;
            }
            return b;
        }
        public static string Read(string path, int language) {
            try {
                using (var f = File.OpenRead(path)) {
                    byte[] header = ReadAt(f, 0, 16);
                    long face = 0;
                    if (U32(header, 0) == 0x74746366) {
                        if (U32(header, 8) == 0) return null;
                        face = U32(header, 12); // First face of a TrueType collection.
                    }
                    header = ReadAt(f, face, 12);
                    uint signature = U32(header, 0);
                    if (signature != 0x10000 && signature != 0x4f54544f && signature != 0x74727565) return null;
                    int tables = U16(header, 4);
                    if (tables > 4096) return null;
                    byte[] directory = ReadAt(f, face + 12, tables * 16);
                    for (int t = 0; t < tables; t++) {
                        int d = t * 16;
                        if (U32(directory, d) != 0x6e616d65) continue;
                        uint length = U32(directory, d + 12);
                        if (length > 1024 * 1024) return null;
                        byte[] names = ReadAt(f, U32(directory, d + 8), (int)length);
                        int count = U16(names, 2), strings = U16(names, 4);
                        if (6 + count * 12 > names.Length) return null;
                        string best = null;
                        int bestScore = -1;
                        for (int n = 0; n < count; n++) {
                            int r = 6 + n * 12;
                            int platform = U16(names, r), lang = U16(names, r + 4), id = U16(names, r + 6);
                            if ((id != 4 && id != 1) || (platform != 0 && platform != 3)) continue;
                            int size = U16(names, r + 8), start = strings + U16(names, r + 10);
                            if ((size & 1) != 0 || start < strings || start > names.Length - size) continue;
                            int score = (id == 4 ? 100 : 0) + (lang == language ? 30 : lang == 0x409 ? 20 : platform == 0 ? 10 : 0);
                            string value = Encoding.BigEndianUnicode.GetString(names, start, size).Trim('\0', ' ');
                            if (value.Length == 0 || value.IndexOf('\0') >= 0) continue;
                            if (score > bestScore) { best = value; bestScore = score; }
                        }
                        return best;
                    }
                }
            } catch (InvalidDataException) { } catch (IOException) { } catch (UnauthorizedAccessException) { } catch (ArgumentException) { }
            return null;
        }
    }
}
'@
}

function New-FontChoice([string]$Path) {
    $fullPath = [IO.Path]::GetFullPath($Path)
    $name = [Yureaka.FontNames]::Read($fullPath, [Globalization.CultureInfo]::CurrentUICulture.LCID)
    if (-not $name) { $name = [IO.Path]::GetFileNameWithoutExtension($fullPath) }
    [pscustomobject]@{ Name = $name; Path = $fullPath }
}

function Select-FontPath($ComboBox, [string]$Path) {
    if (-not $Path) { $ComboBox.SelectedIndex = 0; return }
    $fullPath = [IO.Path]::GetFullPath($Path)
    for ($i = 1; $i -lt $ComboBox.Items.Count; $i++) {
        if ($ComboBox.Items[$i].Path -eq $fullPath) { $ComboBox.SelectedIndex = $i; return }
    }
    $ComboBox.SelectedIndex = $ComboBox.Items.Add((New-FontChoice $fullPath))
}
