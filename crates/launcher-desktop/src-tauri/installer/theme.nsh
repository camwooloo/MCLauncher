; Kawma Launcher installer theme: one dark palette for the native installer
; pages and the custom nsDialogs pages (adapted from Kawma Studio's installer).
; SetWindowTheme disables themed checkbox/radio text colours that ignore SetCtlColors.
!macro KawmaThemeFunctions PREFIX
Function ${PREFIX}KawmaThemeTree
  Exch $0
  Push $1
  Push $2
  SetCtlColors $0 EEEAF4 0E0C13
  System::Call 'user32::GetClassName(p r0, w .r1, i ${NSIS_MAX_STRLEN})'
  ${If} $1 == "Button"
    System::Call 'uxtheme::SetWindowTheme(p r0, w "", w "")'
    SetCtlColors $0 EEEAF4 0E0C13
  ${ElseIf} $1 == "Edit"
    SetCtlColors $0 EEEAF4 1C1726
  ${EndIf}
  System::Call 'user32::GetWindow(p r0, i 5) p .r2'
  ${DoWhile} $2 <> 0
    Push $2
    Call ${PREFIX}KawmaThemeTree
    System::Call 'user32::GetWindow(p r2, i 2) p .r2'
  ${Loop}
  Pop $2
  Pop $1
  Pop $0
FunctionEnd

Function ${PREFIX}KawmaThemePage
  Push $0
  Push $HWNDPARENT
  Call ${PREFIX}KawmaThemeTree
  ; Remove the system branding separator for a quieter footer.
  GetDlgItem $0 $HWNDPARENT 1035
  ShowWindow $0 ${SW_HIDE}
  Pop $0
FunctionEnd

Function ${PREFIX}KawmaThemeInit
  Call ${PREFIX}KawmaThemePage
  ; Supported Windows versions use a dark non-client title bar; older ones ignore this.
  System::Call 'dwmapi::DwmSetWindowAttribute(p $HWNDPARENT, i 20, *i 1, i 4)'
FunctionEnd
!macroend
!insertmacro KawmaThemeFunctions ""
!insertmacro KawmaThemeFunctions "un."
