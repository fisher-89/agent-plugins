@echo off
REM Wrapper for embedded dev-team CLI (standalone bundle)
set "SCRIPT_DIR=%~dp0"
set "BUNDLE=%SCRIPT_DIR%dev-team-bundle.js"

if not exist "%BUNDLE%" (
  echo 错误: dev-team-bundle.js 未找到，请先执行 npm run build
  exit /b 1
)

node "%BUNDLE%" %*
