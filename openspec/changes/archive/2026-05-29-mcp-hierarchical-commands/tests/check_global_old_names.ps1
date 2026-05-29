<#
.SYNOPSIS
全局 grep 旧名称模式，确保源文件中没有旧名称残留。

.DESCRIPTION
AC-7: 所有 eval_log 引用替换为 eval/log
AC-8: 所有 eval_check 引用替换为 eval/check
AC-9: 所有 archi_* 引用替换为 archi/*

搜索范围: bin/src/, skills/, agents/ (排除 node_modules, .git)
#>

$SCRIPT_DIR = Split-Path -Parent $MyInvocation.MyCommand.Path
$PROJECT_ROOT = Resolve-Path "$SCRIPT_DIR/../../../.."

$SEARCH_ROOTS = @(
    (Join-Path $PROJECT_ROOT "plugins/dev-team/bin/src"),
    (Join-Path $PROJECT_ROOT "plugins/dev-team/skills"),
    (Join-Path $PROJECT_ROOT "plugins/dev-team/agents")
)

$EXCLUDE_DIRS = @("node_modules", "dist", ".git")

$INCLUDE_GLOBS = @("*.ts", "*.md", "*.json")

$global:PASS_COUNT = 0
$global:FAIL_COUNT = 0

function Pass($name) {
    Write-Host "  PASS: $name"
    $global:PASS_COUNT++
}

function Fail($name, $detail) {
    Write-Host "  FAIL: $name -- $detail"
    $global:FAIL_COUNT++
}

function Search-OldNames {
    param([string[]]$SearchRoots, [string]$Pattern, [string[]]$ExcludeDirs)

    $results = @()
    foreach ($SearchRoot in $SearchRoots) {
        if (-not (Test-Path $SearchRoot)) { continue }
        try {
            $files = Get-ChildItem -Path $SearchRoot -File -Recurse -Include $INCLUDE_GLOBS -ErrorAction SilentlyContinue |
                     Where-Object {
                         $path = $_.FullName
                         $exclude = $false
                         foreach ($dir in $ExcludeDirs) {
                             if ($path -match [regex]::Escape($dir) + "\\" -or $path -match [regex]::Escape($dir) + "/") {
                                 $exclude = $true; break
                             }
                         }
                         -not $exclude
                     }
            foreach ($file in $files) {
                $content = Get-Content -Path $file.FullName -Raw -ErrorAction SilentlyContinue
                if ($null -eq $content) { continue }
                $m = [regex]::Matches($content, $Pattern)
                if ($m.Count -gt 0) {
                    $results += [PSCustomObject]@{ File = $file.FullName; Count = $m.Count }
                }
            }
        } catch {
            Write-Host "  WARN: $SearchRoot : $_" -ForegroundColor Yellow
        }
    }
    return $results
}

function Test-Ac7EvalLog {
    Write-Host "`n=== AC-7: 检查全局 eval_log 旧名称残留 ===" -ForegroundColor Cyan
    $results = Search-OldNames -SearchRoots $SEARCH_ROOTS -Pattern "eval_log" -ExcludeDirs $EXCLUDE_DIRS
    if ($results.Count -eq 0) {
        Pass "AC-7: 全局无 eval_log 旧名称残留"
    } else {
        foreach ($r in $results) { Write-Host "    $($r.File.Substring($PROJECT_ROOT.Length + 1)) ($($r.Count) 处)" -ForegroundColor Yellow }
        Fail "AC-7: 仍有 eval_log 旧名称残留" "$($results.Count) 个文件"
    }
}

function Test-Ac8EvalCheck {
    Write-Host "`n=== AC-8: 检查全局 eval_check 旧名称残留 ===" -ForegroundColor Cyan
    $results = Search-OldNames -SearchRoots $SEARCH_ROOTS -Pattern "eval_check" -ExcludeDirs $EXCLUDE_DIRS
    if ($results.Count -eq 0) {
        Pass "AC-8: 全局无 eval_check 旧名称残留"
    } else {
        foreach ($r in $results) { Write-Host "    $($r.File.Substring($PROJECT_ROOT.Length + 1)) ($($r.Count) 处)" -ForegroundColor Yellow }
        Fail "AC-8: 仍有 eval_check 旧名称残留" "$($results.Count) 个文件"
    }
}

function Test-Ac9ArchiStar {
    Write-Host "`n=== AC-9: 检查全局 archi_* 旧名称残留 ===" -ForegroundColor Cyan
    $totalFound = 0
    foreach ($pattern in @("archi_query", "archi_validate", "archi_write", "archi_check")) {
        $results = Search-OldNames -SearchRoots $SEARCH_ROOTS -Pattern $pattern -ExcludeDirs $EXCLUDE_DIRS
        if ($results.Count -gt 0) {
            Write-Host "  模式 $pattern : $($results.Count) 个文件" -ForegroundColor Yellow
            $totalFound += $results.Count
        }
    }
    if ($totalFound -eq 0) {
        Pass "AC-9: 全局无 archi_* 旧名称残留"
    } else {
        Fail "AC-9: 仍有 archi_* 旧名称残留" "$totalFound 个文件"
    }
}

function Main {
    Write-Host "==================================================" -ForegroundColor Magenta
    Write-Host "  全局旧名称残留检查" -ForegroundColor Magenta
    foreach ($root in $SEARCH_ROOTS) { Write-Host "  搜索: $root" -ForegroundColor Magenta }
    Write-Host "==================================================" -ForegroundColor Magenta

    Test-Ac7EvalLog
    Test-Ac8EvalCheck
    Test-Ac9ArchiStar

    $total = $global:PASS_COUNT + $global:FAIL_COUNT
    Write-Host "`n==================================================" -ForegroundColor Magenta
    Write-Host "  结果: $($global:PASS_COUNT) passed, $($global:FAIL_COUNT) failed (共 $total 项)" -ForegroundColor Magenta
    Write-Host "==================================================" -ForegroundColor Magenta

    if ($global:FAIL_COUNT -gt 0) { exit 1 }
    exit 0
}

Main
