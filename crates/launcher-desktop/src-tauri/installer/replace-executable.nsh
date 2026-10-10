; Swap in the new launcher executable with retries and rollback (adapted from
; Kawma Studio's installer). A silent update must never half-replace the app:
; if the old exe stays locked, put it back and abort with a non-zero exit code.
!ifndef KAWMA_INSTALL_LOG_DIR
  !define KAWMA_INSTALL_LOG_DIR "$LOCALAPPDATA\${BUNDLEID}\logs"
!endif
!macro KawmaReplaceExecutable
  CreateDirectory "${KAWMA_INSTALL_LOG_DIR}"
  FileOpen $9 "${KAWMA_INSTALL_LOG_DIR}\installer.log" a
  FileSeek $9 0 END
  FileWrite $9 "Installing ${VERSION} into $INSTDIR$\r$\n"
  SetOverwrite on
  ClearErrors
  File /oname=kawma-launcher.update.exe "${MAINBINARYSRCPATH}"
  IfErrors kawma_replace_failed
  StrCpy $7 0
  kawma_replace_retry:
    ClearErrors
    Delete "$INSTDIR\kawma-launcher.previous.exe"
    IfErrors kawma_replace_wait
    IfFileExists "$INSTDIR\${MAINBINARYNAME}.exe" 0 kawma_replace_new
    Rename "$INSTDIR\${MAINBINARYNAME}.exe" "$INSTDIR\kawma-launcher.previous.exe"
    IfErrors kawma_replace_wait
  kawma_replace_new:
    ClearErrors
    Rename "$INSTDIR\kawma-launcher.update.exe" "$INSTDIR\${MAINBINARYNAME}.exe"
    IfErrors kawma_replace_rollback
    Delete "$INSTDIR\kawma-launcher.previous.exe"
    FileWrite $9 "Replaced ${MAINBINARYNAME}.exe with ${VERSION}$\r$\n"
    FileClose $9
    Goto kawma_replace_done
  kawma_replace_wait:
    IntOp $7 $7 + 1
    ${If} $7 < 40
      Sleep 250
      Goto kawma_replace_retry
    ${EndIf}
    FileWrite $9 "Executable stayed locked for 10 seconds.$\r$\n"
    Goto kawma_replace_failed
  kawma_replace_rollback:
    Rename "$INSTDIR\kawma-launcher.previous.exe" "$INSTDIR\${MAINBINARYNAME}.exe"
    FileWrite $9 "Replacement failed; restored previous executable.$\r$\n"
  kawma_replace_failed:
    Delete "$INSTDIR\kawma-launcher.update.exe"
    FileWrite $9 "Update aborted; nothing was changed.$\r$\n"
    FileClose $9
    SetErrorLevel 1
    MessageBox MB_OK|MB_ICONSTOP "Kawma Launcher could not replace the app. Close Kawma Launcher and try the update again. Your instances, servers and settings are safe." /SD IDOK
    Abort
  kawma_replace_done:
!macroend
