Unicode true
; Every Rewa executable runs elevated (see rewa.manifest), so an unelevated
; setup or uninstaller could neither stop them nor remove the elevated logon task.
RequestExecutionLevel admin
SetCompressor /SOLID lzma

!ifndef VERSION
  !error "VERSION must be supplied by the build script"
!endif
!ifndef BINDIR
  !error "BINDIR must be supplied by the build script"
!endif
!ifndef OUTFILE
  !error "OUTFILE must be supplied by the build script"
!endif

!include "MUI2.nsh"

Name "Rewa"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\Rewa"
InstallDirRegKey HKCU "Software\Rewa" "InstallDir"
BrandingText "Rewa"
Icon "${__FILEDIR__}\rewa.ico"
UninstallIcon "${__FILEDIR__}\rewa.ico"

VIProductVersion "${VERSION}.0"
VIAddVersionKey /LANG=1033 "ProductName" "Rewa"
VIAddVersionKey /LANG=1033 "ProductVersion" "${VERSION}"
VIAddVersionKey /LANG=1033 "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=1033 "FileDescription" "Rewa low-overhead replay recorder installer"
VIAddVersionKey /LANG=1033 "LegalCopyright" "Rewa contributors"

!define MUI_ABORTWARNING
!define MUI_FINISHPAGE_RUN "$INSTDIR\rewa-win-ui.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Open Rewa"
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

!macro StopRewaProcesses
  nsExec::ExecToLog '"$SYSDIR\taskkill.exe" /F /T /IM "rewa-win-ui.exe"'
  Pop $1
  nsExec::ExecToLog '"$SYSDIR\taskkill.exe" /F /T /IM "rewa-tray.exe"'
  Pop $1
  nsExec::ExecToLog '"$SYSDIR\taskkill.exe" /F /T /IM "rewad.exe"'
  Pop $1
  ; A tray or UI that was still completing its startup can outlive the first
  ; process-tree sweep briefly. Run a second synchronous sweep so silent
  ; upgrades and uninstalls never leave the binaries mapped in memory.
  Sleep 500
  nsExec::ExecToLog '"$SYSDIR\taskkill.exe" /F /T /IM "rewa-win-ui.exe"'
  Pop $1
  nsExec::ExecToLog '"$SYSDIR\taskkill.exe" /F /T /IM "rewa-tray.exe"'
  Pop $1
  nsExec::ExecToLog '"$SYSDIR\taskkill.exe" /F /T /IM "rewad.exe"'
  Pop $1
  Sleep 500
!macroend

; Rewa was called Wreath. Stop its processes, carry its startup entry over and
; remove its binaries, shortcuts and uninstall entry; config.toml and
; favorites.json stay behind for Rewa to adopt on first start, and clips stay
; wherever that config points.
!macro RetireWreath
  nsExec::ExecToLog '"$SYSDIR\taskkill.exe" /F /T /IM "wreath-win-ui.exe"'
  Pop $1
  nsExec::ExecToLog '"$SYSDIR\taskkill.exe" /F /T /IM "wreath-tray.exe"'
  Pop $1
  nsExec::ExecToLog '"$SYSDIR\taskkill.exe" /F /T /IM "wreathd.exe"'
  Pop $1
  Sleep 500
  ReadRegStr $2 HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Wreath"
  StrCmp $2 "" wreath_run_done
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Rewa" '"$INSTDIR\rewa-tray.exe"'
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Wreath"
  wreath_run_done:
  ; the elevated "Wreath elevated autostart" task is moved by the elevated tray itself
  ReadRegStr $3 HKCU "Software\Wreath" "InstallDir"
  StrCmp $3 "" 0 +2
  StrCpy $3 "$LOCALAPPDATA\Wreath"
  Delete "$3\wreath-win-ui.exe"
  Delete "$3\wreath-tray.exe"
  Delete "$3\wreathd.exe"
  Delete "$3\wreathctl.exe"
  Delete "$3\Uninstall.exe"
  Delete "$SMPROGRAMS\Wreath\Wreath.lnk"
  Delete "$SMPROGRAMS\Wreath\Uninstall Wreath.lnk"
  RMDir "$SMPROGRAMS\Wreath"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Wreath"
  DeleteRegKey HKCU "Software\Wreath"
