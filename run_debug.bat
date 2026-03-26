@echo off
echo ========================================
echo Window Selector - Debug Mode
echo ========================================
echo.
echo 快捷鍵:
echo   Ctrl+Alt+Q  - 全螢幕覆蓋層
echo   Win+Y       - 標籤模式
echo.
echo 日誌位置: %%APPDATA%%\window-selector\logs\
echo.
echo 按 Ctrl+C 結束程式
echo ========================================
echo.

cd /d "%~dp0"
.\target\release\window-selector.exe --debug

pause
