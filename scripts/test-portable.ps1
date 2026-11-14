# Focused end-to-end check using synthetic resources and a tiny argument-recording engine.
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/portable-config.ps1"
function Assert($Condition, [string]$Message) { if (-not $Condition) { throw $Message } }
function Expect-Failure([scriptblock]$Action, [string]$Message) {
    $failed = $false
    try { & $Action | Out-Null } catch { $failed = $true }
    Assert $failed $Message
}
$workspace = Split-Path $PSScriptRoot -Parent
$test = Join-Path $workspace ('work/portable-test-' + (Get-Date -Format 'yyyyMMdd-HHmmss-fffffff'))
$port = Join-Path $test 'port before move'
$source = Join-Path $test 'original installation'
[void](New-Item -ItemType Directory -Path "$port/scripts", "$port/bin", "$source/dll", "$source/save")
Copy-Item -Path "$PSScriptRoot/*.ps1" -Destination "$port/scripts"
foreach ($file in @('data.pac', 'csv.pac', 'yureaka.exe', 'dll/Pal.dll', 'save/original.dat')) {
    [IO.File]::WriteAllText((Join-Path $source $file), 'original')
}
# Missing DLL and linked inputs must be rejected before creating a partial copy.
[void](New-Item -ItemType Directory -Path "$test/incomplete")
Copy-Item -LiteralPath "$source/data.pac", "$source/yureaka.exe" -Destination "$test/incomplete"
Expect-Failure { & "$port/scripts/setup.ps1" -GameDirectory "$test/incomplete" } 'Missing DLL was accepted'
Assert (-not (Test-Path "$port/work/game")) 'Failed setup created output'
[void](New-Item -ItemType Junction -Path "$source/movie" -Target "$source/save")
Expect-Failure { & "$port/scripts/setup.ps1" -GameDirectory $source } 'Linked inputs were accepted'
Assert (-not (Test-Path "$port/work/game")) 'Linked input created output'
# Remove only this test-created junction, without touching its target.
[IO.Directory]::Delete("$source/movie")
& "$port/scripts/setup.ps1" -GameDirectory $source
$stored = Get-Content "$port/yureaka.local.json" -Raw | ConvertFrom-Json
Assert ($stored.game_root -eq 'work/game') 'Setup stored an absolute game path'
Assert (@(Get-ChildItem "$port/work/game/save").Count -eq 0) 'Original saves were imported'
[IO.File]::WriteAllText("$port/work/game/data.pac", 'private change')
Assert ([IO.File]::ReadAllText("$source/data.pac") -eq 'original') 'Resources were hardlinked'
Expect-Failure { & "$port/scripts/setup.ps1" -GameDirectory $source -Destination 'work/game' } 'Setup overwrote an existing copy'
# An external custom font is copied privately; settings no longer depend on its location.
$custom = Join-Path $test 'custom font.ttf'
[IO.File]::WriteAllText($custom, 'synthetic font')
$config = Get-PortConfig $port
$config.font_file = $custom
$config.width = 1600
$config.height = 900
Save-PortConfig $port $config
Assert ($config.font_file.StartsWith('fonts/')) 'Custom font was not made portable'
$probe = @'
using System;
using System.IO;
public class PortableArgumentProbe {
    public static void Main(string[] args) {
        File.WriteAllLines(Environment.GetEnvironmentVariable("YUREAKA_TEST_ARGS"), args);
    }
}
'@
[IO.File]::WriteAllText("$test/probe.cs", $probe)
[IO.File]::WriteAllText("$test/compile-probe.ps1", @'
param([string]$InputFile, [string]$OutputFile)
$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition ([IO.File]::ReadAllText($InputFile)) -OutputAssembly $OutputFile -OutputType ConsoleApplication
'@)
# PowerShell 7 cannot emit console EXEs; Windows PowerShell can and is always available.
& powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$test/compile-probe.ps1" -InputFile "$test/probe.cs" -OutputFile "$port/bin/sena.exe"
Assert ($LASTEXITCODE -eq 0) 'Argument probe compilation failed'
$moved = Join-Path $test ('moved port ' + [char]0x3042)
Move-Item -LiteralPath $port -Destination $moved
Push-Location $workspace
$previous = $env:YUREAKA_TEST_ARGS
try {
    $env:YUREAKA_TEST_ARGS = Join-Path $test 'arguments.txt'
    & "$moved/scripts/run.ps1" -EngineArgs @('--headless', '--no-audio')
    $arguments = @([IO.File]::ReadAllLines($env:YUREAKA_TEST_ARGS))
    Assert ($arguments[0] -eq (Resolve-PortPath $moved 'work/game')) 'Moved game was resolved from the caller directory'
    $fontIndex = [Array]::IndexOf($arguments, '--font-file')
    Assert ($arguments[$fontIndex + 1] -eq (Resolve-PortPath $moved $config.font_file)) 'Moved font did not resolve'
    Assert ($arguments -contains '1600' -and $arguments -contains '900') 'Stored window size was ignored'
    Assert ($arguments -contains '--render-scale' -and $arguments -contains '2') 'Default 2x scale was lost'
    $config.game_root = "$port/work/game" # Simulate settings from an older absolute-path install.
    [IO.File]::WriteAllText("$moved/yureaka.local.json", ($config | ConvertTo-Json))
    $recovered = Get-PortConfig $moved
    Assert ($recovered.game_root -eq 'work/game') 'Legacy absolute game path was not recovered'
    Expect-Failure { Get-PortGame $moved '../original installation' } 'External installation was accepted as writable game root'
    & "$moved/scripts/launch.ps1" -CheckControls
    # First launch can construct defaults without any settings file.
    Move-Item "$moved/yureaka.local.json" "$moved/saved-settings.json"
    & "$moved/scripts/launch.ps1" -CheckControls
} finally {
    $env:YUREAKA_TEST_ARGS = $previous
    Pop-Location
}
Write-Output "PASS: setup isolation, relocation, custom fonts, legacy paths, defaults and launcher controls ($test)"
