param([string]$GameDirectory, [string]$Destination = 'work/game')
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/portable-config.ps1"
$root = Split-Path $PSScriptRoot -Parent
if (-not $GameDirectory) {
    Add-Type -AssemblyName System.Windows.Forms
    $picker = New-Object Windows.Forms.FolderBrowserDialog
    $picker.Description = 'Select the installed Yureaka folder containing yureaka.exe and data.pac. Resources will be copied; the installation and its saves stay unchanged.'
    $picker.ShowNewFolderButton = $false
    try {
        if ($picker.ShowDialog() -ne 'OK') { return }
        $GameDirectory = $picker.SelectedPath
    } finally { $picker.Dispose() }
}
$source = Resolve-PortPath $root $GameDirectory
$target = Resolve-PortPath $root $Destination
if (-not $PSBoundParameters.ContainsKey('Destination') -and (Test-Path -LiteralPath $target)) {
    $target = Resolve-PortPath $root ('work/game-' + (Get-Date -Format 'yyyyMMdd-HHmmss-fffffff'))
}
foreach ($required in @('data.pac', 'yureaka.exe', 'dll/Pal.dll')) {
    if (-not (Test-Path -LiteralPath (Join-Path $source $required) -PathType Leaf)) {
        throw "Select the installation containing data.pac, yureaka.exe and dll/Pal.dll (missing $required)."
    }
}
$work = (Resolve-PortPath $root 'work').TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar
if (-not $target.StartsWith($work, [StringComparison]::OrdinalIgnoreCase)) { throw 'Destination must be inside the port/work folder.' }
$sourcePrefix = $source.TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar
$targetPrefix = $target.TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar
if ($source -eq $target -or $target.StartsWith($sourcePrefix, [StringComparison]::OrdinalIgnoreCase) -or $source.StartsWith($targetPrefix, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'The resource copy must be separate from the installation.'
}
if (Test-Path -LiteralPath $target) { throw 'Destination already exists. Choose a new destination; setup never overwrites saves or resources.' }
# Validate every input and ancestor before creating output or following linked directories.
foreach ($path in @($source, (Split-Path $target -Parent))) {
    while ($path) {
        if (Test-Path -LiteralPath $path) {
            if ((Get-Item -LiteralPath $path).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Linked directory not allowed: $path" }
        }
        $parent = Split-Path $path -Parent
        if ($parent -eq $path) { break }
        $path = $parent
    }
}
$files = @(Get-ChildItem -LiteralPath $source -File -Filter '*.pac')
$files += Get-Item -LiteralPath (Join-Path $source 'yureaka.exe'), (Join-Path $source 'dll/Pal.dll')
foreach ($relative in @('dll', 'movie', 'fonts')) {
    $directory = Join-Path $source $relative
    if (Test-Path -LiteralPath $directory) {
        $pending = New-Object 'Collections.Generic.Stack[string]'
        $pending.Push($directory)
        while ($pending.Count -gt 0) {
            $folder = $pending.Pop()
            if ((Get-Item -LiteralPath $folder).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Linked input not allowed: $folder" }
            foreach ($entry in Get-ChildItem -LiteralPath $folder -Force) {
                if ($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Linked input not allowed: $($entry.FullName)" }
                if ($entry.PSIsContainer) { $pending.Push($entry.FullName) }
                elseif ($relative -ne 'dll') { $files += $entry }
            }
        }
    }
}
foreach ($file in $files) {
    if ($file.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Linked input not allowed: $($file.FullName)" }
}
[void](New-Item -ItemType Directory -Path $target)
foreach ($file in $files) {
    $relative = $file.FullName.Substring($sourcePrefix.Length)
    $output = Join-Path $target $relative
    [void](New-Item -ItemType Directory -Force -Path (Split-Path $output -Parent))
    Write-Host "Copying $relative"
    # Copy bytes, not hardlinks, and leave the private copy writable.
    [IO.File]::Copy($file.FullName, $output, $false)
    [IO.File]::SetAttributes($output, [IO.FileAttributes]::Normal)
}
[void](New-Item -ItemType Directory -Path (Join-Path $target 'save'))
$config = Get-PortConfig $root
$config.game_root = ConvertTo-PortPath $root $target
Save-PortConfig $root $config
Write-Host 'Setup complete. Launch Yureaka HD.cmd opens the font and display chooser.'
