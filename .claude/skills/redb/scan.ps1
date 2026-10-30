#!/usr/bin/env pwsh
# redb-peek 的批量驱动：扫描一个数据根目录下的所有 .redb，跨库汇总表与行数。
#
# 用法:
#   ./scan.ps1 -Root "$env:APPDATA\dev.wps.dev-team"
#   ./scan.ps1 -Root "$env:APPDATA\dev.wps.dev-team" -Details
#
# `-Details` 会把每个库的表清单也打出来（默认只给每库一行摘要）。

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$Root,

    [switch]$Details
)

$ErrorActionPreference = 'Stop'

$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$exe = Join-Path $here 'target\release\redb-peek.exe'
if (-not (Test-Path $exe)) {
    $exe = Join-Path $here 'target\release\redb-peek'
}
if (-not (Test-Path $exe)) {
    Write-Error "找不到 redb-peek 二进制。先在本目录跑: cargo build --release"
}

if (-not (Test-Path $Root)) {
    Write-Error "数据根目录不存在: $Root"
}

$dbs = Get-ChildItem -Path $Root -Recurse -Filter '*.redb' -File -ErrorAction SilentlyContinue |
    Sort-Object FullName

if (-not $dbs) {
    Write-Host "在 $Root 下没有找到 .redb 文件"
    exit 0
}

Write-Host "== 扫描 $($dbs.Count) 个库 @ $Root`n"

$grandTables = 0
$grandRows = 0
$ok = 0
$skipped = @()

foreach ($db in $dbs) {
    $rel = $db.FullName.Substring($Root.Length).TrimStart('\', '/')
    $out = & $exe $db.FullName tables --jsonl 2>&1
    if ($LASTEXITCODE -ne 0) {
        $reason = ($out | Out-String).Trim()
        Write-Host ("[跳过] {0}`n       {1}" -f $rel, $reason)
        $skipped += $rel
        continue
    }

    $ok++
    $rows = @($out | Where-Object { $_.Trim() } | ForEach-Object { $_ | ConvertFrom-Json })
    $tableCount = $rows.Count
    $rowTotal = ($rows | Measure-Object -Property rows -Sum).Sum
    if ($null -eq $rowTotal) { $rowTotal = 0 }
    $grandTables += $tableCount
    $grandRows += $rowTotal

    # `metadata` 是 native_db 自己的版本表，不算业务数据。
    $biggest = ($rows |
        Where-Object { $_.table -ne 'metadata' } |
        Sort-Object rows -Descending |
        Select-Object -First 1)
    $hint = if ($biggest -and $biggest.rows -gt 0) {
        "最大表 $($biggest.table) ($($biggest.rows) 行)"
    } else {
        "无业务数据行"
    }
    Write-Host ("[OK]   {0}`n       {1} 张表, {2} 行; {3}" -f $rel, $tableCount, $rowTotal, $hint)

    if ($Details) {
        foreach ($r in $rows) {
            $kind = if ($r.kind -eq 'multimap') { 'idx' } else { 'tbl' }
            $model = if ($null -ne $r.model_id) { "id=$($r.model_id) v$($r.model_version)" } else { '-' }
            Write-Host ("         {0,-34} {1,-4} {2,8}  {3}" -f $r.table, $kind, $r.rows, $model)
        }
    }
}

Write-Host "`n== 汇总"
Write-Host "  可读库: $ok / $($dbs.Count)"
if ($skipped.Count -gt 0) {
    Write-Host "  跳过:   $($skipped.Count)（见上方原因）"
}
Write-Host "  表总数: $grandTables"
Write-Host "  行总数: $grandRows"
