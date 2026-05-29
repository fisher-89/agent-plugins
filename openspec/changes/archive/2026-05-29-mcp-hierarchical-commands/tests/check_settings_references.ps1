<#
.SYNOPSIS
验证 settings.local.json 权限 allowlist 中的 MCP tool 名称已更新为层级格式。

.DESCRIPTION
AC-6: settings.local.json 权限配置更新。
检查 allowlist 中的 MCP tool 名称，确保旧名称已替换为新名称。

旧名称 (不应存在):
  mcp__plugin_dev-team_dev-team__eval_check
  mcp__plugin_dev-team_dev-team__eval_log

新名称 (应存在):
  mcp__plugin_dev-team_dev-team__eval/check
  mcp__plugin_dev-team_dev-team__eval/log

使用:
    powershell -File check_settings_references.ps1
#>

# ===========================================================================
# 配置
# ===========================================================================

$SCRIPT_DIR = Split-Path -Parent $MyInvocation.MyCommand.Path
$PROJECT_ROOT = Resolve-Path "$SCRIPT_DIR/../../../.."

$SETTINGS_PATH = Join-Path $PROJECT_ROOT ".claude/settings.local.json"

# 旧名称 (以 FQN 形式出现在 allow 列表中)
$OLD_ALLOW_NAMES = @(
    "mcp__plugin_dev-team_dev-team__eval_check",
    "mcp__plugin_dev-team_dev-team__eval_log"
)

# 新名称 (应出现在 allow 列表中)
$NEW_ALLOW_NAMES = @(
    "mcp__plugin_dev-team_dev-team__eval/check",
    "mcp__plugin_dev-team_dev-team__eval/log"
)

# 计数器
$global:PASS_COUNT = 0
$global:FAIL_COUNT = 0

# ===========================================================================
# 帮助函数
# ===========================================================================

function Pass($name) {
    Write-Host "  PASS: $name"
    $global:PASS_COUNT++
}

function Fail($name, $detail) {
    Write-Host "  FAIL: $name -- $detail"
    $global:FAIL_COUNT++
}

function Assert-True($condition, $testName, $detail) {
    if ($condition) {
        Pass $testName
    } else {
        Fail $testName $detail
    }
}

function Assert-False($condition, $testName, $detail) {
    if (-not $condition) {
        Pass $testName
    } else {
        Fail $testName $detail
    }
}

# ===========================================================================
# 测试: AC-6 — settings.local.json 权限配置
# ===========================================================================

function Test-Ac6SettingsReferences {
    Write-Host "`n=== AC-6: settings.local.json 权限配置更新 ===" -ForegroundColor Cyan
    Write-Host "  配置文件: $SETTINGS_PATH"

    if (-not (Test-Path $SETTINGS_PATH)) {
        Fail "AC-6: settings.local.json 存在" "文件不存在: $SETTINGS_PATH"
        return
    }
    Pass "AC-6.a: settings.local.json 存在"

    try {
        $config = Get-Content -Path $SETTINGS_PATH -Raw -ErrorAction Stop | ConvertFrom-Json
    } catch {
        Fail "AC-6: 解析 settings.local.json" "解析失败: $_"
        return
    }

    # 获取 allow 列表
    $allowList = $config.permissions.allow
    if ($null -eq $allowList) {
        Fail "AC-6.b: permissions.allow 存在" "permissions.allow 不存在或为 null"
        return
    }
    Pass "AC-6.b: permissions.allow 存在"

    # 检查旧名称不在 allowlist 中
    foreach ($oldName in $OLD_ALLOW_NAMES) {
        $oldFound = $allowList -contains $oldName
        Assert-False $oldFound "AC-6.c: allowlist 不含旧名称 `"$oldName`"" "allowlist 仍包含旧名称: $oldName"
    }

    # 检查新名称在 allowlist 中
    foreach ($newName in $NEW_ALLOW_NAMES) {
        $newFound = $allowList -contains $newName
        Assert-True $newFound "AC-6.d: allowlist 包含新名称 `"$newName`"" "allowlist 中未找到新名称: $newName"
    }
}

# ===========================================================================
# 边界场景: 部分更新检测
# ===========================================================================

function Test-EdgePartialUpdate {
    Write-Host "`n=== 边界: 部分更新检测 ===" -ForegroundColor Cyan

    if (-not (Test-Path $SETTINGS_PATH)) {
        Write-Host "  SKIP: settings.local.json 不存在"
        return
    }

    try {
        $config = Get-Content -Path $SETTINGS_PATH -Raw -ErrorAction Stop | ConvertFrom-Json
    } catch {
        Write-Host "  SKIP: 无法解析 settings.local.json"
        return
    }

    $allowList = $config.permissions.allow
    if ($null -eq $allowList) {
        return
    }

    $allowText = $allowList -join " "

    # 检查是否有某个旧名称已更新而另一个未更新
    $foundOldCount = 0
    foreach ($oldName in $OLD_ALLOW_NAMES) {
        if ($allowList -contains $oldName) {
            $foundOldCount++
        }
    }

    if ($foundOldCount -gt 0 -and $foundOldCount -lt $OLD_ALLOW_NAMES.Count) {
        Fail "边界.a: 部分旧名称已更新" "已更新 $($OLD_ALLOW_NAMES.Count - $foundOldCount)/$($OLD_ALLOW_NAMES.Count)，残留 $foundOldCount 个旧名称"
    } elseif ($foundOldCount -gt 0) {
        # 全部旧名称都未更新 — 这个场景已经被 AC-6.c 覆盖
        Write-Host "  INFO: 所有旧名称均未更新 (已由 AC-6.c 覆盖)"
    } else {
        Pass "边界.a: 所有旧名称均已更新"
    }
}

# ===========================================================================
# 运行器
# ===========================================================================

function Main {
    Write-Host "==================================================" -ForegroundColor Magenta
    Write-Host "  settings.local.json 权限配置检查" -ForegroundColor Magenta
    Write-Host "==================================================" -ForegroundColor Magenta

    Test-Ac6SettingsReferences
    Test-EdgePartialUpdate

    # 汇总
    $total = $global:PASS_COUNT + $global:FAIL_COUNT
    Write-Host "`n==================================================" -ForegroundColor Magenta
    Write-Host "  结果: $($global:PASS_COUNT) passed, $($global:FAIL_COUNT) failed (共 $total 项)" -ForegroundColor Magenta
    Write-Host "==================================================" -ForegroundColor Magenta

    if ($global:FAIL_COUNT -gt 0) {
        exit 1
    }
    exit 0
}

Main
