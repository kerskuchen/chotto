@echo off
REM WARNING: This file was generated by `ct_makeproject` and should not be modified

cargo run --package ct_makeproject
if %errorlevel% neq 0 goto :error
goto :done

REM ------------------------------------------------------------------------------------------------
:error
echo Failed with error #%errorlevel%.
REM the `if %1.==.` checks if we passed an argument. If not we pause the script. This is useful
REM if we want to call this script from VSCode and don't want to pause
if %1.==. pause
exit /b %errorlevel%

REM ------------------------------------------------------------------------------------------------
:done
echo FINISHED PROJECT REFRESH