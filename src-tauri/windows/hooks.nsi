; Running-app handling is delegated to CheckIfAppIsRunning in installer.nsi.
; Do not force-kill the application from installer hooks.
!macro NSIS_HOOK_PREINSTALL
!macroend

!macro NSIS_HOOK_PREUNINSTALL
!macroend
