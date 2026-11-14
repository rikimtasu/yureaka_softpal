param(
    [string]$FontFile,
    [int]$Width = 1920,
    [int]$Height = 1080,
    [switch]$OriginalFont,
    [switch]$BorderlessFullscreen,
    [switch]$Windowed,
    [switch]$IntegerScaling,
    [ValidateRange(0, 4)][int]$RenderScale = 2,
    [switch]$DebugMode,
    [string]$GameRoot,
    [Parameter(ValueFromRemainingArguments = $true)][string[]]$EngineArgs
)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$config = Get-Content "$root/yureaka.local.json" -Raw | ConvertFrom-Json
$game = [IO.Path]::GetFullPath($(if ($GameRoot) { $GameRoot } else { $config.game_root }))
# Only run against a workspace-owned resource copy. Never fall back to the installation.
$work = [IO.Path]::GetFullPath("$root/work/")
if (-not $game.StartsWith($work, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'game_root must be inside this workspace/work directory. Run scripts/prepare.py first.'
}
$exe = "$root/engine/target/release/sena.exe"
if (-not (Test-Path $exe)) { throw 'Build first: pwsh -File scripts/build.ps1' }
if (-not $FontFile) { $FontFile = $config.font_file }
if (-not $FontFile) { $OriginalFont = $true }
$engineOptions = @($game, '--nls', 'sjis', '--title', 'Yureaka HD',
    '--width', "$Width", '--height', "$Height", '--ignore-config-window-size', '--physical-window-size')
if (-not $OriginalFont) {
    if (-not (Test-Path -LiteralPath $FontFile -PathType Leaf)) { throw "Font not found: $FontFile" }
    $engineOptions += @('--font-file', $FontFile)
}
$previousRustLog = $env:RUST_LOG
$previousPalDebug = $env:PAL_DEBUG
if (-not $env:RUST_LOG) { $env:RUST_LOG = 'info' }
if (-not $Windowed -and ($BorderlessFullscreen -or $config.borderless_fullscreen)) {
    $engineOptions += '--borderless-fullscreen'
}
$useScaling = if ($PSBoundParameters.ContainsKey('IntegerScaling')) { [bool]$IntegerScaling } elseif ($null -eq $config.PSObject.Properties['integer_scaling']) { $true } else { [bool]$config.integer_scaling }
if (-not $PSBoundParameters.ContainsKey('RenderScale') -and $null -ne $config.PSObject.Properties['render_scale']) { $RenderScale = [int]$config.render_scale }
if ($useScaling -and $RenderScale -gt 0) { $engineOptions += @('--render-scale', "$RenderScale") }
$useDebug = if ($PSBoundParameters.ContainsKey('DebugMode')) { [bool]$DebugMode } else { [bool]$config.debug_mode }
Push-Location $root
try {
    if ($useDebug) {
        $env:PAL_DEBUG = '1'
        $env:RUST_LOG = 'info,pal_vm::runtime=debug'
        $debugLog = Join-Path $work ("debug-" + (Get-Date -Format 'yyyyMMdd-HHmmss-fffffff') + '.log')
        Write-Host "Port debug logging enabled. Log: $debugLog"
        & $exe @engineOptions @EngineArgs *> $debugLog
    } else {
        & $exe @engineOptions @EngineArgs
    }
    if ($LASTEXITCODE -ne 0) { throw "Port exited with code $LASTEXITCODE" }
} finally {
    $env:RUST_LOG = $previousRustLog
    $env:PAL_DEBUG = $previousPalDebug
    Pop-Location
}
