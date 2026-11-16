@echo off
rem Builds the Rust version and its setup exe in installer\output\
setlocal
set CARGO=cargo
where cargo >nul 2>nul || set CARGO=%USERPROFILE%\.cargo\bin\cargo.exe
cd /d "%~dp0..\Notepad9Rust"
"%CARGO%" build --release || exit /b 1
rem example plugins: their own workspace, plugins\<name> builds <name>.dll
"%CARGO%" build --release --manifest-path plugins\Cargo.toml || exit /b 1
if not exist target\release\plugins mkdir target\release\plugins
for /d %%d in (plugins\*) do if exist "%%d\Cargo.toml" copy /y "plugins\target\release\%%~nxd.dll" "target\release\plugins\" >nul || exit /b 1
cd /d "%~dp0"
set ISCC=%LOCALAPPDATA%\Programs\Inno Setup 6\ISCC.exe
if not exist "%ISCC%" set ISCC=%ProgramFiles(x86)%\Inno Setup 6\ISCC.exe
if not exist "%ISCC%" set ISCC=%ProgramFiles%\Inno Setup 6\ISCC.exe
"%ISCC%" Notepad9-Rust.iss || exit /b 1
echo.
echo Installer: %~dp0output\Notepad9-Setup-2.1.0.exe
