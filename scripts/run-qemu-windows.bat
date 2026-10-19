@echo off
setlocal
set "NOVA_DIR=%~dp0"
set "QEMU=%NOVA_QEMU%"
if not defined QEMU set "QEMU=qemu-system-x86_64.exe"
where "%QEMU%" >nul 2>nul
if not errorlevel 1 goto qemu_found
if exist "%ProgramFiles%\qemu\qemu-system-x86_64.exe" set "QEMU=%ProgramFiles%\qemu\qemu-system-x86_64.exe"
if exist "%QEMU%" goto qemu_found
echo Nova OS: qemu-system-x86_64.exe not found.
echo Install QEMU for Windows, add it to PATH, or set NOVA_QEMU to its full path.
pause
exit /b 1

:qemu_found
if not exist "%NOVA_DIR%nova-efi.img" goto missing_image
if not exist "%NOVA_DIR%nova-system.img" goto missing_image
if not defined OVMF_CODE if exist "%NOVA_DIR%OVMF_CODE_4M.fd" set "OVMF_CODE=%NOVA_DIR%OVMF_CODE_4M.fd"
if not defined OVMF_VARS if exist "%NOVA_DIR%OVMF_VARS_4M.fd" set "OVMF_VARS=%NOVA_DIR%OVMF_VARS_4M.fd"
if not defined OVMF_CODE if exist "%ProgramFiles%\qemu\share\qemu\edk2-x86_64-code.fd" set "OVMF_CODE=%ProgramFiles%\qemu\share\qemu\edk2-x86_64-code.fd"
if not defined OVMF_VARS if exist "%ProgramFiles%\qemu\share\qemu\edk2-i386-vars.fd" set "OVMF_VARS=%ProgramFiles%\qemu\share\qemu\edk2-i386-vars.fd"
if defined OVMF_CODE if not defined OVMF_VARS for %%F in ("%OVMF_CODE%") do if exist "%%~dpFOVMF_VARS_4M.fd" set "OVMF_VARS=%%~dpFOVMF_VARS_4M.fd"
if not defined OVMF_CODE goto missing_ovmf
if not defined OVMF_VARS goto missing_ovmf
if not exist "%OVMF_CODE%" goto missing_ovmf
if not exist "%OVMF_VARS%" goto missing_ovmf
copy /y "%OVMF_VARS%" "%NOVA_DIR%OVMF_VARS.runtime.fd" >nul
if errorlevel 1 goto copy_failed
if not defined NOVA_RAM set "NOVA_RAM=256M"
if not defined NOVA_SMP set "NOVA_SMP=1"
echo Starting Nova OS in QEMU. Serial output follows in this window.
"%QEMU%" -machine q35 -m "%NOVA_RAM%" -cpu qemu64 -smp "%NOVA_SMP%" ^
  -drive "if=pflash,format=raw,readonly=on,file=%OVMF_CODE%" ^
  -drive "if=pflash,format=raw,file=%NOVA_DIR%OVMF_VARS.runtime.fd" ^
  -drive "if=ide,index=0,format=raw,file=%NOVA_DIR%nova-efi.img" ^
  -drive "if=none,id=nova-system,format=raw,file=%NOVA_DIR%nova-system.img" ^
  -device virtio-blk-pci,drive=nova-system,disable-modern=on ^
  -device VGA -serial stdio -no-reboot %*
set "NOVA_EXIT=%ERRORLEVEL%"
if not "%NOVA_EXIT%"=="0" pause
exit /b %NOVA_EXIT%

:missing_image
echo Nova OS: nova-efi.img and nova-system.img must be next to this BAT file.
pause
exit /b 1

:missing_ovmf
echo Nova OS: UEFI firmware files were not found.
echo Copy OVMF_CODE_4M.fd and OVMF_VARS_4M.fd next to this BAT file,
echo or set OVMF_CODE and OVMF_VARS to their full paths before running it.
pause
exit /b 1

:copy_failed
echo Nova OS: could not create writable OVMF_VARS.runtime.fd.
echo Extract the archive to a writable directory before starting Nova.
pause
exit /b 1
