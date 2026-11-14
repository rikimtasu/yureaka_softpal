param([int]$Frames = 9000)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
[void](New-Item "$root/work/diag" -ItemType Directory -Force)
[Console]::OutputEncoding = [Text.UTF8Encoding]::new()
$options = @('--headless', '--no-audio', '--diagnostic-device-size', '1920x1080',
    '--diagnostic-frames', "$Frames", '--diagnostic-auto-advance',
    '--diagnostic-auto-advance-min-frames', '90', '--diagnostic-click', '3000:280:184',
    '--diagnostic-png-at', "6000:work/diag/adv-1080p.png",
    '--diagnostic-png-at', "$($Frames - 1):work/diag/final-1080p.png")
& "$PSScriptRoot/run.ps1" -EngineArgs $options *> "$root/work/diagnostic.log"
if ($LASTEXITCODE -ne 0) { throw 'Diagnostic run failed; see work/diagnostic.log' }
python "$PSScriptRoot/check_diagnostic.py" "$root/work/diagnostic.log"
if ($LASTEXITCODE -ne 0) { throw 'Skipped operations or faults; see work/diagnostic.log' }
