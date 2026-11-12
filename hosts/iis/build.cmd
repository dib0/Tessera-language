@echo off
rem Builds TesseraModule.dll (native IIS module) with MSVC.
rem Run from an "x64 Native Tools Command Prompt for VS" (cl.exe on PATH,
rem Windows SDK installed: httpserv.h lives in the SDK "um" include folder),
rem or from any shell after running VC\Auxiliary\Build\vcvars64.bat.
rem Builds clean with /W4 on Visual Studio 2022 and Windows SDK 10.0.26100;
rem loading it into IIS is still untested, see ..\README.md.
setlocal
cd /d "%~dp0"
cl /nologo /LD /O2 /W4 /EHsc /std:c++17 /DUNICODE /D_UNICODE /I..\include TesseraModule.cpp ^
   /link /DEF:TesseraModule.def /OUT:TesseraModule.dll kernel32.lib advapi32.lib ws2_32.lib
if errorlevel 1 exit /b 1
echo Built %~dp0TesseraModule.dll
