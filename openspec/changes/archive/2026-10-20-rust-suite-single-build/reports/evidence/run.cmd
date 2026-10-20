@echo off
cargo llvm-cov --workspace --json --output-path "C:\Users\wps\AppData\Local\Temp\llvmcov-s1-20261019-121051\coverage-summary.json" > "C:\Users\wps\AppData\Local\Temp\llvmcov-s1-20261019-121051\results.txt" 2> "C:\Users\wps\AppData\Local\Temp\llvmcov-s1-20261019-121051\stderr.txt"
exit /b %ERRORLEVEL%
