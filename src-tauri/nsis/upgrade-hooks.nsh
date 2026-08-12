!macro HUSHREADER_DELETE_OLD_SHORTCUTS
  !ifmacrodef UnpinShortcut
    !insertmacro UnpinShortcut "$DESKTOP\HushReader.lnk"
    !insertmacro UnpinShortcut "$SMPROGRAMS\HushReader.lnk"
  !endif
  Delete "$DESKTOP\HushReader.lnk"
  Delete "$SMPROGRAMS\HushReader.lnk"
!macroend

!macro HUSHREADER_CREATE_NEW_SHORTCUTS
  ${If} $NoShortcutMode <> 1
    ${IfNot} ${FileExists} "$SMPROGRAMS\${PRODUCTNAME}.lnk"
      CreateShortcut "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$INSTDIR\app.exe"
      !insertmacro SetLnkAppUserModelId "$SMPROGRAMS\${PRODUCTNAME}.lnk"
    ${EndIf}
    ${If} $UpdateMode = 1
    ${OrIf} $PassiveMode = 1
    ${OrIf} ${Silent}
      ${IfNot} ${FileExists} "$DESKTOP\${PRODUCTNAME}.lnk"
        CreateShortcut "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\app.exe"
        !insertmacro SetLnkAppUserModelId "$DESKTOP\${PRODUCTNAME}.lnk"
      ${EndIf}
    ${EndIf}
  ${EndIf}
!macroend

!macro HUSHREADER_STOP_BACKEND
  ClearErrors
  FileOpen $1 "$APPDATA\com.hushreader.desktop\sonovel\backend.pid" r
  ${IfNot} ${Errors}
    FileRead $1 $0
    FileClose $1
    ${If} $0 != ""
      nsExec::Exec '"$SYSDIR\taskkill.exe" /F /PID $0'
      Pop $1
    ${EndIf}
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREINSTALL
  StrCpy $1 $INSTDIR
  ReadRegStr $0 SHCTX "Software\hushreader\HushReader" ""
  ${If} $0 != ""
    ${If} ${FileExists} "$0\*.*"
      StrCpy $INSTDIR $0
      SetOutPath $INSTDIR
      WriteRegStr SHCTX "Software\hushreader\${PRODUCTNAME}" "" $0
      ${If} $1 != $0
        RMDir "$1"
      ${EndIf}
    ${EndIf}
  ${EndIf}

  !insertmacro HUSHREADER_DELETE_OLD_SHORTCUTS
  DeleteRegKey SHCTX "Software\Microsoft\Windows\CurrentVersion\Uninstall\HushReader"
!macroend

!macro NSIS_HOOK_POSTINSTALL
  !insertmacro HUSHREADER_CREATE_NEW_SHORTCUTS
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro HUSHREADER_STOP_BACKEND
  !insertmacro HUSHREADER_DELETE_OLD_SHORTCUTS
  DeleteRegKey SHCTX "Software\Microsoft\Windows\CurrentVersion\Uninstall\HushReader"
!macroend
