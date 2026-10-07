; 起動中アプリの処理は installer.nsi の CheckIfAppIsRunning に委ねる。
; installer hook からアプリを強制終了しない。
!macro NSIS_HOOK_PREINSTALL
!macroend

!macro NSIS_HOOK_PREUNINSTALL
!macroend
