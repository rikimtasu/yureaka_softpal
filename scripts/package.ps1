# Build a source-free, relocatable player package. Game data/settings/fonts stay private.
param([string]$OutputDirectory = 'work/releases')
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/portable-config.ps1"
$root = Split-Path $PSScriptRoot -Parent
$engine = Join-Path $root 'engine/target/release/sena.exe'
# Always use the static-CRT build; Cargo reuses it when nothing has changed.
& "$PSScriptRoot/build.ps1"
$output = Resolve-PortPath $root $OutputDirectory
[void](New-Item -ItemType Directory -Force -Path $output)
$name = 'Yureaka-HD-portable-' + (Get-Date -Format 'yyyyMMdd-HHmmss-fffffff')
$stage = Join-Path $output $name
[void](New-Item -ItemType Directory -Path "$stage/bin")
[void](New-Item -ItemType Directory -Path "$stage/scripts")
[void](New-Item -ItemType Directory -Path "$stage/engine")
Copy-Item -LiteralPath $engine -Destination "$stage/bin/sena.exe"
foreach ($file in @('Launch Yureaka HD.cmd', 'Setup Yureaka HD.cmd', 'README.md', 'PORTING.md')) {
    Copy-Item -LiteralPath (Join-Path $root $file) -Destination $stage
}
Copy-Item -LiteralPath "$root/engine/LICENSE-MPL-2.0" -Destination "$stage/engine/LICENSE-MPL-2.0"
foreach ($file in @('launch.ps1', 'run.ps1', 'setup.ps1', 'portable-config.ps1', 'font-catalog.ps1', 'display-options.ps1')) {
    Copy-Item -LiteralPath (Join-Path "$root/scripts" $file) -Destination "$stage/scripts"
}
$zip = "$stage.zip"
Compress-Archive -LiteralPath $stage -DestinationPath $zip
Write-Output "Portable ZIP: $zip"
