# Yureaka port notes

## Source

These notes describe the engine adaptation for contributors. `engine/` is the OpenEroge sena-rs `iroha` revision `edbc0d3e1047da132d605c4aa6fd4254b45896d7`, derived from xmoezzz/sena-rs. Local changes are committed on top of that revision as `621b558` and published with the rest of this workspace.

The upstream checkout omits its `reverse/adr/` directory. The included `reverse/port-status.md`, `reverse/iroha-native-profile.md` and `reverse/hidpi-presentation.md` describe earlier engine work; their binary-specific conclusions were not treated as Yureaka evidence.

## Game interface

The checked Yureaka release is identified by the executable/DLL hashes in `engine/reverse/abi/yureaka-vm.json`. It has a Sv20 script, entry `0x31C`, 8,254 script jump targets and 89 graphic-table records. `ARCHIVE.DAT` is inside `data.pac`; `SYSTEM.INI` is inside `csv.pac`. Its logical screen is 1280×720, font size 26, bitmap font type 4 and thick-outline effect 3. Script strings are CP932.

The exact executable/DLL pair, native addresses, reviewed VM wrappers and remaining ABI uncertainty are recorded in `engine/reverse/abi/yureaka-vm.json`, referenced by `pal32.json`. Native code was inspected as data, not loaded or executed.

Build profiles in `crates/pal-vm/src/assets/native_profile.rs` select compatible VM handlers. Yureaka enables individually checked wrappers for text, sprite creation/copy, buttons, sound effects, TIPS, favorite voices and host queries. It does not enable all the handlers selected by the existing registered-build or Trenka profiles. Yureaka's action capacities are 16 banks, 54 lines and 120 sections. Packed button references are resolved for sprite movement. `PalFontProfile::Reference` supports thick-outline effect 3 while retaining single-language bitmap behavior.

## Presentation and font changes

An explicit `--font-file` disables bitmap body text regardless of text-table language, preserves script type-4 calls, and errors if the selected face cannot be loaded. The dialogue, name and backlog renderer rounds text density upward for fractional scales. The wgpu and software renderers filter dense surfaces when reducing them to the viewport. Logical coordinates and original resource dimensions remain unchanged.

`--physical-window-size` makes initial `--width`/`--height` physical client dimensions. The launcher selects this mode and ignores the original game's stored window dimensions.

Portable save version 17 records action-table capacities. The reader also accepts versions through 16, whose action records omit those capacities. Rectangular-tween and mask-reveal action sections can be read. Original-engine save compatibility and exact restoration of all effects are not established.

## Checks

The following counts describe the initial port checks. Current results are recorded in the dated log below.

```powershell
# In engine/:
cargo test --locked --release -p pal-vm -p pal-asset -p pal-script -p pal-disassembled
python reverse/abi/generate.py --check

# In the outer workspace:
python scripts/test_prepare.py
./scripts/diagnose.ps1
python scripts/smoke.py
```

The combined crate suites passed 524 tests with 7 ignored, including 304 passing VM library tests. Tests cover explicit font selection, invalid fonts, visible effect-3 text after dispatch, fractional text density, physical window sizing, independent action capacities, action-save round trips and conservative host-query stack behavior. The ABI freshness check and the two setup tests passed. A Windows Forms check opened the font chooser, checked that its font and resolution lists were populated, and cancelled without launching the game.

`smoke.py` creates separate validation roots under `work/`, preserves earlier saves, and runs font, letterbox, backlog and menu scenarios. It exits unsuccessfully if a process fails, skips a VM operation or reports a VM fault. Visual correctness requires inspecting its PNGs under `work/diag/`.

The 9,000-frame opening diagnostic and independent 9,200-frame smoke runs completed without skipped operations or VM faults. Screenshots were inspected. A pixel comparison confirmed that the two font runs matched outside the text area; the letterboxed output had exactly 60 black rows at each edge.

The Windows GPU renderer initialized on an NVIDIA GeForce RTX 3070 at 1920×1080. A 2,100-frame window run completed without skipped operations or VM faults; its GPU capture at frame 2,000 shows opening gameplay and the custom dialogue font (`work/diag/window-1080p.png`). Full-route behavior and audible playback still require playtesting. Sprite-transition logs contain scale-inheritance warnings, but no associated defect was established in the inspected captures; investigate them if a character appears at an incorrect scale.

## Text and asset loading log (2026-11-13)

