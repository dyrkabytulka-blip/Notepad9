@echo off
rem Builds publish\ (self-contained, no .NET needed on the target PC) and the setup exe in installer\output\
setlocal
cd /d "%~dp0"
dotnet publish "..\Notepad9\Notepad9.csproj" -c Release -r win-x64 --self-contained true -o "..\publish" || exit /b 1
set ISCC=%LOCALAPPDATA%\Programs\Inno Setup 6\ISCC.exe
if not exist "%ISCC%" set ISCC=%ProgramFiles(x86)%\Inno Setup 6\ISCC.exe
if not exist "%ISCC%" set ISCC=%ProgramFiles%\Inno Setup 6\ISCC.exe
"%ISCC%" Notepad9.iss || exit /b 1
echo.
echo Installer: %~dp0output