!macroend

Section "Rewa" MainSection
  SectionIn RO
  SetShellVarContext current

  ; v0.1.x used rewa-win-ui.exe as the tray. Stop both old and new process
  ; layouts before replacing files so upgrades cannot retain the legacy tray
  ; mutex or leave the old executable mapped in memory.
  !insertmacro StopRewaProcesses
  !insertmacro RetireWreath

  SetOutPath "$INSTDIR"

  File /oname=rewa-win-ui.exe "${BINDIR}\rewa-win-ui.exe"
  File /oname=rewa-tray.exe "${BINDIR}\rewa-tray.exe"
  File /oname=rewad.exe "${BINDIR}\rewad.exe"
  File /oname=rewactl.exe "${BINDIR}\rewactl.exe"
  WriteUninstaller "$INSTDIR\Uninstall.exe"

  ; Preserve an existing opt-in while migrating the old UI-based autostart.
  ReadRegStr $0 HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Rewa"
  StrCmp $0 "" autostart_migrated
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Rewa" '"$INSTDIR\rewa-tray.exe"'
  autostart_migrated:

  CreateDirectory "$SMPROGRAMS\Rewa"
  CreateShortcut "$SMPROGRAMS\Rewa\Rewa.lnk" "$INSTDIR\rewa-win-ui.exe" "" "$INSTDIR\rewa-win-ui.exe" 0
  CreateShortcut "$SMPROGRAMS\Rewa\Uninstall Rewa.lnk" "$INSTDIR\Uninstall.exe"

  WriteRegStr HKCU "Software\Rewa" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Rewa" "DisplayName" "Rewa"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Rewa" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Rewa" "DisplayIcon" "$INSTDIR\rewa-win-ui.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Rewa" "Publisher" "Rewa"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Rewa" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Rewa" "QuietUninstallString" '"$INSTDIR\Uninstall.exe" /S'
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Rewa" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Rewa" "NoRepair" 1

  ; the executables keep their paths across updates, so Explorer would keep
  ; showing a cached old icon; SHCNE_ASSOCCHANGED makes it read them again
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
SectionEnd

Section "Uninstall"
  SetShellVarContext current

  ; Ask the recorder to shut down cleanly, then force-stop every Rewa process
  ; as a fallback before deleting the installed binaries.
  nsExec::ExecToLog '"$INSTDIR\rewactl.exe" shutdown'
  Pop $1
  !insertmacro StopRewaProcesses

  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Rewa"
  ; The autostart is a logon task because Windows starts no elevated executable
  ; from the Run key. Removing it needs the rights that registered it.
  nsExec::ExecToLog '"$SYSDIR\schtasks.exe" /Delete /TN "Rewa elevated autostart" /F'
  Pop $1
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Rewa"
  DeleteRegKey /ifempty HKCU "Software\Rewa"

  Delete "$SMPROGRAMS\Rewa\Rewa.lnk"
  Delete "$SMPROGRAMS\Rewa\Uninstall Rewa.lnk"
  RMDir "$SMPROGRAMS\Rewa"

  ; First try an immediate delete. A force-stopped process can disappear from
  ; the process table just before Windows releases its executable mapping, so
  ; wait briefly and retry before falling back to a reboot-time deletion.
  Delete "$INSTDIR\rewa-win-ui.exe"
  Delete "$INSTDIR\rewa-tray.exe"
  Delete "$INSTDIR\rewad.exe"
  Delete "$INSTDIR\rewactl.exe"
  Sleep 1000
  Delete /REBOOTOK "$INSTDIR\rewa-win-ui.exe"
  Delete /REBOOTOK "$INSTDIR\rewa-tray.exe"
  Delete /REBOOTOK "$INSTDIR\rewad.exe"
  Delete /REBOOTOK "$INSTDIR\rewactl.exe"
  Delete /REBOOTOK "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
SectionEnd
