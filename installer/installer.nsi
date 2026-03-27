; installer/installer.nsi
; NSIS installer script for window-selector
;
; Structure:
;   1. !include directives
;   2. !define directives (VERSION, icon paths)
;   3. Global attributes (Name, OutFile, InstallDir, InstallDirRegKey, RequestExecutionLevel)
;   4. Var declarations
;   5. MUI page insertions (installer)
;   6. MUI unpage insertions (uninstaller)
;   7. MUI language
;   8. Function CloseRunningInstance   (installer variant)
;   9. Function un.CloseRunningInstance (uninstaller variant)
;  10. Function OptionsPageCreate
;  11. Function OptionsPageLeave
;  12. Function un.CleanupPageCreate
;  13. Function un.CleanupPageLeave
;  14. Function LaunchApp
;  15. Function .onInstSuccess
;  16. Section "Install"
;  17. Section "Uninstall"

; ---------------------------------------------------------------------------
; 1. Includes
; ---------------------------------------------------------------------------
!include "MUI2.nsh"
!include "FileFunc.nsh"
!include "nsDialogs.nsh"
!include "LogicLib.nsh"

; ---------------------------------------------------------------------------
; 2. Defines
; ---------------------------------------------------------------------------

; Version is passed from build system via /DVERSION=x.y.z
!ifndef VERSION
  !define VERSION "0.0.0"
!endif

; Registry path for Add/Remove Programs entry (used in Install and Uninstall sections)
!define UNINST_REG_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\WindowSelector"

; Installer / uninstaller icon
!define MUI_ICON "..\resources\app.ico"
!define MUI_UNICON "..\resources\app.ico"

; ---------------------------------------------------------------------------
; 3. Global attributes
; ---------------------------------------------------------------------------
Name "Window Selector"
OutFile "..\target\x86_64-pc-windows-gnu\release\WindowSelector-${VERSION}-setup.exe"
InstallDir "$LOCALAPPDATA\window-selector"
InstallDirRegKey HKCU "${UNINST_REG_KEY}" "InstallLocation"
RequestExecutionLevel user

; ---------------------------------------------------------------------------
; 4. Variable declarations
; ---------------------------------------------------------------------------
Var StartupCheckbox   ; State of "Start with Windows" checkbox
Var WasRunning        ; "1" if installer closed a running instance, "0" otherwise
Var CleanupCheckbox   ; State of "Remove config and logs?" checkbox in uninstaller

; ---------------------------------------------------------------------------
; 5. Finish page defines (must appear before MUI_PAGE_FINISH insertion)
; ---------------------------------------------------------------------------
!define MUI_FINISHPAGE_RUN ""
!define MUI_FINISHPAGE_RUN_TEXT "Launch Window Selector"
!define MUI_FINISHPAGE_RUN_FUNCTION "LaunchApp"

; ---------------------------------------------------------------------------
; 6. Installer pages
; ---------------------------------------------------------------------------
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
Page custom OptionsPageCreate OptionsPageLeave
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

; ---------------------------------------------------------------------------
; 7. Uninstaller pages
; ---------------------------------------------------------------------------
!insertmacro MUI_UNPAGE_CONFIRM
UninstPage custom un.CleanupPageCreate un.CleanupPageLeave
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_UNPAGE_FINISH

; ---------------------------------------------------------------------------
; 8. Language (must appear after all page macros)
; ---------------------------------------------------------------------------
!insertmacro MUI_LANGUAGE "English"

; ---------------------------------------------------------------------------
; 9. Function CloseRunningInstance  (installer variant)
;
; Finds the message-only window created by window-selector (class name
; "WindowSelectorMsgWnd", parent = HWND_MESSAGE = -3) and sends WM_CLOSE.
; Polls up to 5 seconds for clean exit; falls back to taskkill /f.
; Sets WasRunning = "1" if an instance was found and closed.
; ---------------------------------------------------------------------------
Function CloseRunningInstance
  StrCpy $WasRunning "0"

  ; Find the message-only window.
  ; HWND_MESSAGE = -3 in both 32-bit and 64-bit NSIS (pointer-sized signed int).
  System::Call 'user32::FindWindowExW(p -3, p 0, w "WindowSelectorMsgWnd", p 0) p .r0'
  ${If} $0 P<> 0
    StrCpy $WasRunning "1"

    ; Send WM_CLOSE (0x0010) -- fire-and-forget so installer stays responsive.
    System::Call 'user32::PostMessageW(p r0, i 0x0010, p 0, p 0)'

    ; Poll up to 10 x 500ms = 5 seconds for process exit.
    StrCpy $1 "0"
    ${DoWhile} $1 < 10
      Sleep 500
      System::Call 'user32::FindWindowExW(p -3, p 0, w "WindowSelectorMsgWnd", p 0) p .r0'
      ${If} $0 == 0
        ; Window is gone -- process has exited cleanly.
        Goto done_installer
      ${EndIf}
      IntOp $1 $1 + 1
    ${Loop}

    ; Timeout -- force kill as safety net (AC-3.4).
    nsExec::ExecToLog 'taskkill /f /im window-selector.exe'

    ; Brief pause after force kill to allow OS to release file handles.
    Sleep 500

    done_installer:
  ${EndIf}
