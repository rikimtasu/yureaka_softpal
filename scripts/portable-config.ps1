# Settings paths are relative to the port folder, not the caller's working directory.
function Resolve-PortPath([string]$Root, [string]$Path) {
    if (-not $Path) { return '' }
    $expanded = [Environment]::ExpandEnvironmentVariables($Path)
    if (-not [IO.Path]::IsPathRooted($expanded)) { $expanded = Join-Path $Root $expanded }
    return [IO.Path]::GetFullPath($expanded)
}

function ConvertTo-PortPath([string]$Root, [string]$Path) {
    if (-not $Path) { return '' }
    $full = Resolve-PortPath $Root $Path
    foreach ($base in @(
        @{ Path = [IO.Path]::GetFullPath($Root); Token = '' },
        @{ Path = $(if ($env:WINDIR) { Join-Path $env:WINDIR 'Fonts' }); Token = '%WINDIR%/Fonts/' },
        @{ Path = $(if ($env:LOCALAPPDATA) { Join-Path $env:LOCALAPPDATA 'Microsoft/Windows/Fonts' }); Token = '%LOCALAPPDATA%/Microsoft/Windows/Fonts/' }
    )) {
        if (-not $base.Path) { continue }
        $prefix = $base.Path.TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar
        if ($full.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase)) {
            return $base.Token + $full.Substring($prefix.Length).Replace('\', '/')
        }
    }
    # Custom fonts outside the port are copied before saving. Reject nonportable paths.
    throw 'Keep game resources and custom fonts inside the port folder.'
}

function Get-PortEngine([string]$Root) {
    foreach ($relative in @('bin/sena.exe', 'engine/target/release/sena.exe')) {
        $candidate = Join-Path $Root $relative
        if (Test-Path -LiteralPath $candidate -PathType Leaf) { return $candidate }
    }
    throw 'Engine missing. Use the portable ZIP, or run scripts/build.ps1 from a source checkout (Rust/MSVC required).'
}

function Get-PortConfig([string]$Root) {
    $config = [pscustomobject]@{
        game_root = 'work/game'; font_file = '%WINDIR%/Fonts/msgothic.ttc'
        width = 1920; height = 1080; borderless_fullscreen = $true
        integer_scaling = $true; render_scale = 2; debug_mode = $false
    }
    $path = Join-Path $Root 'yureaka.local.json'
    if (Test-Path -LiteralPath $path) {
        $stored = Get-Content -LiteralPath $path -Raw -Encoding UTF8 | ConvertFrom-Json
        foreach ($property in $stored.PSObject.Properties) {
            if ($null -ne $config.PSObject.Properties[$property.Name]) {
                $config.($property.Name) = $property.Value
            }
        }
    }
    $config.render_scale = [Math]::Max(1, [Math]::Min(4, [int]$config.render_scale))
    # Recover old absolute workspace paths after the folder has been moved.
    $game = Resolve-PortPath $Root $config.game_root
    if (-not (Test-Path -LiteralPath $game)) {
        $normalized = ([string]$config.game_root).Replace('\', '/')
        $marker = $normalized.LastIndexOf('/work/', [StringComparison]::OrdinalIgnoreCase)
        if ($marker -ge 0) {
            $candidate = $normalized.Substring($marker + 1)
            if (Test-Path -LiteralPath (Resolve-PortPath $Root $candidate)) { $config.game_root = $candidate }
        }
    }
    $font = Resolve-PortPath $Root $config.font_file
    if ($font -and -not (Test-Path -LiteralPath $font -PathType Leaf)) {
        $config.font_file = '%WINDIR%/Fonts/msgothic.ttc'
        if (-not (Test-Path -LiteralPath (Resolve-PortPath $Root $config.font_file) -PathType Leaf)) {
            $config.font_file = '' # Original bitmap font needs no installed font.
        }
    }
    return $config
}

function Save-PortConfig([string]$Root, $Config) {
    $Config.game_root = ConvertTo-PortPath $Root $Config.game_root
    if ($Config.game_root.StartsWith('%') -or -not $Config.game_root.StartsWith('work/', [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Game resources must be a private copy inside the port/work folder.'
    }
    $font = Resolve-PortPath $Root $Config.font_file
    if ($font) {
        try { $Config.font_file = ConvertTo-PortPath $Root $font }
        catch {
            $fontDirectory = Join-Path $Root 'fonts'
            [void](New-Item -ItemType Directory -Force -Path $fontDirectory)
            $hash = (Get-FileHash -LiteralPath $font -Algorithm SHA256).Hash.ToLowerInvariant()
            $destination = Join-Path $fontDirectory ($hash + [IO.Path]::GetExtension($font).ToLowerInvariant())
            if (-not (Test-Path -LiteralPath $destination)) { Copy-Item -LiteralPath $font -Destination $destination }
            $Config.font_file = ConvertTo-PortPath $Root $destination
        }
    } else { $Config.font_file = '' }
    $json = $Config | ConvertTo-Json
    [IO.File]::WriteAllText((Join-Path $Root 'yureaka.local.json'), $json, [Text.UTF8Encoding]::new($false))
}

function Get-PortGame([string]$Root, [string]$Path) {
    $game = Resolve-PortPath $Root $Path
    $work = (Resolve-PortPath $Root 'work').TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar
    if (-not $game.StartsWith($work, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Game resources must be inside the port/work folder. Run setup first.'
    }
    # Do not allow junctions or links to bypass the private-copy boundary.
    $directory = Get-Item -LiteralPath $game -ErrorAction Stop
    while ($directory -and $directory.FullName -ne [IO.Path]::GetFullPath($Root)) {
        if ($directory.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Linked game roots are not allowed.' }
        $directory = $directory.Parent
    }
    foreach ($required in @('data.pac', 'yureaka.exe', 'dll/Pal.dll')) {
        $file = Join-Path $game $required
        if (-not (Test-Path -LiteralPath $file -PathType Leaf)) { throw "Game resources missing ($required). Run setup first." }
        foreach ($component in @($file, (Split-Path $file -Parent))) {
            if ((Get-Item -LiteralPath $component).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Linked game resources are not allowed.' }
        }
    }
    return $game
}
