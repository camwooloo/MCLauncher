; Included by template.nsi through `installerHooks` so these files resolve from
; this folder (the template itself is copied into the build directory).
!include LogicLib.nsh
!include "${__FILEDIR__}\theme.nsh"
!include "${__FILEDIR__}\replace-executable.nsh"
