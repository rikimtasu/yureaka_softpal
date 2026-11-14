$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw 'Building from source requires Rust (https://rustup.rs), MSVC C++ build tools and the Windows SDK. Players can use the portable ZIP instead.'
}
Push-Location "$root/engine"
$previousFlags = $env:RUSTFLAGS
try {
    # Ship the C runtime in the executable so players need no VC++ redistributable.
    $env:RUSTFLAGS = "$previousFlags -C target-feature=+crt-static".Trim()
    cargo build --locked --release -p pal-vm --bin sena
    if ($LASTEXITCODE -ne 0) { throw 'Engine build failed' }
} finally {
    $env:RUSTFLAGS = $previousFlags
    Pop-Location
}