FunctionEnd

; ---------------------------------------------------------------------------
; 10. Function un.CloseRunningInstance  (uninstaller variant)
;
; Identical logic to CloseRunningInstance. NSIS requires separate un. prefixed
; function definitions for code called from the Uninstall section.
; WasRunning is NOT set here -- restart logic is installer-only.
; ---------------------------------------------------------------------------
Function un.CloseRunningInstance
  ; Find the message-only window.
  System::Call 'user32::FindWindowExW(p -3, p 0, w "WindowSelectorMsgWnd", p 0) p .r0'
  ${If} $0 P<> 0
    ; Send WM_CLOSE (0x0010).
    System::Call 'user32::PostMessageW(p r0, i 0x0010, p 0, p 0)'

    ; Poll up to 10 x 500ms = 5 seconds for process exit.
    StrCpy $1 "0"
    ${DoWhile} $1 < 10
      Sleep 500
      System::Call 'user32::FindWindowExW(p -3, p 0, w "WindowSelectorMsgWnd", p 0) p .r0'
      ${If} $0 == 0
        Goto done_uninstaller
      ${EndIf}
      IntOp $1 $1 + 1
    ${Loop}

    ; Timeout -- force kill.
    nsExec::ExecToLog 'taskkill /f /im window-selector.exe'
    Sleep 500

    done_uninstaller:
  ${EndIf}
FunctionEnd

; ---------------------------------------------------------------------------
; 11. Function OptionsPageCreate
;
; Custom installer page with "Start with Windows" checkbox (default: checked).
; ---------------------------------------------------------------------------
Function OptionsPageCreate
  !insertmacro MUI_HEADER_TEXT "Options" "Configure startup behavior."
  nsDialogs::Create 1018
  Pop $0
  ${NSD_CreateCheckbox} 10u 30u 100% 12u "Start Window Selector when I sign in to Windows"
  Pop $StartupCheckbox
  ${NSD_Check} $StartupCheckbox   ; Default: checked
  nsDialogs::Show
FunctionEnd

; ---------------------------------------------------------------------------
; 12. Function OptionsPageLeave
;
; Reads the checkbox state into $StartupCheckbox for use in the Install section.
; ---------------------------------------------------------------------------
Function OptionsPageLeave
  ${NSD_GetState} $StartupCheckbox $StartupCheckbox
FunctionEnd

; ---------------------------------------------------------------------------
; 13. Function un.CleanupPageCreate
;
; Custom uninstaller page with "Remove config and logs?" checkbox (default: unchecked).
; ---------------------------------------------------------------------------
Function un.CleanupPageCreate
  !insertmacro MUI_HEADER_TEXT "Remove User Data" "Choose whether to remove configuration and logs."
  nsDialogs::Create 1018
  Pop $0
  ${NSD_CreateCheckbox} 10u 30u 100% 12u \
    "Also remove configuration and log files ($APPDATA\window-selector)"
  Pop $CleanupCheckbox
  ; Default: unchecked -- preserve user data by default.
  nsDialogs::Show
FunctionEnd

; ---------------------------------------------------------------------------
; 14. Function un.CleanupPageLeave
;
; Reads the cleanup checkbox state into $CleanupCheckbox for use in Section "Uninstall".
; ---------------------------------------------------------------------------
Function un.CleanupPageLeave
  ${NSD_GetState} $CleanupCheckbox $CleanupCheckbox
FunctionEnd

; ---------------------------------------------------------------------------
; 15. Function LaunchApp
;
; Called by MUI finish page "Launch Window Selector" checkbox.
; Checks if the app is already running (restarted by .onInstSuccess on upgrade)
; before launching to avoid running two instances.
; ---------------------------------------------------------------------------
Function LaunchApp
  ; Check if already running -- .onInstSuccess may have restarted it for an upgrade.
  System::Call 'user32::FindWindowExW(p -3, p 0, w "WindowSelectorMsgWnd", p 0) p .r0'
  ${If} $0 == 0
    ; Not running -- launch it now.
    Exec '"$INSTDIR\window-selector.exe"'
  ${EndIf}
