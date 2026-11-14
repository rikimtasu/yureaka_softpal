function Get-DisplayResolutions {
    param([int]$Width, [int]$Height, [int]$DesktopWidth, [int]$DesktopHeight)
    $choices = [Collections.Generic.List[string]]::new()
    foreach ($size in @('1280x720', '1366x768', '1600x900', '1920x1080',
            '2560x1440', '3840x2160', '1280x800', '1920x1200', '2560x1600',
            '3440x1440', '1024x768', '1280x1024', '1600x1200')) {
        $choices.Add($size)
    }
    foreach ($size in @(@($Width, $Height), @($DesktopWidth, $DesktopHeight))) {
        if ($size[0] -gt 0 -and $size[1] -gt 0) {
            $label = "$($size[0])x$($size[1])"
            if (-not $choices.Contains($label)) { $choices.Add($label) }
        }
    }
    return $choices.ToArray()
}
