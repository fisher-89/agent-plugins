@echo off
REM Wrapper for embedded openspec CLI (standalone bundle)
set "SCRIPT_DIR=%~dp0"
node "%SCRIPT_DIR%openspec-bundled.js" %*
