# Yureaka HD

Windows port of **紅月ゆれる恋あかり** using a modified [sena-rs](https://github.com/xmoezzz/sena-rs) engine and the [iroha-softpal](https://github.com/OpenEroge/iroha-softpal) fork's rendering and VM work.

This is a work in progress. The opening scenes, dialogue, backlog, menu and full-screen narrative have been checked; full routes, effects coverage and save import have not. Expect rough edges outside the scenes listed under [Validation and limits](#validation-and-limits).

## Play

This local workspace is ready to play: it has a built engine and a private copy of your game resources. The chooser requires **PowerShell 7** (`pwsh`) on Windows. A fresh source checkout needs the setup and build steps under Commands.

1. Double-click **`Launch Yureaka HD.cmd`**.
2. Choose a font file. Keep **Fullscreen (borderless)** and **2x** selected for a 1080p display, then click **Play**.

The default is Windows **MS Gothic** (`msgothic.ttc`). **MS Mincho** (`msmincho.ttc`) is another tested option. Browse accepts `.ttf`, `.otf` and `.ttc` files that the engine can parse. Use a font with Japanese glyphs; collection files use their first face. Invalid fonts stop startup instead of silently selecting a different font. Restart the port to change fonts.

The font selection applies to engine-drawn text, including dialogue, names and backlog. Text baked into images does not change. Select **Original bitmap font** to keep the game's bitmap glyphs instead.

## Display

- The default is **borderless fullscreen** at the monitor resolution. **F11** toggles fullscreen and windowed mode. The chooser's resolution sets the window's client area in physical pixels, including on DPI-scaled desktops.
- Choose an internal render scale from **1× through 4×**. The default **2×** renders the 1280×720 game at **2560×1440**, then fits the whole image to the screen. On a 1920×1080 display it fills the screen without cropping.
- Scripts and hit tests retain their original **1280×720** coordinates. Resizing preserves the aspect ratio and adds black bars where needed.
- With a font file selected, higher render scales also increase the resolution used to draw text. Ruby (pronunciation annotations above Japanese characters) has reserved space above the body line. Full-screen narrative appears over the scene without the dialogue panel or backlog overlay.
- Artwork and movies retain their source detail. Glossary descriptions baked into TIPS popup images stay at the image's original resolution. No replacement or AI-upscaled artwork is installed.

## Files and saves

`yureaka.local.json` stores the selected font, resolution and private resource root. In this prepared workspace it points to `work/game-copy/`, with saves under `work/game-copy/save/`. Changing `game_root` also changes that location.

Setup copies resources without writing to the original installation, which was not modified during this work. Original saves are not imported. Keep port saves separate: compatibility with the original executable is not established. Do not distribute `work/`, the local configuration, screenshots containing game assets, or fonts without permission.

## Commands

Run these from this workspace in PowerShell 7:

```powershell
# Launch without the chooser:
./scripts/run.ps1 -FontFile C:/Windows/Fonts/msmincho.ttc

# Use the original bitmap font:
./scripts/run.ps1 -OriginalFont

# Render at 3x and fit to a window:
./scripts/run.ps1 -Windowed -RenderScale 3

# Enable port debug logging:
./scripts/run.ps1 -DebugMode

# Rebuild with Rust/MSVC installed:
./scripts/build.ps1

# Automated opening diagnostic; writes work/diagnostic.log and work/diag/*.png:
./scripts/diagnose.ps1
```

For a fresh source checkout, create a separate resource copy with Python 3.10+, then build the engine:

```powershell
python scripts/prepare.py 'D:/path/to/your/game' --destination work/my-game
./scripts/build.ps1
```

The destination must not exist. Setup creates `yureaka.local.json` if it is missing. It does not overwrite an existing configuration. To select a different copy, edit that file's `game_root`, for example `"game_root": "D:/Documents/yureaka_HD/work/my-game"`. Forward slashes avoid JSON backslash escaping; use your actual absolute workspace path. Building requires Rust, the MSVC C++ build tools and Windows SDK. The chooser uses Windows Forms.

## Debug logging

Select **Port debug logging** in the chooser, or use `-DebugMode` as shown above. It writes VM events and per-frame sprite/task state to a timestamped `work/debug-*.log` file. Logs can grow quickly; turn it off for normal play. This option records diagnostics; it does not unlock a developer menu or provide scene jumping.

## Validation and limits

- Focused engine and asset/script test suites passed; see [PORTING.md](PORTING.md) for commands and scope.
- Opening gameplay, custom-font dialogue/names, backlog and the sliding menu were inspected at 1920×1080. MS Gothic and MS Mincho produced different text with matching scenery under identical input.
- A 1920×1200 diagnostic produced 60-pixel top/bottom bars and mapped the injected clicks correctly.
- Full routes, every visual effect, importing original-game saves and audible playback remain unverified. Runtime screenshot checks used muted audio. The original engine's hardware-warning dialogs are suppressed.

Engine code remains under [MPL-2.0](engine/LICENSE-MPL-2.0). Game assets are not covered by that license. Upstream revisions and the Yureaka adaptation are recorded in [PORTING.md](PORTING.md).
