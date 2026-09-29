param(
    [switch]$UpdateOnly
)

$ErrorActionPreference = "Stop"
$Target = Join-Path $env:LOCALAPPDATA "Rewa"
$Log = Join-Path $Target "vm-install.log"

New-Item -ItemType Directory -Force -Path $Target | Out-Null
Start-Transcript -Path $Log -Append

try {
    # test VMs set up before the rename still run the Wreath binaries
    foreach ($name in @("rewa-win-ui", "rewa-tray", "rewad", "rewactl", "wreath-win-ui", "wreath-tray", "wreathd", "wreathctl")) {
        Stop-Process -Name $name -Force -ErrorAction SilentlyContinue
    }
    Remove-ItemProperty -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run" -Name Wreath -ErrorAction SilentlyContinue

    foreach ($binary in @("rewa-win-ui.exe", "rewa-tray.exe", "rewad.exe", "rewactl.exe")) {
        Copy-Item -Force (Join-Path $PSScriptRoot $binary) (Join-Path $Target $binary)
    }

    if (-not $UpdateOnly) {
        $videoFolder = [Environment]::GetFolderPath("MyVideos")
        if ([string]::IsNullOrWhiteSpace($videoFolder)) {
            $videoFolder = Join-Path $env:USERPROFILE "Videos"
        }
        $ClipTarget = Join-Path $videoFolder "Rewa"
        New-Item -ItemType Directory -Force -Path $ClipTarget | Out-Null
        Get-ChildItem (Join-Path $PSScriptRoot "Samples") -Filter "*.mp4" -ErrorAction SilentlyContinue |
            ForEach-Object { Copy-Item -Force $_.FullName $ClipTarget }

        $tools = Join-Path $PSScriptRoot "virtio-win-guest-tools.exe"
        if ((Test-Path $tools) -and -not (Test-Path (Join-Path $Target ".virtio-installed"))) {
            $process = Start-Process -FilePath $tools -ArgumentList "/quiet", "/norestart" -Wait -PassThru
            "Exit code: $($process.ExitCode)" | Set-Content (Join-Path $Target ".virtio-installed")
        }

        powercfg.exe /change monitor-timeout-ac 0 | Out-Null
        powercfg.exe /change standby-timeout-ac 0 | Out-Null
        reg.exe add "HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced" /v HideFileExt /t REG_DWORD /d 0 /f | Out-Null
        reg.exe add "HKCU\Software\Microsoft\Windows\CurrentVersion\Run" /v Rewa /t REG_SZ /d ('"' + (Join-Path $Target "rewa-tray.exe") + '"') /f | Out-Null

        $shell = New-Object -ComObject WScript.Shell
        $desktop = [Environment]::GetFolderPath("Desktop")
        $startMenu = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs"
        foreach ($folder in @($desktop, $startMenu)) {
            $shortcut = $shell.CreateShortcut((Join-Path $folder "Rewa.lnk"))
            $shortcut.TargetPath = Join-Path $Target "rewa-win-ui.exe"
            $shortcut.WorkingDirectory = $Target
            $shortcut.IconLocation = (Join-Path $Target "rewa-win-ui.exe") + ",0"
            $shortcut.Save()
        }

        $updateCommand = '@echo off' + "`r`n" +
            'powershell.exe -NoProfile -ExecutionPolicy Bypass -Command "$d=(Get-Volume -FileSystemLabel ''REWA_TEST'').DriveLetter; & ($d + '':\Install-Rewa.ps1'') -UpdateOnly"'
        Set-Content -Encoding Ascii -Path (Join-Path $Target "Update-Rewa.cmd") -Value $updateCommand
        $updateShortcut = $shell.CreateShortcut((Join-Path $desktop "Rewa aus Linux aktualisieren.lnk"))
        $updateShortcut.TargetPath = Join-Path $Target "Update-Rewa.cmd"
        $updateShortcut.WorkingDirectory = $Target
        $updateShortcut.IconLocation = (Join-Path $Target "rewa-win-ui.exe") + ",0"
        $updateShortcut.Save()
    }

    Copy-Item -Force (Join-Path $PSScriptRoot "BUILD-INFO.txt") (Join-Path $Target "BUILD-INFO.txt")
    "ready $(Get-Date -Format o)" | Set-Content (Join-Path $Target "VM-READY.txt")

    if ($UpdateOnly) {
        Start-Process (Join-Path $Target "rewa-tray.exe")
        Start-Process (Join-Path $Target "rewa-win-ui.exe")
    }
    else {
        reg.exe add "HKCU\Software\Microsoft\Windows\CurrentVersion\RunOnce" /v RewaFirstUI /t REG_SZ /d ('"' + (Join-Path $Target "rewa-win-ui.exe") + '"') /f | Out-Null
        shutdown.exe /r /t 20 /c "Rewa VM guest drivers installed"
    }
}
finally {
    Stop-Transcript
}
