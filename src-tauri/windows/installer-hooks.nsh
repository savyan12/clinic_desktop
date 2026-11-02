!include "LogicLib.nsh"
!include "FileFunc.nsh"
!include "WordFunc.nsh"

!macro CLINIC_CHECK_VC_RUNTIME
  StrCpy $4 0
  ReadRegDWORD $2 HKLM "SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64" "Installed"
  ReadRegStr $3 HKLM "SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64" "Version"
  ${If} $2 == 1
  ${AndIf} $3 != ""
    StrCpy $4 $3 1
    ${If} $4 == "v"
      StrCpy $3 $3 "" 1
    ${EndIf}
    ${VersionCompare} "$3" "$1" $4
    ${If} $4 == 2
      StrCpy $4 0
    ${Else}
      StrCpy $4 1
    ${EndIf}
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL
  Push $0
  Push $1
  Push $2
  Push $3
  Push $4
  SetRegView 64

  IfFileExists "$INSTDIR\runtime\vcredist\VC_redist.x64.exe" 0 vc_runtime_missing
  ClearErrors
  ${GetFileVersion} "$INSTDIR\runtime\vcredist\VC_redist.x64.exe" $1
  ${If} ${Errors}
  ${OrIf} $1 == ""
    MessageBox MB_ICONSTOP "تعذر التحقق من إصدار مثبت Microsoft Visual C++. أعد تنزيل حزمة المنظومة الموثوقة."
    Abort
  ${EndIf}

  ; An equal or newer x64 runtime is sufficient; do not attempt a downgrade.
  !insertmacro CLINIC_CHECK_VC_RUNTIME
  ${If} $4 == 1
    DetailPrint "متطلبات Microsoft Visual C++ x64 مثبتة بالإصدار المطلوب أو إصدار أحدث."
    Goto vc_runtime_done
  ${EndIf}

  ClearErrors
  ExecWait '"$INSTDIR\runtime\vcredist\VC_redist.x64.exe" /install /quiet /norestart /log "$TEMP\clinic-vcredist-install.log"' $0
  ${If} ${Errors}
    MessageBox MB_ICONSTOP "تعذر تشغيل مثبت Microsoft Visual C++. تحقق من صلاحيات التثبيت ثم أعد المحاولة."
    Abort
  ${EndIf}
  ${If} $0 == 3010
    SetRebootFlag true
  ${ElseIf} $0 != 0
  ${AndIf} $0 != 1638
    MessageBox MB_ICONSTOP "فشل تثبيت Microsoft Visual C++ (رمز الخطأ: $0). السجل: $TEMP\clinic-vcredist-install.log"
    Abort
  ${EndIf}

  ; 1638 is accepted only after verifying an installed x64 version, never on its own.
  !insertmacro CLINIC_CHECK_VC_RUNTIME
  ${If} $4 != 1
    MessageBox MB_ICONSTOP "لم يتم العثور على إصدار Microsoft Visual C++ x64 المطلوب بعد التثبيت (رمز النتيجة: $0). السجل: $TEMP\clinic-vcredist-install.log"
    Abort
  ${EndIf}
  Goto vc_runtime_done

  vc_runtime_missing:
    MessageBox MB_ICONSTOP "ملف Microsoft Visual C++ المطلوب غير موجود داخل المثبت."
    Abort

  vc_runtime_done:
    SetRegView lastused
    Pop $4
    Pop $3
    Pop $2
    Pop $1
    Pop $0
!macroend
