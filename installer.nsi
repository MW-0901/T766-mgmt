!include "MUI2.nsh"

!define APP_NAME "T766 Control System"
!define APP_VERSION "0.1.0"
!define APP_PUBLISHER "T766"
!define APP_IDENTIFIER "com.t766.control"

Name "${APP_NAME}"
OutFile "T766-ControlClient-Installer.exe"
InstallDir "$PROGRAMFILES64\${APP_NAME}"
RequestExecutionLevel admin

;--- MUI pages ---
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

;--- Install section ---
Section "Install"
  SetOutPath "$INSTDIR"

  File "T766-ControlClient.exe"
  File "T766-CheckinApp.exe"
  File "settings.toml"

  WriteUninstaller "$INSTDIR\Uninstall.exe"

  ; Add/Remove Programs entry
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_IDENTIFIER}" \
    "DisplayName" "${APP_NAME}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_IDENTIFIER}" \
    "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_IDENTIFIER}" \
    "Publisher" "${APP_PUBLISHER}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_IDENTIFIER}" \
    "DisplayVersion" "${APP_VERSION}"

  ; --- T766 custom setup (from Packager.toml preinstall-section) ---
  CreateDirectory "$COMMONAPPDATA\${APP_NAME}"

  IfFileExists "$COMMONAPPDATA\${APP_NAME}\checkin-logs" +3 0
  FileOpen $0 "$COMMONAPPDATA\${APP_NAME}\checkin-logs" w
  FileClose $0

  nsExec::ExecToLog '"$SYSDIR\icacls.exe" "$COMMONAPPDATA\${APP_NAME}\checkin-logs" /grant "Users:(M)" /Q'

  ; Scheduled task via XML
  StrCpy $R3 "$TEMP\t766-task.xml"
  FileOpen $9 "$R3" w
  FileWrite $9 '<?xml version="1.0" encoding="UTF-16"?>'
  FileWrite $9 '<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">'
  FileWrite $9 '<Principals><Principal id="Author"><UserId>S-1-5-18</UserId><RunLevel>HighestAvailable</RunLevel></Principal></Principals>'
  FileWrite $9 '<Settings><DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries><StopIfGoingOnBatteries>false</StopIfGoingOnBatteries><MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy></Settings>'
  FileWrite $9 '<Triggers><BootTrigger><Delay>PT2S</Delay></BootTrigger></Triggers>'
  FileWrite $9 '<Actions Context="Author"><Exec><Command>$INSTDIR\T766-ControlClient.exe</Command></Exec></Actions>'
  FileWrite $9 '</Task>'
  FileClose $9

  nsExec::ExecToStack '"$SYSDIR\schtasks.exe" /Create /F /TN "${APP_NAME}\T766-ControlClient" /XML "$R3" /RU SYSTEM'
  Pop $R0
  Pop $R1
  Delete "$R3"
  IntCmp $R0 0 schtasks_ok schtasks_fail schtasks_fail
  schtasks_fail:
    MessageBox MB_OK "WARNING: Failed to create scheduled task. Exit code: $R0 Output: $R1"
  schtasks_ok:

  CreateShortCut "$SMSTARTUP\T766-CheckinApp.lnk" "$INSTDIR\T766-CheckinApp.exe"

  Delete "$DESKTOP\${APP_NAME}.lnk"
  Delete "$DESKTOP\T766-ControlClient.lnk"
  Delete "$DESKTOP\T766-CheckinApp.lnk"

  WriteRegStr HKLM "Software\T766\Control" "InstallPath" "$INSTDIR"
SectionEnd

;--- Uninstall section ---
Section "Uninstall"
  nsExec::ExecToLog '"$SYSDIR\schtasks.exe" /Delete /F /TN "${APP_NAME}\T766-ControlClient"'

  Delete "$SMSTARTUP\T766-CheckinApp.lnk"

  DeleteRegKey HKLM "Software\T766\Control"

  MessageBox MB_YESNO "Do you want to remove application settings & logs?" IDNO skip_data
  Delete "$COMMONAPPDATA\${APP_NAME}\checkin-logs"
  Delete "$COMMONAPPDATA\${APP_NAME}\old-checkin-logs"
  Delete "$COMMONAPPDATA\${APP_NAME}\settings.toml"
  Delete "$COMMONAPPDATA\${APP_NAME}\last_run.txt"
  Delete "$COMMONAPPDATA\${APP_NAME}\client.log"
  Delete "$COMMONAPPDATA\${APP_NAME}\client.old.log"
  RMDir "$COMMONAPPDATA\${APP_NAME}"
  skip_data:

  Delete "$INSTDIR\T766-ControlClient.exe"
  Delete "$INSTDIR\T766-CheckinApp.exe"
  Delete "$INSTDIR\settings.toml"
  Delete "$INSTDIR\Uninstall.exe"

  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_IDENTIFIER}"

  RMDir "$INSTDIR"
SectionEnd
