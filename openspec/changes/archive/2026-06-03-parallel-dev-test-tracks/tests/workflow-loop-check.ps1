<#
.SYNOPSIS
    Temporary verification script for AC-17: workflow skill files no longer call phase/check.

.DESCRIPTION
    Verifies that workflow skill files (.md) do NOT contain calls to the phase/check MCP tool.
    phase/next is now the single decision point; phase/check is deprecated from the workflow loop.

    Run: pwsh -NoProfile workflow-loop-check.ps1
    Exit code: 0 if all checks pass, 1 if any file still references phase/check.

.NOTES
    AC-17: workflow skill files contain no phase/check MCP call; phase/next is the only decision point.
#>

param(
    [string]$ProjectRoot = (Resolve-Path "$PSScriptRoot/../../../..")
)

$skillFiles = @(
    "$ProjectRoot/plugins/dev-team/skills/workflow-requirement/SKILL.md"
)

$allPassed = $true

foreach ($file in $skillFiles) {
    if (-not (Test-Path $file)) {
        Write-Warning "SKIPPED (file not found): $file"
        continue
    }

    $content = Get-Content $file -Raw
    if ($content -match 'phase/check') {
        Write-Error "FAIL: $file contains 'phase/check' reference"
        $allPassed = $false
    } else {
        Write-Host "PASS: $file has no phase/check reference"
    }
}

if ($allPassed) {
    Write-Host "`nAC-17 passed: All workflow skill files use phase/next as the single decision point."
    exit 0
} else {
    Write-Error "`nAC-17 FAILED: Some workflow skill files still reference phase/check."
    exit 1
}
#>