param([switch]$CheckControls)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
. "$PSScriptRoot/font-catalog.ps1"
. "$PSScriptRoot/display-options.ps1"
. "$PSScriptRoot/portable-config.ps1"
$root = Split-Path $PSScriptRoot -Parent
$config = Get-PortConfig $root
if (-not $CheckControls) {
    try { $null = Get-PortEngine $root }
    catch {
        if (-not (Test-Path -LiteralPath "$root/engine/Cargo.toml")) { throw }
        $answer = [Windows.Forms.MessageBox]::Show(
            'This source checkout needs to build the engine first. Rust, MSVC C++ build tools and the Windows SDK must already be installed. Build now? To play without these tools, use the portable ZIP.',
            'Engine missing', 'YesNo', 'Question')
        if ($answer -ne 'Yes') { return }
        & "$PSScriptRoot/build.ps1"
        $null = Get-PortEngine $root
    }
    try { $null = Get-PortGame $root $config.game_root }
    catch {
        & "$PSScriptRoot/setup.ps1"
        $config = Get-PortConfig $root
        try { $null = Get-PortGame $root $config.game_root } catch { return }
    }
}
$form = New-Object Windows.Forms.Form
$form.Text = 'Yureaka HD - Font and display'
$form.ClientSize = New-Object Drawing.Size(620, 420)
$form.StartPosition = 'CenterScreen'
$form.FormBorderStyle = 'FixedDialog'
$form.MaximizeBox = $false
$label = New-Object Windows.Forms.Label
$label.Text = 'Text font (TrueType / OpenType / collection; Japanese coverage required)'
$label.SetBounds(15, 15, 590, 25)
$fonts = New-Object Windows.Forms.ComboBox
$fonts.SetBounds(15, 43, 480, 30)
$fonts.DropDownStyle = 'DropDownList'
$fonts.DisplayMember = 'Name'
$fonts.ValueMember = 'Path'
[void]$fonts.Items.Add([pscustomobject]@{ Name = 'Original bitmap font'; Path = '' })
$fontDirs = @("$root/fonts", "$env:WINDIR/Fonts", "$env:LOCALAPPDATA/Microsoft/Windows/Fonts")
foreach ($dir in $fontDirs) {
    if (Test-Path $dir) {
        Get-ChildItem $dir -File | Where-Object { $_.Extension -in '.ttf', '.otf', '.ttc' } | Sort-Object Name | ForEach-Object {
            [void]$fonts.Items.Add((New-FontChoice $_.FullName))
        }
    }
}
Select-FontPath $fonts (Resolve-PortPath $root $config.font_file)
$fontPathLabel = New-Object Windows.Forms.Label
$fontPathLabel.SetBounds(15, 75, 590, 22)
$fontPathLabel.AutoEllipsis = $true
$fontPathLabel.Text = $fonts.SelectedItem.Path
$fonts.Add_SelectedIndexChanged({ $fontPathLabel.Text = $fonts.SelectedItem.Path })
$browse = New-Object Windows.Forms.Button
$browse.Text = 'Browse...'
$browse.SetBounds(510, 42, 95, 28)
$browse.Add_Click({
    $dialog = New-Object Windows.Forms.OpenFileDialog
    $dialog.Filter = 'Font files (*.ttf;*.otf;*.ttc)|*.ttf;*.otf;*.ttc'
    if ($dialog.ShowDialog() -eq 'OK') { Select-FontPath $fonts $dialog.FileName }
    $dialog.Dispose()
})
$resolution = New-Object Windows.Forms.ComboBox
$resolutionLabel = New-Object Windows.Forms.Label
$resolutionLabel.Text = 'Window resolution'
$resolutionLabel.SetBounds(15, 102, 190, 22)
$resolution.SetBounds(15, 127, 190, 30)
$resolution.DropDownStyle = 'DropDownList'
$desktop = [Windows.Forms.Screen]::PrimaryScreen.Bounds
[void]$resolution.Items.AddRange([string[]](Get-DisplayResolutions $config.width $config.height $desktop.Width $desktop.Height))
$resolution.SelectedItem = "$($config.width)x$($config.height)"
if ($resolution.SelectedIndex -lt 0) { $resolution.SelectedIndex = 0 }
$modeLabel = New-Object Windows.Forms.Label
$modeLabel.Text = 'Display mode'
$modeLabel.SetBounds(225, 102, 360, 22)
$displayMode = New-Object Windows.Forms.ComboBox
$displayMode.DropDownStyle = 'DropDownList'
$displayMode.SetBounds(225, 127, 380, 30)
[void]$displayMode.Items.AddRange(@('Windowed', 'Fullscreen (borderless)'))
$displayMode.SelectedIndex = [int][bool]$config.borderless_fullscreen
$resolution.Enabled = $displayMode.SelectedIndex -eq 0
$displayMode.Add_SelectedIndexChanged({ $resolution.Enabled = $displayMode.SelectedIndex -eq 0 })
$integer = New-Object Windows.Forms.CheckBox
$integer.Text = 'Integer render scale, then fit screen'
$integer.SetBounds(15, 165, 300, 30)
$integer.Checked = $config.integer_scaling
$scale = New-Object Windows.Forms.ComboBox
$scale.DropDownStyle = 'DropDownList'
$scale.SetBounds(335, 165, 270, 30)
[void]$scale.Items.AddRange(@('1x (1280x720)', '2x (2560x1440) - default', '3x (3840x2160)', '4x (5120x2880)'))
$scale.SelectedIndex = $config.render_scale - 1
$scale.Enabled = $integer.Checked
$integer.Add_CheckedChanged({ $scale.Enabled = $integer.Checked })
$note = New-Object Windows.Forms.Label
$note.Text = 'F11 toggles fullscreen. The selected scale is the internal rendering resolution. The whole image is fitted to your screen without cropping. At 1080p, 2x renders 1440p and downsamples to fill 1080p.'
$note.SetBounds(15, 210, 590, 60)
$debugMode = New-Object Windows.Forms.CheckBox
$debugMode.Text = 'Port debug logging (not the original developer menu)'
$debugMode.SetBounds(15, 275, 590, 30)
$debugMode.Checked = $config.debug_mode
$setup = New-Object Windows.Forms.Button
$setup.Text = 'Set up another game copy...'
$setup.SetBounds(15, 320, 260, 30)
$setup.Add_Click({
    try {
        & "$PSScriptRoot/setup.ps1" -Destination ('work/game-' + (Get-Date -Format 'yyyyMMdd-HHmmss-fffffff'))
        $updated = Get-PortConfig $root
        $config.game_root = $updated.game_root
    } catch { [void][Windows.Forms.MessageBox]::Show($_.Exception.Message, 'Setup failed') }
})
$start = New-Object Windows.Forms.Button
$start.Text = 'Play'
$start.SetBounds(510, 375, 95, 30)
$start.Add_Click({
    $font = $fonts.SelectedItem.Path
    if ($font -and -not (Test-Path -LiteralPath $font -PathType Leaf)) {
        [void][Windows.Forms.MessageBox]::Show('Select an existing font file.'); return
    }
    $size = $resolution.SelectedItem.Split('x')
    $config.font_file = $font
    $config.width = [int]$size[0]
    $config.height = [int]$size[1]
    $config.borderless_fullscreen = $displayMode.SelectedIndex -eq 1
    $config.integer_scaling = $integer.Checked
    $config.render_scale = $scale.SelectedIndex + 1
    $config.debug_mode = $debugMode.Checked
    try { Save-PortConfig $root $config }
    catch { [void][Windows.Forms.MessageBox]::Show($_.Exception.Message, 'Cannot save settings'); return }
    $form.DialogResult = 'OK'
    $form.Close()
})
$form.Controls.AddRange(@($label, $fonts, $browse, $fontPathLabel, $resolutionLabel, $resolution, $modeLabel, $displayMode, $integer, $scale, $note, $debugMode, $setup, $start))
$form.AcceptButton = $start
if ($CheckControls) {
    $displayMode.SelectedIndex = 0
    if (-not $resolution.Enabled) { throw 'Window resolution must be enabled in windowed mode' }
    $displayMode.SelectedIndex = 1
    if ($resolution.Enabled) { throw 'Fullscreen must use the monitor resolution' }
    if ($resolution.Items.Count -lt 10) { throw 'Common display resolutions are missing' }
    if ($resolution.SelectedIndex -lt 0) { throw 'Stored display resolution was not selected' }
    if ($integer.Checked -ne [bool]$config.integer_scaling) { throw 'Integer scaling preference was not selected' }
    if ($scale.SelectedIndex -ne $config.render_scale - 1) { throw 'Stored render scale was not selected' }
    if ($scale.Items.Count -ne 4) { throw 'Render scales 1x through 4x are missing' }
    $integer.Checked = $false
    if ($scale.Enabled) { throw 'Render scale control must follow the checkbox' }
    $integer.Checked = $true
    if (-not $scale.Enabled) { throw 'Render scale control must follow the checkbox' }
    $form.Dispose()
    'PASS: display modes, common resolutions and resolution-control state; no settings written or game launched.'
    return
}
$result = $form.ShowDialog()
$form.Dispose()
if ($result -eq 'OK') {
    & "$PSScriptRoot/run.ps1" -FontFile $config.font_file -Width $config.width -Height $config.height -OriginalFont:(!$config.font_file) -BorderlessFullscreen:$config.borderless_fullscreen -IntegerScaling:$config.integer_scaling -RenderScale $config.render_scale -DebugMode:$config.debug_mode
}