FunctionEnd

; ---------------------------------------------------------------------------
; 16. Function .onInstSuccess
;
; Fired automatically by NSIS after the install section completes successfully.
; If the app was running before the upgrade (WasRunning == "1"), restart it now
; so the upgrade is transparent to the user.
; ---------------------------------------------------------------------------
Function .onInstSuccess
  ${If} $WasRunning == "1"
    Exec '"$INSTDIR\window-selector.exe"'
  ${EndIf}
FunctionEnd

; ---------------------------------------------------------------------------
; 17. Section "Install"
;
; Main installer section:
;   - Close any running instance (sets WasRunning)
;   - Copy files to $INSTDIR
;   - Create uninstaller
;   - Create Start Menu shortcut
;   - Write Add/Remove Programs registry keys
;   - Conditionally write/delete startup Run registry key
; ---------------------------------------------------------------------------
Section "Install"
  ; Close any running instance before overwriting files.
  Call CloseRunningInstance

  SetOutPath "$INSTDIR"

  ; Install application binary and icon.
  File "..\target\x86_64-pc-windows-gnu\release\window-selector.exe"
  File "..\resources\app.ico"

  ; Create uninstaller stub in the install directory.
  WriteUninstaller "$INSTDIR\uninstall.exe"

  ; Start Menu shortcut (user-scope $SMPROGRAMS).
  CreateShortcut \
    "$SMPROGRAMS\Window Selector.lnk" \
    "$INSTDIR\window-selector.exe" \
    "" \
    "$INSTDIR\app.ico"

  ; Add/Remove Programs (Programs and Features) registry keys.
  WriteRegStr HKCU \
    "${UNINST_REG_KEY}" \
    "DisplayName" "Window Selector"
  WriteRegStr HKCU \
    "${UNINST_REG_KEY}" \
    "DisplayIcon" "$INSTDIR\app.ico"
  WriteRegStr HKCU \
    "${UNINST_REG_KEY}" \
    "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKCU \
    "${UNINST_REG_KEY}" \
    "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU \
    "${UNINST_REG_KEY}" \
    "DisplayVersion" "${VERSION}"
  WriteRegDWORD HKCU \
    "${UNINST_REG_KEY}" \
    "NoModify" 1
  WriteRegDWORD HKCU \
    "${UNINST_REG_KEY}" \
    "NoRepair" 1
  WriteRegStr HKCU \
    "${UNINST_REG_KEY}" \
    "Publisher" "window-selector"

  ; Compute and write estimated install size (in KB) for Programs and Features.
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKCU \
    "${UNINST_REG_KEY}" \
    "EstimatedSize" $0

  ; Startup Run registry key -- conditional on Options page checkbox.
  ; Written on every install for idempotency: either add or remove the key.
  ${If} $StartupCheckbox == ${BST_CHECKED}
    WriteRegStr HKCU \
      "Software\Microsoft\Windows\CurrentVersion\Run" \
      "WindowSelector" '"$INSTDIR\window-selector.exe"'
  ${Else}
    ; Remove startup key if it exists from a previous install where it was enabled.
    DeleteRegValue HKCU \
      "Software\Microsoft\Windows\CurrentVersion\Run" \
      "WindowSelector"
  ${EndIf}
SectionEnd

; ---------------------------------------------------------------------------
; 18. Section "Uninstall"
;
; Uninstaller section:
;   - Close any running instance
;   - Delete installed files
;   - Delete Start Menu shortcut
;   - Delete startup Run registry key
;   - Delete Add/Remove Programs registry key
;   - Conditionally remove $APPDATA\window-selector (user data)
;   - Remove install directory (only if empty after file deletion)
; ---------------------------------------------------------------------------
Section "Uninstall"
  ; Close any running instance before deleting files.
  Call un.CloseRunningInstance

  ; Remove installed files.
  Delete "$INSTDIR\window-selector.exe"
  Delete "$INSTDIR\app.ico"
  Delete "$INSTDIR\uninstall.exe"

  ; Remove Start Menu shortcut.
  Delete "$SMPROGRAMS\Window Selector.lnk"

  ; Remove startup registry key (safe to call even if key does not exist).
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "WindowSelector"

  ; Remove Add/Remove Programs entry.
  DeleteRegKey HKCU "${UNINST_REG_KEY}"

  ; Conditionally remove user config and log data.
  ${If} $CleanupCheckbox == ${BST_CHECKED}
    RMDir /r "$APPDATA\window-selector"
  ${EndIf}

  ; Remove install directory -- plain RMDir (no /r) so it only removes if empty.
  ; This prevents accidental deletion of user-placed files.
  RMDir "$INSTDIR"
SectionEnd
