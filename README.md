# Yureaka HD

Windows port of **紅月ゆれる恋あかり** using a modified [sena-rs](https://github.com/xmoezzz/sena-rs) engine and the [iroha-softpal](https://github.com/OpenEroge/iroha-softpal) fork's rendering and VM work.

This is a work in progress. The opening scenes, dialogue, backlog, menu and full-screen narrative have been checked; full routes, effects coverage and save import have not. Expect rough edges outside the scenes listed under [Validation and limits](#validation-and-limits).

## Play

Use the **portable ZIP** on 64-bit Windows 10 or 11. Look for `Yureaka-HD-portable-*.zip` on the [Releases page](https://github.com/rikimtasu/yureaka_softpal/releases). If no player package is published, use [Build from source](#build-from-source). The player ZIP includes the engine; playing does not require Rust, Python or PowerShell 7. You need your own installed copy of the game and enough free space for a separate copy of its resources. GitHub's **Download ZIP** contains source code, not the built player package.

1. Extract the portable ZIP to a writable folder. Do not run it from inside the ZIP or put it inside the original game's installation.
2. Double-click **`Launch Yureaka HD.cmd`**. On first launch, select the installed game's folder containing `yureaka.exe`, `data.pac` and `dll/Pal.dll`. Setup copies resources without importing original saves. Wait for the copy to finish.
3. Choose a font, keep **Fullscreen (borderless)** and **2x** selected for a 1080p display, then click **Play**.

You can also run **`Setup Yureaka HD.cmd`** separately. To select a different installation later, use **Set up another game copy...** in the chooser; existing copies and saves are kept.

The default is Windows **MS Gothic** (`msgothic.ttc`). **MS Mincho** (`msmincho.ttc`) is another tested option. Browse accepts `.ttf`, `.otf` and `.ttc` files that the engine can parse. Use a font with Japanese glyphs; collection files use their first face. Invalid fonts stop startup instead of silently selecting a different font. Restart the port to change fonts.

The font selection applies to engine-drawn text, including dialogue, names and backlog. Text baked into images does not change. Select **Original bitmap font** to keep the game's bitmap glyphs instead.

## Display

- The default is **borderless fullscreen** at the monitor resolution. **F11** toggles fullscreen and windowed mode. The chooser's resolution sets the window's client area in physical pixels, including on DPI-scaled desktops.
- Choose an internal render scale from **1× through 4×**. The default **2×** renders the 1280×720 game at **2560×1440**, then fits the whole image to the screen. On a 1920×1080 display it fills the screen without cropping.
- Scripts and hit tests retain their original **1280×720** coordinates. Resizing preserves the aspect ratio and adds black bars where needed.
- With a font file selected, higher render scales also increase the resolution used to draw text. Ruby (pronunciation annotations above Japanese characters) has reserved space above the body line. Full-screen narrative appears over the scene without the dialogue panel or backlog overlay.
- Artwork and movies retain their source detail. Glossary descriptions baked into TIPS popup images stay at the image's original resolution. No replacement or AI-upscaled artwork is installed.

## Files and saves

Settings are in `yureaka.local.json`; the private game copy defaults to `work/game/`, with saves under `work/game/save/`. Game paths and custom-font paths are relative to the port folder, not the current working directory. Installed fonts are located automatically on each PC. Browsing to a custom font outside the folder copies it into the private `fonts/` directory when you click Play. If an installed font is missing on another PC, the chooser falls back to MS Gothic or the original bitmap font.

To move the port, close it and copy the **whole folder**, including `work/`, `fonts/` and `yureaka.local.json`. You do not need to edit paths. Setting up another game copy selects that copy's save directory; it does not move existing saves.

Setup does not write to the original installation. Keep port saves separate: compatibility with the original executable is not established. Do not distribute `work/`, the local configuration, screenshots containing game assets, or fonts without permission.

## Optional launch commands

After completing setup, you can run these from the port folder in Windows PowerShell 5.1 or PowerShell 7. They use the game copy selected in `yureaka.local.json`:

```powershell
# Launch without the chooser:
./scripts/run.ps1 -FontFile '%WINDIR%/Fonts/msmincho.ttc'

# Use the original bitmap font:
./scripts/run.ps1 -OriginalFont

# Render at 3x and fit to a window:
./scripts/run.ps1 -Windowed -RenderScale 3

# Enable port debug logging:
./scripts/run.ps1 -DebugMode

```

The `.cmd` launchers set the script execution policy for their own process. If Windows blocks a manually entered script command, use the `.cmd` launcher instead.

## Build from source

Download and extract the [repository's source ZIP](https://github.com/rikimtasu/yureaka_softpal/archive/refs/heads/main.zip), or clone the repository. Install [Rust](https://rustup.rs/) and [Visual Studio Build Tools](https://visualstudio.microsoft.com/downloads/) with **Desktop development with C++** and a Windows SDK. Run these commands from the extracted source folder:

```powershell
./scripts/build.ps1
./scripts/setup.ps1
```

Then double-click `Launch Yureaka HD.cmd`. Launching an unbuilt source checkout also offers to build it using the installed tools. The build links the C runtime into the executable.

To create a player ZIP with the engine and launchers, but no game resources, saves, local settings or custom fonts:

```powershell
./scripts/package.ps1
```

The package is written under `work/releases/`. Python 3.10+ is only needed for developer diagnostics or the optional `scripts/prepare.py` setup alternative. Source: [rikimtasu/yureaka_softpal](https://github.com/rikimtasu/yureaka_softpal).

## Debug logging

Select **Port debug logging** in the chooser, or use `-DebugMode` as shown above. It writes VM events and per-frame sprite/task state to a timestamped `work/debug-*.log` file. Logs can grow quickly; turn it off for normal play. This option records diagnostics; it does not unlock a developer menu or provide scene jumping.

## Validation and limits

- Focused engine and asset/script test suites passed; see [PORTING.md](PORTING.md) for commands and scope.
- Opening gameplay, custom-font dialogue/names, backlog and the sliding menu were inspected at 1920×1080. MS Gothic and MS Mincho produced different text with matching scenery under identical input.
- A 1920×1200 diagnostic produced 60-pixel top/bottom bars and mapped the injected clicks correctly.
- Full routes, every visual effect, importing original-game saves and audible playback remain unverified. Runtime screenshot checks used muted audio. The original engine's hardware-warning dialogs are suppressed.

Engine code remains under [MPL-2.0](engine/LICENSE-MPL-2.0). Game assets are not covered by that license. Upstream revisions and the Yureaka adaptation are recorded in [PORTING.md](PORTING.md).