Lines with native markup render outline glyphs at the same density as ordinary dialogue. Script `sptext` and `drawtext` operations also update dense surfaces and preserve previously drawn text when appending. TIPS offsets count the renderer's visible text units and exclude ruby/control tags. The reference font profile uses the script's text origin, so TIPS overlays align with the dialogue. Glossary popup descriptions are baked into PGD images and remain source-resolution artwork.

Image decoding uses a 128 MiB cache of decoded bases; audio uses a 32 MiB cache of original PCM. Both caches are bounded by bytes and entry count, and their keys hash the complete encoded input. Delta images resolve their bases again and patch replacements are reapplied, so loose edits and resource overrides are not hidden. Channel gain, fades and loop settings are not cached. These caches are disabled on Vita and Switch. Desktop static audio uses one decode pass; those consoles retain the two-pass path. Local PAC readers keep archive handles open between range reads; browser and network VFSes keep their existing read API.

In the read-only probes, repeated loads of the two tested 2048×2048 sprites took about 3.9 ms, versus 16.0–16.2 ms before this follow-up. Uncached decoding took about 13.5 ms, versus 15.0–15.2 ms. The tested UI image loaded in 0.6 ms on repeat. Three voice clips loaded in 4.8–6.4 ms on first use, versus 8.8–10.8 ms, and about 0.3 ms on repeat. These are CPU/resource loading measurements, not audible playback latency or whole-game frame-rate guarantees. Logs: `work/asset-images-after.log` and `work/audio-load-after.log`.

The current combined crate suites passed 536 tests with 7 ignored (`work/asset-text-final-tests.log`). Tests cover markup density, TIPS origins and ruby offsets, appended dense text, cache eviction and mutation isolation, changed delta bases and new loose overrides. The audio comparison checked every sample bit against the two-pass decoder for a generated WAVE and three real voice clips. The checked sprite matched every RGBA byte in `work/perf-before.png`. Gameplay replay uses `scripts/check_asset_text.py`, which copies saves into a separate validation root and keeps the chosen font.

Run these read-only probes from `engine/` against the prepared resource copy:

```powershell
cargo run --locked --release -p pal-vm --example bench_image_load -- ../work/game-copy 100 ST18A_A070.PGD ST18A_A020.PGD MAIN_BASE00.PGD
cargo run --locked --release -p pal-vm --example bench_audio_load -- ../work/game-copy 100 VO01_00000.OGG VO01_00001.OGG VO01_00002.OGG
```

Both probes use the same local resource copy and Shift-JIS lookup. Audio decoding runs with the device disabled. The image probe reports uncached decoding separately from first and repeated loads through the cache. The sprite baseline is `work/pgd-check-current.log`. The voice baseline is `work/audio-load-before.log`; it used the same probe before the single-pass decoder and cache changes. Fully decoded, non-streaming audio is what this section calls static audio.

## Portable Windows setup (2026-11-14)

`scripts/portable-config.ps1` resolves paths relative to the port folder. Settings store game roots under `work/`, local custom fonts under `fonts/`, and installed fonts with `%WINDIR%` or `%LOCALAPPDATA%` tokens. Earlier setup versions wrote absolute game paths. After moving a folder with those settings, the reader automatically tries the portion starting at `work/` inside the new port folder and uses it if that directory exists. Missing installed fonts fall back to MS Gothic or bitmap text.

`scripts/setup.ps1` uses a folder picker when no installation is supplied. It copies resources to a new workspace-owned directory and excludes original saves. Validation rejects symlinks and NTFS junctions in input files/directories and their parents, and rejects overlapping source/destination directories before copying. Files are copied as bytes, not hardlinked. Repeated setup preserves existing copies. The Python setup alternative also writes a relative game root; developer replay scripts resolve that format.

The `.cmd` launchers use PowerShell 7 if available, otherwise Windows PowerShell 5.1. A source checkout uses `engine/target/release/sena.exe`; a player ZIP uses `bin/sena.exe`. `scripts/build.ps1` requests static CRT linkage, and `scripts/package.ps1` packages the engine and runtime scripts without local game resources, custom fonts or settings.

Focused checks:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -STA -File scripts/test-portable.ps1
python scripts/test_prepare.py
```

The PowerShell check uses synthetic resources and an argument-recording executable. It covers resource-copy independence, save exclusion, missing DLLs, linked inputs, custom-font copying, moving a folder with spaces and Japanese characters, legacy-path recovery, and launcher controls without a settings file. It does not establish gameplay compatibility on another PC. Full-route and audio validation remain outstanding.
