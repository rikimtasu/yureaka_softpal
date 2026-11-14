$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
. "$PSScriptRoot/font-catalog.ps1"
$temp = Join-Path ([IO.Path]::GetTempPath()) "opencode/yureaka-font-tests-$([guid]::NewGuid())"
[void](New-Item $temp -ItemType Directory)
try {
    $text = [Text.Encoding]::BigEndianUnicode.GetBytes('Readable Typeface')
    $bytes = [byte[]]::new(46 + $text.Length)
    $bytes[1] = 1; $bytes[5] = 1
    [Array]::Copy([Text.Encoding]::ASCII.GetBytes('name'), 0, $bytes, 12, 4)
    $bytes[23] = 28; $bytes[27] = 18 + $text.Length
    $bytes[31] = 1; $bytes[33] = 18
    $bytes[35] = 3; $bytes[37] = 1; $bytes[38] = 4; $bytes[39] = 9
    $bytes[41] = 4; $bytes[43] = $text.Length
    [Array]::Copy($text, 0, $bytes, 46, $text.Length)
    $font = Join-Path $temp 'opaque-filename.ttf'
    [IO.File]::WriteAllBytes($font, $bytes)
    $choice = New-FontChoice $font
    if ($choice.Name -ne 'Readable Typeface') { throw 'Font label did not come from its name table' }
    $collection = [byte[]]::new($bytes.Length + 16)
    [Array]::Copy([Text.Encoding]::ASCII.GetBytes('ttcf'), 0, $collection, 0, 4)
    $collection[5] = 1; $collection[11] = 1; $collection[15] = 16
    [Array]::Copy($bytes, 0, $collection, 16, $bytes.Length)
    $collection[39] = 44 # Absolute name-table offset inside the collection.
    $ttc = Join-Path $temp 'collection.ttc'
    [IO.File]::WriteAllBytes($ttc, $collection)
    if ((New-FontChoice $ttc).Name -ne 'Readable Typeface') { throw 'Collection did not select face 0' }
    $combo = [Windows.Forms.ComboBox]::new()
    $combo.DisplayMember = 'Name'; $combo.ValueMember = 'Path'
    [void]$combo.Items.Add([pscustomobject]@{ Name='Original bitmap font'; Path='' })
    Select-FontPath $combo $font
    if ($combo.SelectedItem.Path -ne $font) { throw 'Display name replaced the stored file path' }
    Select-FontPath $combo $font
    if ($combo.Items.Count -ne 2) { throw 'Selecting the same file duplicated its entry' }
    Select-FontPath $combo ''
    if ($combo.SelectedIndex -ne 0) { throw 'Original bitmap font was not selected' }
    $combo.Dispose()
    [IO.File]::WriteAllBytes($font, [byte[]]@(0,1))
    if ((New-FontChoice $font).Name -ne 'opaque-filename') { throw 'Malformed font did not use filename fallback' }
    'PASS: display names, first collection face, stored paths, duplicate selection, bitmap selection and malformed metadata.'
} finally {
    Get-ChildItem $temp -File | Remove-Item
    Remove-Item $temp
}
